// GPU probe (PLAN.md): a shoebox energetic particle tracer, one thread per particle, timed.
// Build: build.ps1 (nvcc, VS 2022 host compiler). Run: box_tracer <particles> <steps> <out.csv> [cpu_particles]
// Prints one JSON line with the rates; writes the receivers' per-step energy histograms to out.csv.
#include <cuda_runtime.h>
#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cmath>
#include <chrono>
#include <vector>
#include <string>
#include <omp.h>

// ---- the box (tests/fixtures/rooms/seats_box.simpa) ----------------------------------------------
#define LX 6.0f
#define LY 10.0f
#define LZ 3.0f
#define C_SOUND 343.0f
#define DT 0.001f
#define EXTINCTION 1e-5f
#define N_RX 2
__constant__ float c_alpha[6];        // -x, +x, -y, +y, -z, +z
__constant__ float c_src[3];
__constant__ float c_rx[N_RX * 3];
__constant__ float c_rx_r;
static const float h_alpha[6] = {0.2f, 0.2f, 0.2f, 0.2f, 0.1f, 0.3f};   // walls 20 %, floor 10 %, ceiling 30 %
static const float h_src[3] = {3.0f, 5.0f, 1.8f};
static const float h_rx[N_RX * 3] = {1.0f, 1.0f, 1.8f, 3.0f, 7.0f, 1.8f};
static const float h_rx_r = 0.31f;

// ---- a counter-based hash for the initial direction (no RNG state) -------------------------------
__host__ __device__ inline uint32_t hash32(uint32_t x) {
  x ^= x >> 16; x *= 0x7feb352dU; x ^= x >> 15; x *= 0x846ca68bU; x ^= x >> 16;
  return x;
}
__host__ __device__ inline float unit01(uint32_t h) { return (h >> 8) * (1.0f / 16777216.0f); }

// Length of the segment p -> p + d*t inside the sphere (centre c, radius r); d is a unit vector.
__host__ __device__ inline float sphere_len(const float p[3], const float d[3], float t, const float c[3], float r) {
  float ox = p[0] - c[0], oy = p[1] - c[1], oz = p[2] - c[2];
  float b = ox * d[0] + oy * d[1] + oz * d[2];
  float cc = ox * ox + oy * oy + oz * oz - r * r;
  float disc = b * b - cc;
  if (disc <= 0.0f) return 0.0f;
  float s = sqrtf(disc);
  float t0 = -b - s, t1 = -b + s;
  if (t0 < 0.0f) t0 = 0.0f;
  if (t1 > t) t1 = t;
  return t1 > t0 ? t1 - t0 : 0.0f;
}

// One particle's whole run. `add(step, rx, value)` accumulates the receiver histograms.
template <typename Add>
__host__ __device__ inline int trace_particle(uint32_t id, int steps, const float* alpha, const float* src, const float* rx, float rx_r, Add add) {
  // Uniform direction from two hashes.
  float u = unit01(hash32(id * 2u + 1u)) * 2.0f - 1.0f;
  float phi = unit01(hash32(id * 2u + 2u)) * 6.2831853f;
  float s = sqrtf(fmaxf(0.0f, 1.0f - u * u));
  float p[3] = {src[0], src[1], src[2]};
  float d[3] = {s * cosf(phi), s * sinf(phi), u};
  float e = 1.0f;
  const float len[3] = {LX, LY, LZ};
  for (int step = 0; step < steps; step++) {
    float remaining = C_SOUND * DT;
    while (remaining > 0.0f) {
      // Time (as length) to the nearest plane along d.
      float tmin = remaining;
      int face = -1;
      for (int k = 0; k < 3; k++) {
        if (d[k] > 1e-9f) {
          float t = (len[k] - p[k]) / d[k];
          if (t < tmin) { tmin = t; face = 2 * k + 1; }
        } else if (d[k] < -1e-9f) {
          float t = (0.0f - p[k]) / d[k];
          if (t < tmin) { tmin = t; face = 2 * k; }
        }
      }
      if (tmin < 0.0f) tmin = 0.0f;
      // The receivers see the segment.
      for (int r = 0; r < N_RX; r++) {
        float l = sphere_len(p, d, tmin, rx + 3 * r, rx_r);
        if (l > 0.0f) add(step, r, e * l);
      }
      p[0] += d[0] * tmin; p[1] += d[1] * tmin; p[2] += d[2] * tmin;
      remaining -= tmin;
      if (face >= 0) {
        int k = face >> 1;
        d[k] = -d[k];
        p[k] = (face & 1) ? len[k] - 1e-5f : 1e-5f;   // off the wall
        e *= (1.0f - alpha[face]);
        if (e < EXTINCTION) return step + 1;
      }
    }
  }
  return steps;
}

