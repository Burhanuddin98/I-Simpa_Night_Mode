// A1 (PLAN.md): the probe's tracer walking a tetrahedral mesh (prep_mesh.py's .walk), timed on the GPU
// and with the same function on every CPU thread. Energetic, specular, one band, per-face absorption,
// extinction at 1e-5, point receivers as spheres. Not SPPS's loop (that is A2); its cost.
// usage: mesh_walk <in.walk> <particles> <steps> <dt_s> <out.csv> [cpu_particles]
#include <cuda_runtime.h>
#include <omp.h>
#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cmath>
#include <chrono>
#include <vector>

#define MAX_RX 16
#define EXTINCTION 1e-5f
#define MAX_CROSSINGS 256   // faces crossed in one step before the particle is called lost

struct Mesh {
  uint32_t T, F, R; int32_t src_tet;
  float src[3], rx_r, c;
  const float4* planes;   // T*4
  const int32_t* neigh;   // T*4
  const int32_t* mark;    // T*4
  const float* alpha;     // F
  float rx[MAX_RX * 3];
};

__host__ __device__ inline uint32_t hash32(uint32_t x) {
  x ^= x >> 16; x *= 0x7feb352dU; x ^= x >> 15; x *= 0x846ca68bU; x ^= x >> 16; return x;
}
__host__ __device__ inline float unit01(uint32_t h) { return (h >> 8) * (1.0f / 16777216.0f); }

__host__ __device__ inline float sphere_len(const float p[3], const float d[3], float t, const float* c, float r) {
  float ox = p[0] - c[0], oy = p[1] - c[1], oz = p[2] - c[2];
  float b = ox * d[0] + oy * d[1] + oz * d[2];
  float cc = ox * ox + oy * oy + oz * oz - r * r;
  float disc = b * b - cc;
  if (disc <= 0.0f) return 0.0f;
  float s = sqrtf(disc), t0 = -b - s, t1 = -b + s;
  if (t0 < 0.0f) t0 = 0.0f;
  if (t1 > t) t1 = t;
  return t1 > t0 ? t1 - t0 : 0.0f;
}

// Returns steps traced; *lost set when the walk failed (no exit face, a hole, too many crossings).
template <typename Add>
__host__ __device__ inline int walk(uint32_t id, int steps, float dt, const Mesh& m, Add add, int* lost) {
  float u = unit01(hash32(id * 2u + 1u)) * 2.0f - 1.0f;
  float phi = unit01(hash32(id * 2u + 2u)) * 6.2831853f;
  float s = sqrtf(fmaxf(0.0f, 1.0f - u * u));
  float p[3] = {m.src[0], m.src[1], m.src[2]};
  float d[3] = {s * cosf(phi), s * sinf(phi), u};
  float e = 1.0f;
  int t = m.src_tet;
  for (int step = 0; step < steps; step++) {
    float remaining = m.c * dt;
    int crossings = 0;
    while (remaining > 0.0f) {
      float tmin = 3.4e38f; int face = -1;
      for (int f = 0; f < 4; f++) {
        float4 pl = m.planes[4 * t + f];
        float den = pl.x * d[0] + pl.y * d[1] + pl.z * d[2];
        if (den > 1e-12f) {
          float tt = (pl.w - (pl.x * p[0] + pl.y * p[1] + pl.z * p[2])) / den;
          if (tt < tmin) { tmin = tt; face = f; }
        }
      }
      if (face < 0) { *lost = 1; return step; }
      if (tmin < 0.0f) tmin = 0.0f;
      float go = tmin < remaining ? tmin : remaining;
      for (uint32_t r = 0; r < m.R; r++) {
        float l = sphere_len(p, d, go, m.rx + 3 * r, m.rx_r);
        if (l > 0.0f) add(step, (int)r, e * l);
      }
      p[0] += d[0] * go; p[1] += d[1] * go; p[2] += d[2] * go;
      remaining -= go;
      if (go < tmin) break;                       // the step ended inside the tetrahedron
      if (++crossings > MAX_CROSSINGS) { *lost = 1; return step; }
      int k = 4 * t + face;
      int sf = m.mark[k];
      if (sf >= 0) {                              // a scene face: specular reflection, absorption
        float4 pl = m.planes[k];
        float dn = d[0] * pl.x + d[1] * pl.y + d[2] * pl.z;
        d[0] -= 2.0f * dn * pl.x; d[1] -= 2.0f * dn * pl.y; d[2] -= 2.0f * dn * pl.z;
        e *= 1.0f - m.alpha[sf];
        if (e < EXTINCTION) return step + 1;
      } else if (m.neigh[k] >= 0) {
        t = m.neigh[k];
      } else { *lost = 1; return step; }
    }
  }
  return steps;
}

