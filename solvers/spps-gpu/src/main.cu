// spps-gpu: a drop-in for upstream I-Simpa's spps.exe. Usage:
//   spps-gpu <config.xml>           the GPU walk (CUDA), or the CPU walk when SPPS_GPU_BACKEND=cpu
//   spps-gpu --cpu <config.xml>     the same walk on every CPU thread (OpenMP)
//   spps-gpu --probe                print the CUDA device, or why there is none; exit 0 or 1
// Bed options (A2 verification): --dump-walk <file> writes every primary particle's step count,
// fate and final energy; --dump-sums <file> writes the double accumulators of every band.
#include <cuda_runtime.h>
#include <omp.h>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <string>
#include <vector>
#ifdef _WIN32
#include <intrin.h>
#endif
#include "walk.h"
#include "model.h"
#include "output.h"

using namespace spg;

static const char* VERSION = "0.1.0 (A2, 2026-10-06)";

struct WalkRec { uint32_t steps; int32_t state; double E; uint32_t children; uint32_t pad; uint64_t childSteps; };

// ---------------------------------------------------------------------------------------------
// GPU accumulator: double atomics for the per-step and point-receiver sums, float atomics for the
// surface and cut cells; the per-step total is summed across the warp's lanes at the same step first.
struct GpuAcc {
  double *total, *rpE, *rpLf, *rpLfc, *rpI, *rpSrc;
  float *surf, *cut;
  unsigned long long* states;   // 6 states, then child overflow
  double* overflowE;
  int nbSteps, nbBins, nbSrc, bySource;
  __host__ __device__ void addTotal(int step, double e) {
#ifdef __CUDA_ARCH__
    unsigned active = __activemask();
    unsigned peers = __match_any_sync(active, step);
    int lane = threadIdx.x & 31;
    int leader = __ffs(peers) - 1;
    double sum = 0;
    if (peers == 0xFFFFFFFFu) {
      sum = e;
      for (int o = 16; o > 0; o >>= 1) sum += __shfl_xor_sync(0xFFFFFFFFu, sum, o);
    } else {
      unsigned mm = peers;
      while (mm) {
        int l = __ffs(mm) - 1;
        sum += __shfl_sync(peers, e, l);
        mm &= mm - 1;
      }
    }
    if (lane == leader) atomicAdd(&total[step], sum);
#endif
  }
  __host__ __device__ void addRp(int r, int step, int src, double e, double lf, double lfc, double ix, double iy, double iz) {
#ifdef __CUDA_ARCH__
    size_t k = (size_t)r * nbSteps + step;
    atomicAdd(&rpE[k], e);
    atomicAdd(&rpLf[k], lf);
    atomicAdd(&rpLfc[k], lfc);
    atomicAdd(&rpI[3 * k], ix);
    atomicAdd(&rpI[3 * k + 1], iy);
    atomicAdd(&rpI[3 * k + 2], iz);
    size_t cols = bySource ? (size_t)nbSteps * nbSrc : (size_t)nbSrc;
    atomicAdd(&rpSrc[(size_t)r * cols + (bySource ? (size_t)step * nbSrc + src : (size_t)src)], e);
#endif
  }
  __host__ __device__ void addSurf(int face, int bin, double v) {
#ifdef __CUDA_ARCH__
    atomicAdd(&surf[(size_t)face * nbBins + bin], (float)v);
#endif
  }
  __host__ __device__ void addCut(long long idx, double v) {
#ifdef __CUDA_ARCH__
    atomicAdd(&cut[idx], (float)v);
#endif
  }
  __host__ __device__ void stat(int st) {
#ifdef __CUDA_ARCH__
    atomicAdd(&states[st], 1ull);
#endif
  }
  __host__ __device__ void childOverflow(double e) {
#ifdef __CUDA_ARCH__
    atomicAdd(&states[6], 1ull);
    atomicAdd(overflowE, e);
#endif
  }
};

__global__ void kFamilies(Scene s, const SrcBand* srcs, int N, long long first, long long count, GpuAcc acc, WalkRec* dump) {
  long long g = first + (long long)blockIdx.x * blockDim.x + threadIdx.x;
  if (g >= first + count) return;
  int src = (int)(g / N);
  uint32_t idpart = (uint32_t)(g % N);
  const SrcBand sb = srcs[src];
  if (!sb.active) return;
  NullRec rec;
  Walker<GpuAcc, NullRec> w(s, acc, rec);
  if (dump) {
    WalkRec& d = dump[g];
    int st; double E; uint32_t steps, ch; uint64_t chs;
    w.family(sb, idpart, src, 0, &steps, &st, &E, &ch, &chs);
    d.steps = steps; d.state = st; d.E = E; d.children = ch; d.childSteps = chs; d.pad = 0;
  } else {
    w.family(sb, idpart, src, 0, nullptr, nullptr, nullptr, nullptr, nullptr);
  }
}