struct GpuAdd {
  float* hist; int steps;
  __device__ void operator()(int step, int r, float v) const { atomicAdd(&hist[r * steps + step], v); }
};
struct CpuAdd {
  double* hist; int steps;
  void operator()(int step, int r, float v) const { hist[r * steps + step] += v; }
};

__global__ void trace_kernel(uint32_t n, int steps, float* hist, unsigned long long* done) {
  uint32_t id = blockIdx.x * blockDim.x + threadIdx.x;
  if (id >= n) return;
  int k = trace_particle(id, steps, c_alpha, c_src, c_rx, c_rx_r, GpuAdd{hist, steps});
  atomicAdd(done, (unsigned long long)k);
}

static void check(cudaError_t e, const char* what) {
  if (e != cudaSuccess) { fprintf(stderr, "CUDA %s: %s\n", what, cudaGetErrorString(e)); exit(2); }
}

int main(int argc, char** argv) {
  if (argc < 4) { fprintf(stderr, "usage: box_tracer <particles> <steps> <out.csv> [cpu_particles]\n"); return 2; }
  uint32_t n = (uint32_t)strtoull(argv[1], nullptr, 10);
  int steps = atoi(argv[2]);
  const char* out = argv[3];
  uint32_t ncpu = argc > 4 ? (uint32_t)strtoull(argv[4], nullptr, 10) : 0;

  cudaDeviceProp prop; check(cudaGetDeviceProperties(&prop, 0), "props");
  check(cudaMemcpyToSymbol(c_alpha, h_alpha, sizeof h_alpha), "alpha");
  check(cudaMemcpyToSymbol(c_src, h_src, sizeof h_src), "src");
  check(cudaMemcpyToSymbol(c_rx, h_rx, sizeof h_rx), "rx");
  check(cudaMemcpyToSymbol(c_rx_r, &h_rx_r, sizeof h_rx_r), "rx_r");

  float* d_hist = nullptr;
  unsigned long long* d_done = nullptr;
  size_t hist_n = (size_t)N_RX * steps;
  check(cudaMalloc(&d_hist, hist_n * sizeof(float)), "malloc");
  check(cudaMalloc(&d_done, sizeof(unsigned long long)), "malloc done");
  check(cudaMemset(d_done, 0, sizeof(unsigned long long)), "memset done");
  check(cudaMemset(d_hist, 0, hist_n * sizeof(float)), "memset");
  // Warm-up (context, module load) so the timed kernel is the kernel.
  trace_kernel<<<1, 32>>>(32, 8, d_hist, d_done);
  check(cudaDeviceSynchronize(), "warm-up");
  check(cudaMemset(d_hist, 0, hist_n * sizeof(float)), "memset2");
  check(cudaMemset(d_done, 0, sizeof(unsigned long long)), "memset done2");

  auto w0 = std::chrono::steady_clock::now();
  cudaEvent_t e0, e1; cudaEventCreate(&e0); cudaEventCreate(&e1);
  int block = 256;
  uint32_t grid = (n + block - 1) / block;
  cudaEventRecord(e0);
  trace_kernel<<<grid, block>>>(n, steps, d_hist, d_done);
  cudaEventRecord(e1);
  check(cudaEventSynchronize(e1), "kernel");
  float kernel_ms = 0; cudaEventElapsedTime(&kernel_ms, e0, e1);
  std::vector<float> hist(hist_n);
  check(cudaMemcpy(hist.data(), d_hist, hist_n * sizeof(float), cudaMemcpyDeviceToHost), "copy");
  unsigned long long done = 0;
  check(cudaMemcpy(&done, d_done, sizeof done, cudaMemcpyDeviceToHost), "copy done");
  auto w1 = std::chrono::steady_clock::now();
  double wall_s = std::chrono::duration<double>(w1 - w0).count();

  // CPU cross-check on the first ncpu particles: the GPU's partial sums over the same particles
  // cannot be separated from the rest, so the CPU runs ncpu particles AND the GPU runs the same
  // ncpu into a second histogram; those two are compared.
  double cpu_s = 0; double cpu_maxrel = 0; double cpu_maxpeak = 0; uint32_t cpu_bins = 0; unsigned long long cpu_done = 0;
  if (ncpu > 0) {
    std::vector<double> ch(hist_n, 0.0);
    auto c0 = std::chrono::steady_clock::now();
    for (uint32_t id = 0; id < ncpu; id++) cpu_done += trace_particle(id, steps, h_alpha, h_src, h_rx, h_rx_r, CpuAdd{ch.data(), steps});
    auto c1 = std::chrono::steady_clock::now();
    cpu_s = std::chrono::duration<double>(c1 - c0).count();
    check(cudaMemset(d_hist, 0, hist_n * sizeof(float)), "memset3");
    trace_kernel<<<(ncpu + block - 1) / block, block>>>(ncpu, steps, d_hist, d_done);
    check(cudaDeviceSynchronize(), "kernel-cpu-check");
    std::vector<float> gh(hist_n);
    check(cudaMemcpy(gh.data(), d_hist, hist_n * sizeof(float), cudaMemcpyDeviceToHost), "copy2");
    double peak = 0;
    for (size_t i = 0; i < hist_n; i++) peak = fmax(peak, fabs(ch[i]));
    for (size_t i = 0; i < hist_n; i++) {
      double a = ch[i], b = gh[i];
      if (a == 0 && b == 0) continue;
      cpu_bins++;
      double rel = fabs(a - b) / fmax(fabs(a), 1e-30);
      if (rel > cpu_maxrel) cpu_maxrel = rel;
      double rp = fabs(a - b) / fmax(peak, 1e-30);
      if (rp > cpu_maxpeak) cpu_maxpeak = rp;
    }
  }

  // All CPU threads (OpenMP), the same function: argv[5] particles, if given.
  uint32_t npar = argc > 5 ? (uint32_t)strtoull(argv[5], nullptr, 10) : 0;
  double par_s = 0; unsigned long long par_done = 0; int par_threads = 0;
  if (npar > 0) {
    std::vector<double> total(hist_n, 0.0);
    auto p0 = std::chrono::steady_clock::now();
    #pragma omp parallel reduction(+:par_done)
    {
      #pragma omp single
      par_threads = omp_get_num_threads();
      std::vector<double> mine(hist_n, 0.0);
      #pragma omp for schedule(dynamic, 1024)
      for (long long id = 0; id < (long long)npar; id++)
        par_done += trace_particle((uint32_t)id, steps, h_alpha, h_src, h_rx, h_rx_r, CpuAdd{mine.data(), steps});
      #pragma omp critical
      for (size_t i = 0; i < hist_n; i++) total[i] += mine[i];
    }
    auto p1 = std::chrono::steady_clock::now();
    par_s = std::chrono::duration<double>(p1 - p0).count();
  }
  fprintf(stderr, "{\"cpu_all_threads\":%d,\"cpu_par_particles\":%u,\"cpu_par_s\":%.4f,\"cpu_par_traced\":%llu,\"rate_cpu_par\":%.4g}\n",
          par_threads, npar, par_s, par_done, npar ? (double)npar * steps / par_s : 0.0);

  FILE* f = fopen(out, "w");
  if (!f) { fprintf(stderr, "cannot write %s\n", out); return 2; }
  fprintf(f, "step,t_s");
  for (int r = 0; r < N_RX; r++) fprintf(f, ",rx%d", r);
  fprintf(f, "\n");
  for (int s = 0; s < steps; s++) {
    fprintf(f, "%d,%.6f", s, s * (double)DT);
    for (int r = 0; r < N_RX; r++) fprintf(f, ",%.9g", hist[(size_t)r * steps + s]);
    fprintf(f, "\n");
  }
  fclose(f);

  double ps = (double)n * steps;
  double ks = kernel_ms / 1000.0;
  printf("{\"gpu\":\"%s\",\"sm\":%d,\"particles\":%u,\"steps\":%d,\"dt_s\":%g,\"kernel_s\":%.4f,\"wall_s\":%.4f,"
         "\"particle_steps\":%.0f,\"steps_traced\":%llu,\"alive_fraction\":%.4f,\"rate_kernel\":%.4g,\"rate_kernel_traced\":%.4g,\"rate_wall\":%.4g,"
         "\"cpu_particles\":%u,\"cpu_s\":%.4f,\"rate_cpu\":%.4g,\"rate_cpu_traced\":%.4g,\"cpu_gpu_maxrel\":%.3g,\"cpu_gpu_max_of_peak\":%.3g,\"cpu_gpu_bins\":%u}\n",
         prop.name, prop.multiProcessorCount, n, steps, (double)DT, ks, wall_s, ps, done, done / ps, ps / ks, done / ks, ps / wall_s,
         ncpu, cpu_s, ncpu ? (double)ncpu * steps / cpu_s : 0.0, ncpu ? cpu_done / cpu_s : 0.0, cpu_maxrel, cpu_maxpeak, cpu_bins);
  cudaFree(d_hist);
  return 0;
}