struct GpuAdd { float* hist; int steps; __device__ void operator()(int s, int r, float v) const { atomicAdd(&hist[r * steps + s], v); } };
struct CpuAdd { double* hist; int steps; void operator()(int s, int r, float v) const { hist[r * steps + s] += v; } };

__global__ void kernel(uint32_t n, int steps, float dt, Mesh m, float* hist, unsigned long long* done, unsigned int* lostc) {
  uint32_t id = blockIdx.x * blockDim.x + threadIdx.x;
  if (id >= n) return;
  int lost = 0;
  int k = walk(id, steps, dt, m, GpuAdd{hist, steps}, &lost);
  atomicAdd(done, (unsigned long long)k);
  if (lost) atomicAdd(lostc, 1u);
}

static void check(cudaError_t e, const char* w) { if (e != cudaSuccess) { fprintf(stderr, "CUDA %s: %s\n", w, cudaGetErrorString(e)); exit(2); } }

int main(int argc, char** argv) {
  if (argc < 6) { fprintf(stderr, "usage: mesh_walk <in.walk> <particles> <steps> <dt_s> <out.csv> [cpu_particles]\n"); return 2; }
  FILE* f = fopen(argv[1], "rb"); if (!f) { fprintf(stderr, "cannot read %s\n", argv[1]); return 2; }
  uint32_t n = (uint32_t)strtoull(argv[2], nullptr, 10);
  int steps = atoi(argv[3]); float dt = (float)atof(argv[4]);
  uint32_t ncpu = argc > 6 ? (uint32_t)strtoull(argv[6], nullptr, 10) : 0;
  Mesh m{};
  uint32_t hdr[3]; int32_t st; float fl[6];
  fread(hdr, 4, 3, f); fread(&st, 4, 1, f); fread(fl, 4, 6, f);
  m.T = hdr[0]; m.F = hdr[1]; m.R = hdr[2]; m.src_tet = st;
  m.src[0] = fl[0]; m.src[1] = fl[1]; m.src[2] = fl[2]; m.rx_r = fl[3]; m.c = fl[4];
  if (m.R > MAX_RX || m.src_tet < 0) { fprintf(stderr, "receivers %u, source tetra %d\n", m.R, m.src_tet); return 2; }
  std::vector<float4> planes(4 * (size_t)m.T); std::vector<int32_t> neigh(4 * (size_t)m.T), mark(4 * (size_t)m.T); std::vector<float> alpha(m.F);
  fread(planes.data(), 16, planes.size(), f); fread(neigh.data(), 4, neigh.size(), f); fread(mark.data(), 4, mark.size(), f);
  fread(alpha.data(), 4, alpha.size(), f); fread(m.rx, 4, 3 * m.R, f); fclose(f);

  // Device copies.
  Mesh dm = m;
  float4* d_pl; int32_t *d_ne, *d_ma; float* d_al;
  check(cudaMalloc(&d_pl, planes.size() * 16), "pl"); check(cudaMalloc(&d_ne, neigh.size() * 4), "ne");
  check(cudaMalloc(&d_ma, mark.size() * 4), "ma"); check(cudaMalloc(&d_al, alpha.size() * 4), "al");
  cudaMemcpy(d_pl, planes.data(), planes.size() * 16, cudaMemcpyHostToDevice);
  cudaMemcpy(d_ne, neigh.data(), neigh.size() * 4, cudaMemcpyHostToDevice);
  cudaMemcpy(d_ma, mark.data(), mark.size() * 4, cudaMemcpyHostToDevice);
  cudaMemcpy(d_al, alpha.data(), alpha.size() * 4, cudaMemcpyHostToDevice);
  dm.planes = d_pl; dm.neigh = d_ne; dm.mark = d_ma; dm.alpha = d_al;
  m.planes = planes.data(); m.neigh = neigh.data(); m.mark = mark.data(); m.alpha = alpha.data();

  size_t hn = (size_t)m.R * steps;
  float* d_hist; unsigned long long* d_done; unsigned int* d_lost;
  check(cudaMalloc(&d_hist, hn * 4), "hist"); check(cudaMalloc(&d_done, 8), "done"); check(cudaMalloc(&d_lost, 4), "lost");
  auto zero = [&] { cudaMemset(d_hist, 0, hn * 4); cudaMemset(d_done, 0, 8); cudaMemset(d_lost, 0, 4); };
  zero(); kernel<<<1, 32>>>(32, 4, dt, dm, d_hist, d_done, d_lost); check(cudaDeviceSynchronize(), "warm-up"); zero();

  cudaEvent_t e0, e1; cudaEventCreate(&e0); cudaEventCreate(&e1);
  cudaEventRecord(e0);
  kernel<<<(n + 255) / 256, 256>>>(n, steps, dt, dm, d_hist, d_done, d_lost);
  cudaEventRecord(e1); check(cudaEventSynchronize(e1), "kernel");
  float ms = 0; cudaEventElapsedTime(&ms, e0, e1);
  std::vector<float> hist(hn); unsigned long long done = 0; unsigned int lostn = 0;
  cudaMemcpy(hist.data(), d_hist, hn * 4, cudaMemcpyDeviceToHost); cudaMemcpy(&done, d_done, 8, cudaMemcpyDeviceToHost); cudaMemcpy(&lostn, d_lost, 4, cudaMemcpyDeviceToHost);

  // The same walk on every CPU thread, on the same first ncpu particles; and the GPU on those, compared.
  double cpu_s = 0, maxpeak = 0; unsigned long long cpu_done = 0; int threads = 0; unsigned int cpu_lost = 0;
  if (ncpu) {
    std::vector<double> total(hn, 0.0);
    auto c0 = std::chrono::steady_clock::now();
    #pragma omp parallel reduction(+:cpu_done, cpu_lost)
    {
      #pragma omp single
      threads = omp_get_num_threads();
      std::vector<double> mine(hn, 0.0);
      #pragma omp for schedule(dynamic, 512)
      for (long long id = 0; id < (long long)ncpu; id++) { int lost = 0; cpu_done += walk((uint32_t)id, steps, dt, m, CpuAdd{mine.data(), steps}, &lost); cpu_lost += lost; }
      #pragma omp critical
      for (size_t i = 0; i < hn; i++) total[i] += mine[i];
    }
    cpu_s = std::chrono::duration<double>(std::chrono::steady_clock::now() - c0).count();
    zero(); kernel<<<(ncpu + 255) / 256, 256>>>(ncpu, steps, dt, dm, d_hist, d_done, d_lost); check(cudaDeviceSynchronize(), "check");
    std::vector<float> g(hn); cudaMemcpy(g.data(), d_hist, hn * 4, cudaMemcpyDeviceToHost);
    double peak = 0; for (size_t i = 0; i < hn; i++) peak = fmax(peak, fabs(total[i]));
    for (size_t i = 0; i < hn; i++) maxpeak = fmax(maxpeak, fabs(total[i] - g[i]) / fmax(peak, 1e-30));
  }
  FILE* o = fopen(argv[5], "w");
  fprintf(o, "step,t_s"); for (uint32_t r = 0; r < m.R; r++) fprintf(o, ",rx%u", r); fprintf(o, "\n");
  for (int s2 = 0; s2 < steps; s2++) { fprintf(o, "%d,%.6f", s2, s2 * (double)dt); for (uint32_t r = 0; r < m.R; r++) fprintf(o, ",%.9g", hist[(size_t)r * steps + s2]); fprintf(o, "\n"); }
  fclose(o);
  cudaDeviceProp prop; cudaGetDeviceProperties(&prop, 0);
  double ks = ms / 1000.0;
  printf("{\"gpu\":\"%s\",\"tetra\":%u,\"particles\":%u,\"steps\":%d,\"dt_s\":%g,\"kernel_s\":%.4f,\"steps_traced\":%llu,\"lost\":%u,"
         "\"cpu_threads\":%d,\"cpu_particles\":%u,\"cpu_s\":%.4f,\"cpu_lost\":%u,\"gpu_s_per_particle\":%.4g,\"cpu_s_per_particle\":%.4g,"
         "\"gpu_over_cpu_all\":%.2f,\"cpu_gpu_max_of_peak\":%.3g}\n",
         prop.name, m.T, n, steps, (double)dt, ks, done, lostn, threads, ncpu, cpu_s, cpu_lost, ks / n, ncpu ? cpu_s / ncpu : 0.0,
         ncpu ? (cpu_s / ncpu) / (ks / n) : 0.0, maxpeak);
  return 0;
}