// ---------------------------------------------------------------------------------------------
// CPU accumulator: private per-thread sums, merged in thread order; shared cells with CAS float adds
static inline void casAddF(float* p, float v) {
#ifdef _WIN32
  volatile long* lp = (volatile long*)p;
  long old = *lp;
  for (;;) {
    float f;
    memcpy(&f, (const void*)&old, 4);
    f += v;
    long nw;
    memcpy(&nw, &f, 4);
    long prev = _InterlockedCompareExchange(lp, nw, old);
    if (prev == old) break;
    old = prev;
  }
#else
#pragma omp atomic
  *p += v;
#endif
}
struct CpuAcc {
  std::vector<double> total, rpE, rpLf, rpLfc, rpI, rpSrc;
  float *surf = nullptr, *cut = nullptr;
  uint64_t states[7] = {0, 0, 0, 0, 0, 0, 0};
  double overflowE = 0;
  int nbSteps = 0, nbBins = 0, nbSrc = 0, bySource = 0;
  __host__ __device__ void addTotal(int step, double e) {
#ifndef __CUDA_ARCH__
    total[step] += e;
#endif
  }
  __host__ __device__ void addRp(int r, int step, int src, double e, double lf, double lfc, double ix, double iy, double iz) {
#ifndef __CUDA_ARCH__
    size_t k = (size_t)r * nbSteps + step;
    rpE[k] += e; rpLf[k] += lf; rpLfc[k] += lfc;
    rpI[3 * k] += ix; rpI[3 * k + 1] += iy; rpI[3 * k + 2] += iz;
    size_t cols = bySource ? (size_t)nbSteps * nbSrc : (size_t)nbSrc;
    rpSrc[(size_t)r * cols + (bySource ? (size_t)step * nbSrc + src : (size_t)src)] += e;
#endif
  }
  __host__ __device__ void addSurf(int face, int bin, double v) {
#ifndef __CUDA_ARCH__
    casAddF(&surf[(size_t)face * nbBins + bin], (float)v);
#endif
  }
  __host__ __device__ void addCut(long long idx, double v) {
#ifndef __CUDA_ARCH__
    casAddF(&cut[idx], (float)v);
#endif
  }
  __host__ __device__ void stat(int st) {
#ifndef __CUDA_ARCH__
    states[st]++;
#endif
  }
  __host__ __device__ void childOverflow(double e) {
#ifndef __CUDA_ARCH__
    states[6]++; overflowE += e;
#endif
  }
};
// No sums: the host re-trace of the particles SPPS writes to its particle file
struct NullAcc {
  __host__ __device__ void addTotal(int, double) {}
  __host__ __device__ void addRp(int, int, int, double, double, double, double, double, double) {}
  __host__ __device__ void addSurf(int, int, double) {}
  __host__ __device__ void addCut(long long, double) {}
  __host__ __device__ void stat(int) {}
  __host__ __device__ void childOverflow(double) {}
};
struct HostRec {
  ParticleFiles* pf;
  const Config* cfg;
  __host__ __device__ void newParticle(const Particle& p) {
#ifndef __CUDA_ARCH__
    if (p.flag) pf->newParticle();
#endif
  }
  __host__ __device__ void saveParticle() {
#ifndef __CUDA_ARCH__
    pf->save();
#endif
  }
  __host__ __device__ void step(const Particle& p) {
#ifndef __CUDA_ARCH__
    pf->step(p.pos, p.E, p.step);
#endif
  }
  __host__ __device__ void surfHit(const Particle& p, V3 n) {
#ifndef __CUDA_ARCH__
    pf->surfHit(n, p.reflOrder, p.colPos, p.dir, p.E);
#endif
  }
  __host__ __device__ void rpHit(const Particle& p, int r, double energy) {
#ifndef __CUDA_ARCH__
    if (!cfg->saveRp) return;
    float time = (float)p.step * cfg->dt + p.elapsed;
    pf->rpHit(time, r, p.dir, energy * cfg->receivers[r].cdt_vol);
#endif
  }
};

// ---------------------------------------------------------------------------------------------
struct HostScene {
  std::vector<int> rpOff, rpList, cutOff, cutList, smat;
  std::vector<unsigned char> senc;
  std::vector<V3> sn, rpPos, rpOri;
  std::vector<CutPlane> cuts;
  size_t cutCells = 0;
};

static void check(cudaError_t e, const char* what) {
  if (e != cudaSuccess) {
    std::cerr << "spps-gpu: cuda_error: " << what << ": " << cudaGetErrorString(e) << std::endl;
    exit(3);
  }
}
template <class T> static T* upload(const std::vector<T>& v) {
  T* d = nullptr;
  size_t n = v.empty() ? 1 : v.size();
  check(cudaMalloc(&d, n * sizeof(T)), "cudaMalloc");
  if (!v.empty()) check(cudaMemcpy(d, v.data(), v.size() * sizeof(T), cudaMemcpyHostToDevice), "cudaMemcpy");
  return d;
}

static int probe(std::string& desc) {
  int n = 0;
  cudaError_t e = cudaGetDeviceCount(&n);
  if (e != cudaSuccess) { desc = std::string("no CUDA device: ") + cudaGetErrorString(e); return 1; }
  if (n == 0) { desc = "no CUDA device: none found"; return 1; }
  cudaDeviceProp p;
  if (cudaGetDeviceProperties(&p, 0) != cudaSuccess) { desc = "no CUDA device: properties unreadable"; return 1; }
  int drv = 0, rt = 0;
  cudaDriverGetVersion(&drv);
  cudaRuntimeGetVersion(&rt);
  char buf[512];
  snprintf(buf, sizeof(buf), "%s, sm_%d%d, %d SMs, %.1f GiB, driver CUDA %d.%d, runtime %d.%d", p.name, p.major, p.minor,
           p.multiProcessorCount, p.totalGlobalMem / 1073741824.0, drv / 1000, (drv % 1000) / 10, rt / 1000, (rt % 1000) / 10);
  // a kernel must run on it, or the build has no code for this architecture
  void* d = nullptr;
  e = cudaMalloc(&d, 16);
  if (e != cudaSuccess) { desc = std::string("no usable CUDA device: ") + cudaGetErrorString(e); return 1; }
  cudaFree(d);
  desc = buf;
  return 0;
}

struct Progress {
  float last = 0;
  void show(double pct) {
    float cur = (float)pct;
    if ((int)(cur * 100) > (int)(last * 100) && cur < 100) {
      last = cur;
      std::cout.precision(4);
      std::cout << "#" << cur << std::endl;
    }
  }
};

int main(int argc, char** argv) {
  std::string path;
  bool cpu = false, probeOnly = false;
  std::string dumpWalk, dumpSums;
  for (int i = 1; i < argc; i++) {
    std::string a = argv[i];
    if (a == "--cpu") cpu = true;
    else if (a == "--gpu") cpu = false;
    else if (a == "--probe") probeOnly = true;
    else if (a == "--dump-walk" && i + 1 < argc) dumpWalk = argv[++i];
    else if (a == "--dump-sums" && i + 1 < argc) dumpSums = argv[++i];
    else if (a == "-v" && path.empty()) {}
    else if (path.empty()) path = a;
    else path += " " + a;   // sppsNantes.cpp:266-282: further words join the path
  }
  if (probeOnly) {
    std::string desc;
    int rc = probe(desc);
    std::cout << desc << std::endl;
    return rc;
  }
  if (const char* env = getenv("SPPS_GPU_BACKEND")) {
    if (std::string(env) == "cpu") cpu = true;
  }
  if (path.empty()) {
    std::cout << "The path of the XML configuration file must be specified!" << std::endl;
    return 1;
  }
  auto t0 = std::chrono::steady_clock::now();
  Config cfg;
  if (!loadConfig(path, cfg)) {
    // an unparseable config: SPPS reads nothing and fails on the scene mesh (docs/formats/config_xml.md)
    std::cout << "Unable to read the scene mesh file :" << std::endl << std::endl;
    return 0;
  }
  Model m;
  if (!loadScene(cfg, m)) return 0;   // SPPS's main returns 0 whatever MainProcess returns (sppsNantes.cpp:449-460)
  if (!loadTetra(cfg, m)) return 0;
  if (cfg.alog != 0 || cfg.blin != 0) { /* InitTetraCelerite would run here; refused below */ }
  prepare(cfg, m);
  std::vector<std::string> refused = refusals(cfg, m);
  if (!refused.empty()) {
    for (const std::string& r : refused) std::cerr << "spps-gpu: refused: " << r << std::endl;
    return 2;
  }
  if (!checkSourcePosition(cfg, m)) {
    std::cerr << "A sound source position is intersecting with the 3D model. Move the sound source inside the 3D model" << std::endl;
    return 0;
  }
  uint32_t seed = cfg.seed != 0 ? (uint32_t)cfg.seed : (uint32_t)time(nullptr);
  std::string device;
  if (!cpu) {
    if (probe(device) != 0) {
      std::cerr << "spps-gpu: refused: no_cuda_device: " << device << "; run with --cpu (or SPPS_GPU_BACKEND=cpu) for the CPU build of the same walk" << std::endl;
      return 2;
    }
  }

  const int T = m.T, R = (int)cfg.receivers.size(), NS = (int)cfg.sources.size(), S = cfg.nbSteps, NB = cfg.nbBins;
  // flat scene
  HostScene hs;
  hs.rpOff.assign(T + 1, 0);
  hs.cutOff.assign(T + 1, 0);
  for (int t = 0; t < T; t++) {
    hs.rpOff[t] = (int)hs.rpList.size();
    for (int r : m.rpByTetra[t]) hs.rpList.push_back(r);
    hs.cutOff[t] = (int)hs.cutList.size();
    for (int c : m.cutByTetra[t]) hs.cutList.push_back(c);
  }
  hs.rpOff[T] = (int)hs.rpList.size();
  hs.cutOff[T] = (int)hs.cutList.size();
  for (const auto& f : m.faces) { hs.sn.push_back(f.normal); hs.smat.push_back(f.mat); hs.senc.push_back(f.enc >= 0 ? 1 : 0); }
  for (const auto& r : cfg.receivers) { hs.rpPos.push_back(r.pos); hs.rpOri.push_back(r.orient); }
  for (const auto& c : cfg.cuts) {
    CutPlane cp;
    cp.A = c.A; cp.B = c.B; cp.C = c.C; cp.normal = c.normal; cp.nu = c.nu; cp.nv = c.nv;
    cp.base = (long long)hs.cutCells * NB;
    hs.cuts.push_back(cp);
    hs.cutCells += (size_t)c.nu * c.nv;
  }
  const size_t surfN = (size_t)m.nbRxFaces * NB, cutN = hs.cutCells * NB;
  const size_t srcCols = cfg.bySource ? (size_t)S * NS : (size_t)NS;

  Scene base{};
  base.R = R; base.radius = cfg.radius; base.C = (int)hs.cuts.size();
  base.dt = cfg.dt; base.nbSteps = S; base.absAtmo = cfg.absAtmoCalc != 0; base.energetic = cfg.method != 0;
  base.directCalc = cfg.directCalc != 0; base.transCalc = cfg.transCalc != 0; base.surfMode = cfg.surfMode;
  base.ratio = cfg.ratio; base.nbBins = NB; base.nbSrc = NS; base.bySource = cfg.bySource != 0;
  base.saveSurfHist = cfg.saveSurf != 0; base.saveRpHist = cfg.saveRp != 0; base.seed = seed;

  // device copies of the band-independent arrays
  struct Dev { V3 *nodes, *fn, *sn, *rpPos, *rpOri; int *fv, *nb, *sf, *rf, *corner, *rpOff, *rpList, *cutOff, *cutList, *smat; unsigned char* senc; CutPlane* cuts; MatBand* mat; SrcBand* srcs; } dv{};
  GpuAcc ga{};
  WalkRec* dDump = nullptr;
  const long long totalFamilies = (long long)NS * cfg.nbPart;
  if (!cpu) {
    size_t freeB = 0, totB = 0;
    cudaMemGetInfo(&freeB, &totB);
    size_t need = (surfN + cutN) * 4 + (S + 6ull * R * S + R * srcCols) * 8 + 64ull * 1048576;
    if (need > freeB) {
      std::cerr << "spps-gpu: refused: surface_maps_too_large: the band's sums need " << need / 1048576 << " MiB of device memory, "
                << freeB / 1048576 << " MiB are free; set recepteurs_surfaciques_pas_temps to a longer bin (patch 0001) or a coarser plane" << std::endl;
      return 2;
    }
    dv.nodes = upload(m.nodes); dv.fn = upload(m.fn); dv.sn = upload(hs.sn); dv.rpPos = upload(hs.rpPos); dv.rpOri = upload(hs.rpOri);
    dv.fv = upload(m.fv); dv.nb = upload(m.nb); dv.sf = upload(m.sf); dv.rf = upload(m.rf); dv.corner = upload(m.corner);
    dv.rpOff = upload(hs.rpOff); dv.rpList = upload(hs.rpList); dv.cutOff = upload(hs.cutOff); dv.cutList = upload(hs.cutList);
    dv.smat = upload(hs.smat); dv.senc = upload(hs.senc); dv.cuts = upload(hs.cuts);
    check(cudaMalloc(&dv.mat, std::max<size_t>(1, cfg.materials.size()) * sizeof(MatBand)), "mat");
    check(cudaMalloc(&dv.srcs, std::max<size_t>(1, (size_t)NS) * sizeof(SrcBand)), "srcs");
    check(cudaMalloc(&ga.total, std::max<size_t>(1, S) * 8), "total");
    check(cudaMalloc(&ga.rpE, std::max<size_t>(1, (size_t)R * S) * 8), "rpE");
    check(cudaMalloc(&ga.rpLf, std::max<size_t>(1, (size_t)R * S) * 8), "rpLf");
    check(cudaMalloc(&ga.rpLfc, std::max<size_t>(1, (size_t)R * S) * 8), "rpLfc");
    check(cudaMalloc(&ga.rpI, std::max<size_t>(1, 3ull * R * S) * 8), "rpI");
    check(cudaMalloc(&ga.rpSrc, std::max<size_t>(1, (size_t)R * srcCols) * 8), "rpSrc");
    check(cudaMalloc(&ga.surf, std::max<size_t>(1, surfN) * 4), "surf");
    check(cudaMalloc(&ga.cut, std::max<size_t>(1, cutN) * 4), "cut");
    check(cudaMalloc(&ga.states, 8 * 8), "states");
    check(cudaMalloc(&ga.overflowE, 8), "overflow");
    ga.nbSteps = S; ga.nbBins = NB; ga.nbSrc = NS; ga.bySource = base.bySource;
    if (!dumpWalk.empty()) check(cudaMalloc(&dDump, std::max<long long>(1, totalFamilies) * sizeof(WalkRec)), "dump");
    Scene& b = base;
    b.nodes = dv.nodes; b.fv = dv.fv; b.fn = dv.fn; b.nb = dv.nb; b.sf = dv.sf; b.rf = dv.rf; b.corner = dv.corner;
    b.rpOff = dv.rpOff; b.rpList = dv.rpList; b.cutOff = dv.cutOff; b.cutList = dv.cutList; b.sn = dv.sn; b.smat = dv.smat;
    b.senc = dv.senc; b.mat = dv.mat; b.rpPos = dv.rpPos; b.rpOri = dv.rpOri; b.cuts = dv.cuts;
  }

  Report report(cfg, m);
  Progress prog;
  std::ofstream walkOut, sumsOut;
  if (!dumpWalk.empty()) walkOut.open(std::filesystem::u8path(dumpWalk), std::ios::binary | std::ios::trunc);
  if (!dumpSums.empty()) sumsOut.open(std::filesystem::u8path(dumpSums), std::ios::binary | std::ios::trunc);
  double traceSeconds = 0;
  uint64_t overflowTotal = 0;
  int bandsDone = 0;
  const int nbCalc = std::max(1, cfg.nbBandsCalc);
  int threads = 1;

  for (size_t bi = 0; bi < cfg.bands.size(); bi++) {
    if (!cfg.bands[bi].docalc) continue;
    // material and source parameters at the band
    std::vector<MatBand> mats(cfg.materials.size());
    for (size_t i = 0; i < cfg.materials.size(); i++) {
      const MatBandCfg& mb = cfg.materials[i].bands[bi];
      mats[i].absorption = mb.absorption; mats[i].diffusion = mb.diffusion; mats[i].tau = mb.tau;
      mats[i].dotransmission = mb.dotrans; mats[i].law = mb.law; mats[i].doubleSided = cfg.materials[i].doubleSided ? 1 : 0;
    }
    std::vector<SrcBand> sbs(NS);
    for (int i = 0; i < NS; i++) {
      const Source& s = cfg.sources[i];
      SrcBand& sb = sbs[i];
      sb.pos = s.pos;
      sb.tetra = s.tetra;
      sb.type = s.type;
      // sppsNantes.cpp:73-93
      float e = s.w[bi] / (float)cfg.nbPart;
      sb.energie = e;
      sb.eps = sb.energie * pow(10., -(double)cfg.transEps);
      sb.norm = cfg.c * cfg.dt;
      sb.dirUnit = s.direction;
      sb.startStep = (int)(uint16_t)(int)ceilf(s.delay / cfg.dt);
      sb.active = (s.tetra >= 0 && S > sb.startStep) ? 1 : 0;
      if (s.tetra < 0) std::cerr << "Unable to find the source position!";
    }
    Scene sc = base;
    sc.band = (uint32_t)bi;
    sc.densite = cfg.bands[bi].densite;
    BandSums bs;
    bs.band = (int)bi;
    bs.total.assign(S, 0.0);
    bs.rpE.assign((size_t)R * S, 0.0); bs.rpLf.assign((size_t)R * S, 0.0); bs.rpLfc.assign((size_t)R * S, 0.0);
    bs.rpI.assign(3ull * R * S, 0.0);
    bs.rpSrc.assign((size_t)R * srcCols, 0.0);
    bs.surf.assign(surfN, 0.f);
    bs.cut.assign(cutN, 0.f);
    std::vector<WalkRec> walk;
    if (!dumpWalk.empty()) walk.assign((size_t)totalFamilies, WalkRec{});
    auto tb = std::chrono::steady_clock::now();
    if (!cpu) {
      check(cudaMemcpy(dv.mat, mats.data(), mats.size() * sizeof(MatBand), cudaMemcpyHostToDevice), "mat");
      if (NS) check(cudaMemcpy(dv.srcs, sbs.data(), NS * sizeof(SrcBand), cudaMemcpyHostToDevice), "srcs");
      cudaMemset(ga.total, 0, std::max<size_t>(1, S) * 8);
      cudaMemset(ga.rpE, 0, std::max<size_t>(1, (size_t)R * S) * 8);
      cudaMemset(ga.rpLf, 0, std::max<size_t>(1, (size_t)R * S) * 8);
      cudaMemset(ga.rpLfc, 0, std::max<size_t>(1, (size_t)R * S) * 8);
      cudaMemset(ga.rpI, 0, std::max<size_t>(1, 3ull * R * S) * 8);
      cudaMemset(ga.rpSrc, 0, std::max<size_t>(1, (size_t)R * srcCols) * 8);
      cudaMemset(ga.surf, 0, std::max<size_t>(1, surfN) * 4);
      cudaMemset(ga.cut, 0, std::max<size_t>(1, cutN) * 4);
      cudaMemset(ga.states, 0, 64);
      cudaMemset(ga.overflowE, 0, 8);
      // launches short enough to stay well under the display driver's watchdog (TDR)
      long long chunk = 16384, done = 0;
      cudaEvent_t e0, e1;
      cudaEventCreate(&e0); cudaEventCreate(&e1);
      while (done < totalFamilies) {
        long long n = std::min(chunk, totalFamilies - done);
        cudaEventRecord(e0);
        kFamilies<<<(unsigned)((n + 127) / 128), 128>>>(sc, dv.srcs, cfg.nbPart, done, n, ga, dDump);
        cudaEventRecord(e1);
        check(cudaEventSynchronize(e1), "kernel");
        check(cudaGetLastError(), "kernel launch");
        float ms = 0;
        cudaEventElapsedTime(&ms, e0, e1);
        done += n;
        double target = 300.0;   // ms
        if (ms > 0) chunk = (long long)std::max(1024.0, std::min(4.0 * chunk, chunk * target / ms));
        prog.show(100.0 * (bandsDone + (double)done / std::max(1LL, totalFamilies)) / nbCalc);
      }
      cudaEventDestroy(e0); cudaEventDestroy(e1);
      check(cudaMemcpy(bs.total.data(), ga.total, (size_t)S * 8, cudaMemcpyDeviceToHost), "copy");
      if (R) {
        check(cudaMemcpy(bs.rpE.data(), ga.rpE, (size_t)R * S * 8, cudaMemcpyDeviceToHost), "copy");
        check(cudaMemcpy(bs.rpLf.data(), ga.rpLf, (size_t)R * S * 8, cudaMemcpyDeviceToHost), "copy");
        check(cudaMemcpy(bs.rpLfc.data(), ga.rpLfc, (size_t)R * S * 8, cudaMemcpyDeviceToHost), "copy");
        check(cudaMemcpy(bs.rpI.data(), ga.rpI, 3ull * R * S * 8, cudaMemcpyDeviceToHost), "copy");
        check(cudaMemcpy(bs.rpSrc.data(), ga.rpSrc, (size_t)R * srcCols * 8, cudaMemcpyDeviceToHost), "copy");
      }
      if (surfN) check(cudaMemcpy(bs.surf.data(), ga.surf, surfN * 4, cudaMemcpyDeviceToHost), "copy");
      if (cutN) check(cudaMemcpy(bs.cut.data(), ga.cut, cutN * 4, cudaMemcpyDeviceToHost), "copy");
      unsigned long long st[8];
      check(cudaMemcpy(st, ga.states, 64, cudaMemcpyDeviceToHost), "copy");
      for (int k = 0; k < 6; k++) bs.states[k] = st[k];
      bs.childOverflow = st[6];
      check(cudaMemcpy(&bs.overflowEnergy, ga.overflowE, 8, cudaMemcpyDeviceToHost), "copy");
      if (dDump) check(cudaMemcpy(walk.data(), dDump, (size_t)totalFamilies * sizeof(WalkRec), cudaMemcpyDeviceToHost), "copy");
    } else {
      // the CPU build: host pointers
      sc.nodes = m.nodes.data(); sc.fv = m.fv.data(); sc.fn = m.fn.data(); sc.nb = m.nb.data(); sc.sf = m.sf.data(); sc.rf = m.rf.data();
      sc.corner = m.corner.data(); sc.rpOff = hs.rpOff.data(); sc.rpList = hs.rpList.data(); sc.cutOff = hs.cutOff.data();
      sc.cutList = hs.cutList.data(); sc.sn = hs.sn.data(); sc.smat = hs.smat.data(); sc.senc = hs.senc.data(); sc.mat = mats.data();
      sc.rpPos = hs.rpPos.data(); sc.rpOri = hs.rpOri.data(); sc.cuts = hs.cuts.data();
      int nt = omp_get_max_threads();
      threads = nt;
      std::vector<CpuAcc> accs(nt);
      for (CpuAcc& a : accs) {
        a.total.assign(S, 0.0); a.rpE.assign((size_t)R * S, 0.0); a.rpLf.assign((size_t)R * S, 0.0); a.rpLfc.assign((size_t)R * S, 0.0);
        a.rpI.assign(3ull * R * S, 0.0); a.rpSrc.assign((size_t)R * srcCols, 0.0);
        a.surf = bs.surf.data(); a.cut = bs.cut.data();
        a.nbSteps = S; a.nbBins = NB; a.nbSrc = NS; a.bySource = base.bySource;
      }
      long long done = 0;
      const long long block = 65536;
      std::vector<Walker<CpuAcc, NullRec>*> walkers(nt, nullptr);
      std::vector<NullRec> recs(nt);
      for (int t = 0; t < nt; t++) walkers[t] = new Walker<CpuAcc, NullRec>(sc, accs[t], recs[t]);
      while (done < totalFamilies) {
        long long n = std::min(block, totalFamilies - done);
#pragma omp parallel for schedule(dynamic, 64)
        for (long long g = done; g < done + n; g++) {
          int tid = omp_get_thread_num();
          int src = (int)(g / cfg.nbPart);
          uint32_t idpart = (uint32_t)(g % cfg.nbPart);
          const SrcBand& sb = sbs[src];
          if (!sb.active) continue;
          Walker<CpuAcc, NullRec>* w = walkers[tid];
          if (!walk.empty()) {
            WalkRec& d = walk[g];
            int st; double E; uint32_t steps, ch; uint64_t chs;
            w->family(sb, idpart, src, 0, &steps, &st, &E, &ch, &chs);
            d.steps = steps; d.state = st; d.E = E; d.children = ch; d.childSteps = chs; d.pad = 0;
          } else {
            w->family(sb, idpart, src, 0, nullptr, nullptr, nullptr, nullptr, nullptr);
          }
        }
        done += n;
        prog.show(100.0 * (bandsDone + (double)done / std::max(1LL, totalFamilies)) / nbCalc);
      }
      for (auto* w : walkers) delete w;
      for (CpuAcc& a : accs) {
        for (int s = 0; s < S; s++) bs.total[s] += a.total[s];
        for (size_t k = 0; k < bs.rpE.size(); k++) { bs.rpE[k] += a.rpE[k]; bs.rpLf[k] += a.rpLf[k]; bs.rpLfc[k] += a.rpLfc[k]; }
        for (size_t k = 0; k < bs.rpI.size(); k++) bs.rpI[k] += a.rpI[k];
        for (size_t k = 0; k < bs.rpSrc.size(); k++) bs.rpSrc[k] += a.rpSrc[k];
        for (int k = 0; k < 6; k++) bs.states[k] += a.states[k];
        bs.childOverflow += a.states[6];
        bs.overflowEnergy += a.overflowE;
      }
    }
    traceSeconds += std::chrono::duration<double>(std::chrono::steady_clock::now() - tb).count();
    overflowTotal += bs.childOverflow;
    if (bs.childOverflow)
      std::cerr << "spps-gpu: warning: child_queue_overflow: " << bs.childOverflow << " transmitted particles dropped at " << cfg.bands[bi].freq
                << " Hz (queue of " << QCAP << " per particle), energy " << bs.overflowEnergy << " J" << std::endl;
    // the particle file: SPPS marks every k-th particle of each source (sppsNantes.cpp:66-72, 129-138);
    // the same walk re-traces those particles on the host, identical paths, to record them
    if ((size_t)cfg.nbPartRender * NS != 0) {
      ParticleFiles pf(cfg, (int)bi);
      Scene hsc = sc;
      hsc.nodes = m.nodes.data(); hsc.fv = m.fv.data(); hsc.fn = m.fn.data(); hsc.nb = m.nb.data(); hsc.sf = m.sf.data(); hsc.rf = m.rf.data();
      hsc.corner = m.corner.data(); hsc.rpOff = hs.rpOff.data(); hsc.rpList = hs.rpList.data(); hsc.cutOff = hs.cutOff.data();
      hsc.cutList = hs.cutList.data(); hsc.sn = hs.sn.data(); hsc.smat = hs.smat.data(); hsc.senc = hs.senc.data(); hsc.mat = mats.data();
      hsc.rpPos = hs.rpPos.data(); hsc.rpOri = hs.rpOri.data(); hsc.cuts = hs.cuts.data();
      NullAcc na;
      HostRec hr{&pf, &cfg};
      for (int src = 0; src < NS; src++) {
        const SrcBand& sb = sbs[src];
        if (!sb.active) continue;
        float rapport = 1;
        if (cfg.nbPart > 0) rapport = (float)cfg.nbPartRender / (float)cfg.nbPart;
        if (!(rapport >= 0 && rapport <= 1)) rapport = 0;
        float current = 0;
        for (int idpart = 1; idpart <= cfg.nbPart; idpart++) {
          current += rapport;
          bool flag = false;
          if (current >= 1) { flag = true; current = 0; }
          if (!flag) continue;
          Walker<NullAcc, HostRec>* w = new Walker<NullAcc, HostRec>(hsc, na, hr);
          w->family(sb, (uint32_t)(idpart - 1), src, 1, nullptr, nullptr, nullptr, nullptr, nullptr);
          delete w;
        }
      }
      pf.close();
    }
    if (walkOut) {
      int32_t hdr[2] = {(int32_t)bi, (int32_t)totalFamilies};
      walkOut.write((const char*)hdr, 8);
      walkOut.write((const char*)walk.data(), (std::streamsize)(walk.size() * sizeof(WalkRec)));
    }
    if (sumsOut) {
      int32_t hdr[8] = {(int32_t)bi, S, R, (int32_t)srcCols, (int32_t)surfN, (int32_t)cutN, 0, 0};
      sumsOut.write((const char*)hdr, 32);
      sumsOut.write((const char*)bs.total.data(), (std::streamsize)bs.total.size() * 8);
      sumsOut.write((const char*)bs.rpE.data(), (std::streamsize)bs.rpE.size() * 8);
      sumsOut.write((const char*)bs.rpLf.data(), (std::streamsize)bs.rpLf.size() * 8);
      sumsOut.write((const char*)bs.rpLfc.data(), (std::streamsize)bs.rpLfc.size() * 8);
      sumsOut.write((const char*)bs.rpI.data(), (std::streamsize)bs.rpI.size() * 8);
      sumsOut.write((const char*)bs.rpSrc.data(), (std::streamsize)bs.rpSrc.size() * 8);
      sumsOut.write((const char*)bs.surf.data(), (std::streamsize)bs.surf.size() * 4);
      sumsOut.write((const char*)bs.cut.data(), (std::streamsize)bs.cut.size() * 4);
      uint64_t st[8] = {bs.states[0], bs.states[1], bs.states[2], bs.states[3], bs.states[4], bs.states[5], bs.childOverflow, 0};
      sumsOut.write((const char*)st, 64);
    }
    report.band(bs);
    bandsDone++;
  }
  report.finish();
  double wall = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
  // the run's own record beside the outputs: what traced it, with which seed (a run is replayable)
  {
    std::ofstream j(std::filesystem::u8path(cfg.wd + "spps-gpu.json"), std::ios::trunc);
    j << "{\n  \"solver\": \"spps-gpu\",\n  \"version\": \"" << VERSION << "\",\n  \"backend\": \"" << (cpu ? "cpu" : "gpu") << "\",\n";
    std::string d = cpu ? ("OpenMP, " + std::to_string(threads) + " threads") : device;
    std::string esc;
    for (char ch : d) { if (ch == '"' || ch == '\\') esc += '\\'; esc += ch; }
    j << "  \"device\": \"" << esc << "\",\n  \"seed\": " << seed << ",\n  \"seed_from_config\": " << (cfg.seed != 0 ? "true" : "false") << ",\n";
    j << "  \"rng\": \"philox4x32-10, key (seed, band << 16 | source), counter (particle, child, draw / 4, 0)\",\n";
    j << "  \"trace_seconds\": " << traceSeconds << ",\n  \"wall_seconds\": " << wall << ",\n";
    j << "  \"child_queue_overflow\": " << overflowTotal << "\n}\n";
  }
  return 0;
}
