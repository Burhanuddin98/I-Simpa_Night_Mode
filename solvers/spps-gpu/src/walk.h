// spps-gpu: SPPS's particle walk, one source for the CPU (OpenMP) and the GPU (CUDA) builds.
//
// Every rule here is a port of upstream I-Simpa's SPPS (tag v1.4.0_snapshot_14_01_2026), receipts
// as paths under B:/repos/I-Simpa-upstream/src. The map is docs/investigations/2026-10-06-gpu/SPPS-LOOP.md
// and the rule table A2-SPEC.md. Types follow upstream: geometry in float (decimal), particle energy
// and point-receiver sums in double (l_decimal), surface and cutting-plane cells in float.
//
// Bit identity between the two builds: the walk uses only IEEE +,-,*,/ and sqrt (correctly rounded on
// both, with nvcc -fmad=false and MSVC's x64 SSE2 code, which has no FMA), and its own sin, cos, acos,
// log, exp and pow below, written with those operations only, so CPU and GPU compute the same bits.
#pragma once
#include <cstdint>
#include <cmath>

#if defined(__CUDACC__)
#define SPG_HD __host__ __device__
#define SPG_F __host__ __device__ __forceinline__
#else
#define SPG_HD
#define SPG_F inline
#endif

namespace spg {

// lib_interface/Core/mathlib.h:55-66 (decimal = float)
constexpr float EPS = 0.000001f;
constexpr float BEPS = 0.0001f;
constexpr float PIDIV2_F = 1.570796326794896619231321691639f;
constexpr float TWOPI_F = 6.283185307179586476925286766559f;

// spps/sppsTypes.h PARTICULE_STATE, same numbering
enum State : int { ALIVE = 0, LOST = 1, ABS_ATMO = 2, ABS_SURF = 3, LOOP = 4, ABS_ENC = 5 };

// ---------------------------------------------------------------------------------------------
// vec3 (mathlib.h base_vec3<float>), the operations SPPS uses, in its evaluation order

struct V3 { float x, y, z; };
SPG_F V3 mk(float x, float y, float z) { V3 r; r.x = x; r.y = y; r.z = z; return r; }
SPG_F V3 add(V3 a, V3 b) { return mk(a.x + b.x, a.y + b.y, a.z + b.z); }
SPG_F V3 sub(V3 a, V3 b) { return mk(a.x - b.x, a.y - b.y, a.z - b.z); }
SPG_F V3 neg(V3 a) { return mk(-a.x, -a.y, -a.z); }
SPG_F V3 mul(V3 a, float f) { return mk(a.x * f, a.y * f, a.z * f); }
// operator/(base_t): returns the vector unchanged when |f| < EPSILON, else multiplies by 1/f (mathlib.h:109-113)
SPG_F V3 divs(V3 a, float f) { if (fabsf(f) < EPS) return a; f = 1.0f / f; return mul(a, f); }
SPG_F float dot(V3 a, V3 b) { return ((a.x * b.x) + (a.y * b.y)) + (a.z * b.z); }
SPG_F float len(V3 a) { return sqrtf(a.x * a.x + a.y * a.y + a.z * a.z); }

// ---------------------------------------------------------------------------------------------
// Deterministic double-precision sin, cos, acos, log, exp, pow: only +,-,*,/, sqrt, floor, frexp,
// ldexp, all exact or correctly rounded on host and device. Accurate to a few ulp of double, then
// rounded to float where upstream works in float (std::cos(float) etc.).

SPG_F void d_sincos(double x, double* s, double* c) {
  const double INV_PIO2 = 6.36619772367581382433e-01;
  const double PIO2_1 = 1.57079632673412561417e+00;   // fdlibm pio2_1
  const double PIO2_1T = 6.07710050650619224932e-11;  // fdlibm pio2_1t
  double k = floor(x * INV_PIO2 + 0.5);
  double r = (x - k * PIO2_1) - k * PIO2_1T;
  double r2 = r * r;
  double sp = r * (1.0 + r2 * (-1.66666666666666666667e-01 + r2 * (8.33333333333333333333e-03 +
              r2 * (-1.98412698412698412698e-04 + r2 * (2.75573192239858906526e-06 + r2 * (-2.50521083854417187751e-08 +
              r2 * (1.60590438368216145994e-10 + r2 * (-7.64716373181981647590e-13 + r2 * 2.81145725434552076320e-15))))))));
  double cp = 1.0 + r2 * (-0.5 + r2 * (4.16666666666666666667e-02 + r2 * (-1.38888888888888888889e-03 +
              r2 * (2.48015873015873015873e-05 + r2 * (-2.75573192239858906526e-07 + r2 * (2.08767569878680989792e-09 +
              r2 * (-1.14707455977297247139e-11 + r2 * (4.77947733238738529744e-14 + r2 * -1.56192069685862264622e-16))))))));
  long long q = (long long)k;
  int quad = (int)(((q % 4) + 4) % 4);
  switch (quad) {
    case 0: *s = sp; *c = cp; break;
    case 1: *s = cp; *c = -sp; break;
    case 2: *s = -sp; *c = -cp; break;
    default: *s = -cp; *c = sp; break;
  }
}
SPG_F double d_sin(double x) { double s, c; d_sincos(x, &s, &c); return s; }
SPG_F double d_cos(double x) { double s, c; d_sincos(x, &s, &c); return c; }

// The series' coefficients as literals (no division in the loops: a double division costs dozens of
// instructions on a consumer GPU), evaluated by Horner's rule, the same operations on both builds.
#if defined(__CUDA_ARCH__)
#define SPG_UNROLL _Pragma("unroll")
#else
#define SPG_UNROLL
#endif
// asin by its series on |z| <= 0.5: sum over n of (2n)! / (4^n (n!)^2 (2n + 1)) z^(2n+1), to n = 30
SPG_F double d_asin_small(double z) {
  const double C[31] = {1.00000000000000000e+00, 1.66666666666666657e-01, 7.49999999999999972e-02, 4.46428571428571438e-02,
      3.03819444444444441e-02, 2.23721590909090919e-02, 1.73527644230769239e-02, 1.39648437500000007e-02, 1.15518008961397051e-02,
      9.76160952919407840e-03, 8.39033580961681506e-03, 7.31252587359884545e-03, 6.44721031188964875e-03, 5.74003767084192359e-03,
      5.15330968231990458e-03, 4.66014348691509619e-03, 4.24090709367936324e-03, 3.88096455883766905e-03, 3.56920539382593474e-03,
      3.29705950347348488e-03, 3.05782164925803065e-03, 2.84617840110894206e-03, 2.65787063820729008e-03, 2.48944867824688358e-03,
      2.33809189211197505e-03, 2.20147397371013836e-03, 2.07766103251816759e-03, 1.96503361627728369e-03, 1.86222640640312754e-03,
      1.76808112051541830e-03, 1.68160939358310679e-03};
  double z2 = z * z, p = C[30];
  SPG_UNROLL
  for (int n = 29; n >= 0; n--) p = p * z2 + C[n];
  return z * p;
}
// acos; NaN outside [-1, 1], as acosf returns there (upstream's angle() can feed it 1 + 1 ulp)
SPG_F double d_acos(double x) {
  const double PI = 3.14159265358979323846, PIO2 = 1.57079632679489661923;
  if (!(x >= -1.0 && x <= 1.0)) { double z = 0.0; return z / z; }
  if (x <= 0.5 && x >= -0.5) return PIO2 - d_asin_small(x);
  if (x > 0.5) return 2.0 * d_asin_small(sqrt((1.0 - x) * 0.5));
  return PI - 2.0 * d_asin_small(sqrt((1.0 + x) * 0.5));
}
SPG_F double d_log(double x) {
  const double LN2 = 6.93147180559945309417e-01;
  int e;
  double m = frexp(x, &e);
  if (m < 7.07106781186547524401e-01) { m *= 2.0; e -= 1; }
  // log m = 2 atanh z, z = (m - 1)/(m + 1), |z| <= 0.172: 2 z sum over n of z^(2n) / (2n + 1), to n = 14
  const double C[15] = {1.00000000000000000e+00, 3.33333333333333315e-01, 2.00000000000000011e-01, 1.42857142857142849e-01,
      1.11111111111111105e-01, 9.09090909090909116e-02, 7.69230769230769273e-02, 6.66666666666666657e-02, 5.88235294117647051e-02,
      5.26315789473684181e-02, 4.76190476190476164e-02, 4.34782608695652162e-02, 4.00000000000000008e-02, 3.70370370370370350e-02,
      3.44827586206896547e-02};
  double z = (m - 1.0) / (m + 1.0), z2 = z * z, p = C[14];
  SPG_UNROLL
  for (int n = 13; n >= 0; n--) p = p * z2 + C[n];
  return 2.0 * (z * p) + (double)e * LN2;
}
SPG_F double d_exp(double y) {
  const double INV_LN2 = 1.44269504088896338700e+00;
  const double LN2_HI = 6.93147180369123816490e-01, LN2_LO = 1.90821492927058770002e-10;
  double k = floor(y * INV_LN2 + 0.5);
  double r = (y - k * LN2_HI) - k * LN2_LO;
  // |r| <= ln2 / 2: sum over n of r^n / n!, to n = 20
  const double C[21] = {1.00000000000000000e+00, 1.00000000000000000e+00, 5.00000000000000000e-01, 1.66666666666666657e-01,
      4.16666666666666644e-02, 8.33333333333333322e-03, 1.38888888888888894e-03, 1.98412698412698413e-04, 2.48015873015873016e-05,
      2.75573192239858925e-06, 2.75573192239858883e-07, 2.50521083854417202e-08, 2.08767569878681002e-09, 1.60590438368216133e-10,
      1.14707455977297245e-11, 7.64716373181981641e-13, 4.77947733238738525e-14, 2.81145725434552060e-15, 1.56192069685862253e-16,
      8.22063524662432950e-18, 4.11031762331216484e-19};
  double p = C[20];
  SPG_UNROLL
  for (int n = 19; n >= 0; n--) p = p * r + C[n];
  return ldexp(p, (int)k);
}
SPG_F double d_pow(double a, double b) {
  if (a == 0.0) return 0.0;
  if (a < 0.0) { double z = 0.0; return z / z; }
  return d_exp(b * d_log(a));
}
SPG_F float f_cos(float x) { return (float)d_cos((double)x); }
SPG_F float f_sin(float x) { return (float)d_sin((double)x); }
SPG_F float f_acos(float x) { return (float)d_acos((double)x); }
// base_vec3::angle (mathlib.h:176-180)
SPG_F float angle(V3 a, V3 b) {
  float an = f_acos(dot(a, b) / (len(a) * len(b)));
  if (an < EPS) return 0;
  return an;
}
// base_vec3::Rotation (mathlib.h:220-230): rotates `self` about axis n by `an`
SPG_F V3 rotation(V3 self, V3 n, float an) {
  float m1 = f_cos(an);
  float m2 = 1 - m1;
  float m3 = f_sin(an);
  float m2nx = m2 * n.x;
  return mk((m1 + m2nx * n.x) * self.x + (m2nx * n.y - m3 * n.z) * self.y + (m2nx * n.z + m3 * n.y) * self.z,
            (m2nx * n.y + m3 * n.z) * self.x + (m1 + m2 * n.y * n.y) * self.y + (m2 * n.y * n.z - m3 * n.x) * self.z,
            (m2nx * n.z - m3 * n.y) * self.x + (m2 * n.y * n.z + m3 * n.x) * self.y + (m1 + m2 * n.z * n.z) * self.z);
}

// ---------------------------------------------------------------------------------------------
// Philox-4x32-10 (Salmon et al. 2011, Random123; the generator cuRAND implements), written out.
// key = (seed, band << 16 | source), counter = (particle, child, draw block, 0); one call gives four
// 32-bit draws, used in order, so the n-th draw of a particle's stream is lane n & 3 of block n >> 2.

SPG_F uint32_t mulhilo(uint32_t a, uint32_t b, uint32_t* hi) {
  uint64_t p = (uint64_t)a * (uint64_t)b;
  *hi = (uint32_t)(p >> 32);
  return (uint32_t)p;
}
SPG_F void philox4x32_10(uint32_t c0, uint32_t c1, uint32_t c2, uint32_t c3, uint32_t k0, uint32_t k1, uint32_t out[4]) {
  for (int r = 0; r < 10; r++) {
    if (r > 0) { k0 += 0x9E3779B9u; k1 += 0xBB67AE85u; }
    uint32_t hi0, hi1;
    uint32_t lo0 = mulhilo(0xD2511F53u, c0, &hi0);
    uint32_t lo1 = mulhilo(0xCD9E8D57u, c2, &hi1);
    uint32_t n0 = hi1 ^ c1 ^ k0, n2 = hi0 ^ c3 ^ k1;
    c0 = n0; c1 = lo1; c2 = n2; c3 = lo0;
  }
  out[0] = c0; out[1] = c1; out[2] = c2; out[3] = c3;
}

struct Rng {
  uint32_t k0, k1, part, child, n, blk;
  uint32_t b[4];
  SPG_F void init(uint32_t key0, uint32_t key1, uint32_t particle, uint32_t childId) {
    k0 = key0; k1 = key1; part = particle; child = childId; n = 0; blk = 0xFFFFFFFFu;
  }
  // a float in [0, 1), 24 bits (GetRandValue, sppsTypes.cpp:19-22, returns a float too)
  SPG_F float next() {
    uint32_t want = n >> 2;
    if (want != blk) { philox4x32_10(part, child, want, 0u, k0, k1, b); blk = want; }
    uint32_t lane = n & 3u, v;
    if (lane == 0) v = b[0]; else if (lane == 1) v = b[1]; else if (lane == 2) v = b[2]; else v = b[3];
    n++;
    return (float)(v >> 8) * (1.0f / 16777216.0f);
  }
};

// ---------------------------------------------------------------------------------------------
// The scene as the walk reads it (flat arrays, host or device pointers)

struct MatBand {      // t_Material_BFreq at the band (coreTypes.h), plus the material's side flag
  float absorption, diffusion, tau;
  int dotransmission, law, doubleSided;
};
struct CutPlane {     // r_SurfCut (coreTypes.h:263)
  V3 A, B, C, normal;
  int nu, nv;         // NbCellU (along BC), NbCellV (along BA)
  long long base;     // first cell's offset in the cut accumulator (cells x bins)
};
struct SrcBand {      // one source at the band, as runSourceCalculation sets up its particles
  V3 pos, dirUnit;    // position after TranslateSourceAtTetrahedronVertex; unidirection's Direction
  double energie, eps;
  float norm;         // GetNormVecPart: c dt
  int tetra, type, startStep, active;
};

// One tetrahedron face as the walk reads it, packed: its three corners (copies of the mesh nodes,
// bit for bit), its normal, the neighbour across it, its scene face and its surface-receiver face.
struct TFace {
  V3 p0, p1, p2, n;
  int nb, sf, rf, pad;
};

struct Scene {
  const TFace* tf;        // 4 per tetrahedron
  // tetrahedra, 4 faces each, index 4 t + f
  const V3* nodes;
  const int* fv;          // 3 node indices per tetra face
  const V3* fn;           // tetra face normal (FaceNormal of its 3 nodes, coreTypes.cpp:233)
  const int* nb;          // neighbour across the face, -1 none
  const int* sf;          // scene face (mbin marker), -1 none
  const int* rf;          // surface-receiver face (global index), -1 none
  const int* corner;      // 4 corners per tetra
  const int* rpOff;       // T + 1 offsets into rpList: the point receivers linked to the tetra
  const int* rpList;
  const int* cutOff;      // T + 1 offsets into cutList: the cutting planes crossing the tetra
  const int* cutList;
  // scene faces
  const V3* sn;           // FaceNormal
  const int* smat;        // material index
  const unsigned char* senc;  // faceEncombrement set
  const MatBand* mat;     // per material, at the band
  // point receivers
  const V3* rpPos;
  const V3* rpOri;
  int R;
  float radius;
  // cutting planes
  const CutPlane* cuts;
  int C;
  // run settings
  float dt;
  int nbSteps;
  float densite;          // densite_proba_absorption_atmospherique at the band (float)
  int absAtmo, energetic, directCalc, transCalc, surfMode, ratio, nbBins, nbSrc, bySource;
  int saveSurfHist, saveRpHist;
  uint32_t seed, band;
};

struct Particle {
  V3 pos, dir, colPos;
  double E, eps;
  float elapsed;
  int idface, tetra, step, state, src;
  int reflOrder, flag;
  Rng rng;
};

// ---------------------------------------------------------------------------------------------
// lib_interface/tools/collision.h:98-167, the non-culling branch, float throughout

SPG_F void CROSS3(float d[3], const float a[3], const float b[3]) {
  d[0] = a[1] * b[2] - a[2] * b[1];
  d[1] = a[2] * b[0] - a[0] * b[2];
  d[2] = a[0] * b[1] - a[1] * b[0];
}
SPG_F float DOT3(const float a[3], const float b[3]) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
SPG_F void SUB3(float d[3], const float a[3], const float b[3]) { d[0] = a[0] - b[0]; d[1] = a[1] - b[1]; d[2] = a[2] - b[2]; }

SPG_F int intersect_triangle(V3 o, V3 d, V3 v0, V3 v1, V3 v2, float* t, float* u, float* v) {
  float orig[3] = {o.x, o.y, o.z}, dir[3] = {d.x, d.y, d.z};
  float vert0[3] = {v0.x, v0.y, v0.z}, vert1[3] = {v1.x, v1.y, v1.z}, vert2[3] = {v2.x, v2.y, v2.z};
  float edge1[3], edge2[3], tvec[3], pvec[3], qvec[3], det, inv_det;
  SUB3(edge1, vert1, vert0);
  SUB3(edge2, vert2, vert0);
  CROSS3(pvec, dir, edge2);
  det = DOT3(edge1, pvec);
  if (det > -EPS && det < EPS) return 0;
  inv_det = 1.0f / det;
  SUB3(tvec, orig, vert0);
  *u = DOT3(tvec, pvec) * inv_det;
  if (*u < 0.0 || *u > 1.0) return 0;
  CROSS3(qvec, tvec, edge1);
  *v = DOT3(dir, qvec) * inv_det;
  if (*v < 0.0 || *u + *v > 1.0) return 0;
  *t = DOT3(edge2, qvec) * inv_det;
  return 1;
}
// collision.h:169-237, non-culling branch
SPG_F int intersect_parallelogram(V3 o, V3 d, V3 v0, V3 v1, V3 v2, float* t, float* u, float* v) {
  float orig[3] = {o.x, o.y, o.z}, dir[3] = {d.x, d.y, d.z};
  float vert0[3] = {v0.x, v0.y, v0.z}, vert1[3] = {v1.x, v1.y, v1.z}, vert2[3] = {v2.x, v2.y, v2.z};
  float edge1[3], edge2[3], tvec[3], pvec[3], qvec[3], det, inv_det;
  SUB3(edge1, vert1, vert0);
  SUB3(edge2, vert2, vert0);
  CROSS3(pvec, dir, edge2);
  det = DOT3(edge1, pvec);
  if (det > -EPS && det < EPS) return 0;
  inv_det = 1.0f / det;
  SUB3(tvec, orig, vert0);
  *u = DOT3(tvec, pvec) * inv_det;
  if (*u < 0.0f || *u >= 1.0f) return 0;
  CROSS3(qvec, tvec, edge1);
  *v = DOT3(dir, qvec) * inv_det;
  if (*v < 0.0f || *v >= 1.0f) return 0;
  *t = DOT3(edge2, qvec) * inv_det;
  return 1;
}
// mathlib.h:469 Determinant(vec3 x4) and :747 DotInTetra
SPG_F float det4(V3 vp1, V3 vp2, V3 vp3, V3 vp4) {
  return vp1.x * (vp2.y * (vp3.z - vp4.z) - vp3.y * (vp2.z - vp4.z) + vp4.y * (vp2.z - vp3.z)) -
         vp2.x * (vp1.y * (vp3.z - vp4.z) - vp3.y * (vp1.z - vp4.z) + vp4.y * (vp1.z - vp3.z)) +
         vp3.x * (vp1.y * (vp2.z - vp4.z) - vp2.y * (vp1.z - vp4.z) + vp4.y * (vp1.z - vp2.z)) -
         vp4.x * (vp1.y * (vp2.z - vp3.z) - vp2.y * (vp1.z - vp3.z) + vp3.y * (vp1.z - vp2.z));
}
SPG_F int fsign(float x) { return x > 0 ? 1 : -1; }
SPG_F bool dotInTetra(V3 p, V3 v1, V3 v2, V3 v3, V3 v4) {
  int sd0 = fsign(det4(v1, v2, v3, v4));
  if (fsign(det4(p, v2, v3, v4)) != sd0) return false;
  if (fsign(det4(v1, p, v3, v4)) != sd0) return false;
  if (fsign(det4(v1, v2, p, v4)) != sd0) return false;
  if (fsign(det4(v1, v2, v3, p)) != sd0) return false;
  return true;
}
// mathlib.h:184-195
SPG_F V3 closestPointOnSegment(V3 self, V3 vA, V3 vB) {
  V3 v = sub(vB, vA);
  float factor = dot(v, sub(self, vA)) / dot(v, v);
  if (factor <= 0.0f) return vA;
  if (factor >= 1.0f) return vB;
  return add(mul(sub(vB, vA), factor), vA);
}
// mathlib.h:201-209
SPG_F double distanceD(V3 a, V3 b) {
  double dx = a.x - b.x, dy = a.y - b.y, dz = a.z - b.z;
  return sqrt(dx * dx + dy * dy + dz * dz);
}
// spps/input_output/reportmanager.cpp:26-52
SPG_F bool raySphere(V3 p1, V3 p2, V3 sc, double r, double* mu1, double* mu2) {
  V3 dp = sub(p2, p1);
  double a = dp.x * dp.x + dp.y * dp.y + dp.z * dp.z;
  double b = 2 * (dp.x * (p1.x - sc.x) + dp.y * (p1.y - sc.y) + dp.z * (p1.z - sc.z));
  double c = sc.x * sc.x + sc.y * sc.y + sc.z * sc.z;
  c += p1.x * p1.x + p1.y * p1.y + p1.z * p1.z;
  c -= 2 * (sc.x * p1.x + sc.y * p1.y + sc.z * p1.z);
  c -= r * r;
  double bb4ac = b * b - 4 * a * c;
  if (fabs(a) < (double)EPS || bb4ac < 0) { *mu1 = 0; *mu2 = 0; return false; }
  *mu1 = (-b + sqrt(bb4ac)) / (2 * a);
  *mu2 = (-b - sqrt(bb4ac)) / (2 * a);
  return true;
}

// ---------------------------------------------------------------------------------------------
// The walk. Acc receives the sums, Rec the particle-file events (a no-op outside the host re-trace).

constexpr int QCAP = 16;   // transmitted children waiting per thread; overflow is counted, never silent

#if defined(__CUDACC__)
#pragma nv_exec_check_disable
#endif
template <class Acc, class Rec>
struct Walker {
  const Scene& s;
  Acc& acc;
  Rec& rec;
  Particle* q;          // the child queue, QCAP particles, storage owned by the caller
  int qh, qn;
  uint32_t nextChild;
  uint32_t partIndex;

  SPG_HD Walker(const Scene& sc, Acc& a, Rec& r, Particle* queue) : s(sc), acc(a), rec(r), q(queue), qh(0), qn(0), nextChild(1), partIndex(0) {}

  // CalculationCore.cpp:9-12 TetraFaceTest
  SPG_HD bool faceTest(int T, int f, V3 pos, V3 dir, float* t) {
    const TFace& F = s.tf[4 * T + f];
    if (dot(F.n, dir) < EPS) {
      float u, v;
      if (intersect_triangle(pos, dir, F.p0, F.p1, F.p2, t, &u, &v) == 1) return true;
    }
    return false;
  }
  // CalculationCore.cpp:413-462 GetTetraFaceCollision: the FIRST face hit in order 0..3
  SPG_HD int tetraFaceCollision(Particle& p, V3 dir, float* t) {
    for (int f = 0; f < 4; f++) if (faceTest(p.tetra, f, p.pos, dir, t)) return f;
    int old = p.tetra;
    for (int k = 0; k < 4; k++) {
      int nbt = s.tf[4 * old + k].nb;
      if (nbt >= 0) {
        p.tetra = nbt;
        const int* c = s.corner + 4 * nbt;
        if (dotInTetra(p.pos, s.nodes[c[0]], s.nodes[c[1]], s.nodes[c[2]], s.nodes[c[3]]))
          for (int f = 0; f < 4; f++) if (faceTest(nbt, f, p.pos, dir, t)) return f;
      }
    }
    p.E = 0;
    if (p.state == ALIVE) p.state = LOST;
    return -1;
  }
  // CalculationCore.cpp:551-558
  SPG_HD void setNextCollision(Particle& p) {
    float t = 0.0f;
    p.idface = tetraFaceCollision(p, p.dir, &t);
    p.colPos = add(p.pos, mul(p.dir, t));
  }
  // CalculationCore.cpp:559-566 FreeParticleTranslation, with reportmanager.cpp:169-247
  // ParticuleFreeTranslation: cutting planes and point receivers of the leg's START tetrahedron
  SPG_HD void freeTranslation(Particle& p, V3 tr) {
    V3 next = add(p.pos, tr);
    V3 direction = sub(next, p.pos);
    int T = p.tetra;
    for (int i = s.cutOff[T]; i < s.cutOff[T + 1]; i++) {
      const CutPlane& cp = s.cuts[s.cutList[i]];
      float t, u, v;
      if (intersect_parallelogram(p.pos, direction, cp.B, cp.C, cp.A, &t, &u, &v) && t >= 0 && t <= 1) {
        unsigned row = (unsigned)floorf(u * (float)cp.nu);
        unsigned col = (unsigned)floorf(v * (float)cp.nv);
        V3 normal = cp.normal;
        if (dot(p.dir, normal) < 0) normal = mul(normal, -1.0f);
        double val = p.E;
        if (s.surfMode != 0) { float cs = f_cos(angle(normal, p.dir)); val = p.E / (double)(cs > 0.000000001f ? cs : 0.000000001f); }
        int bin = p.step / s.ratio;   // patch 0001: the surface-receiver time bin
        acc.addCut(cp.base + ((long long)row * cp.nv + col) * (long long)s.nbBins + bin, val);
      }
    }
    for (int i = s.rpOff[T]; i < s.rpOff[T + 1]; i++) {
      int r = s.rpList[i];
      V3 rp = s.rpPos[r];
      V3 closest = closestPointOnSegment(rp, p.pos, next);
      if (distanceD(closest, rp) < (double)s.radius) {
        double mu1, mu2;
        if (raySphere(p.pos, next, rp, (double)s.radius, &mu1, &mu2)) {
          if (mu2 < 0) mu2 = 0; else if (mu2 > 1) mu2 = 1;
          if (mu1 < 0) mu1 = 0; else if (mu1 > 1) mu1 = 1;
          float norm_dir = len(direction);
          double L = fabs(mu2 - mu1) * norm_dir;
          if (L > 0) {
            float cosphi = f_cos(PIDIV2_F - angle(direction, s.rpOri[r]));
            double energy = p.E * L;
            V3 pi = mul(divs(p.dir, len(p.dir)), (float)energy);
            acc.addRp(r, p.step, p.src, energy, energy * ((double)cosphi * (double)cosphi), energy * (double)fabsf(cosphi),
                      (double)pi.x, (double)pi.y, (double)pi.z);
            if (p.flag) rec.rpHit(p, r, energy);
          }
        }
      }
    }
    p.pos = add(p.pos, tr);
  }
  // reportmanager.cpp:254-286 ParticuleCollideWithSceneMesh. Returns the scene normal as the
  // collision then reads it: on a surface-receiver face SPPS flips the shared normal IN PLACE to face
  // the particle (hazard 4); the flip is reproduced per hit, its persistence across particles is not.
  SPG_HD V3 collideScene(Particle& p) {
    const TFace& F = s.tf[4 * p.tetra + p.idface];
    int sfi = F.sf;
    if (sfi < 0) return mk(0, 0, 0);
    V3 n = s.sn[sfi];
    if (p.flag && s.saveSurfHist) { p.reflOrder++; rec.surfHit(p, n); }
    int rfi = F.rf;
    if (rfi >= 0) {
      if (dot(p.dir, n) < 0) n = mul(n, -1.0f);
      double val = p.E;
      if (s.surfMode != 0) { float cs = f_cos(angle(n, p.dir)); val = p.E / (double)(cs > 0.000000001f ? cs : 0.000000001f); }
      acc.addSurf(rfi, p.step / s.ratio, val);
    }
    return n;
  }
  // CalculationCore.cpp:357-387 TraverserTetra (fittings refused upstream of the walk)
  SPG_HD void traverse(Particle& p) {
    int nbt = s.tf[4 * p.tetra + p.idface].nb;
    if (nbt < 0) {
      p.E = 0;
      if (p.state == ALIVE) p.state = LOST;
      return;
    }
    p.tetra = nbt;
    setNextCollision(p);
  }
  // dotreflection.h:110-122
  SPG_HD V3 baseUniformReflection(V3 faceNormal, float theta, float phi) {
    V3 r;
    if (len(mk(faceNormal.x, faceNormal.y, 0)) > EPS) r = mk(faceNormal.y, -faceNormal.x, 0);
    else r = mk(1, 0, 0);
    r = rotation(faceNormal, r, phi);
    r = rotation(r, faceNormal, theta);
    return divs(r, len(r));
  }
  // dotreflection.h:53-56
  SPG_HD V3 specular(V3 d, V3 n) {
    V3 r = sub(d, mul(mul(n, dot(d, n)), 2.0f));
    return divs(r, len(r));
  }
  // dotreflection.h:59-64: theta drawn first, then phi
  SPG_HD V3 baseWn(Particle& p, V3 n, float expo) {
    float theta = p.rng.next() * TWOPI_F;
    float base = (float)1 - p.rng.next();
    float ex = (float)(1. / ((double)expo + 1.));
    float phi = f_acos((float)d_pow((double)base, (double)ex));
    return baseUniformReflection(n, theta, phi);
  }
  // dotreflection.h:21-46 SolveReflection
  SPG_HD V3 solveReflection(Particle& p, int law, V3 n) {
    switch (law) {
      case 0: return specular(p.dir, n);
      case 2: return baseWn(p, n, 1.0f);   // Lambert
      case 1: return baseWn(p, n, 0.0f);   // uniform
      case 3: return baseWn(p, n, 2.0f);
      case 4: return baseWn(p, n, 3.0f);
      case 5: return baseWn(p, n, 4.0f);
      default: return specular(p.dir, n);  // 6 (semi-diffuse) falls through to specular upstream
    }
  }
  SPG_HD void pushChild(const Particle& c) {
    if (qn >= QCAP) { acc.childOverflow(c.E); return; }
    q[(qh + qn) % QCAP] = c;
    qn++;
  }
  // CalculationCore.cpp:113-354 Movement
  SPG_HD void movement(Particle& p) {
    float deltaT = s.dt;
    float distanceSurLePas = len(p.dir);
    float celeriteLocal = distanceSurLePas / deltaT;
    bool collisionResolution = true;
    int iteration = 0;
    while (collisionResolution && p.state == ALIVE) {
      iteration++;
      collisionResolution = false;
      float distanceCollision = len(sub(p.colPos, p.pos));
      float distanceToTravel = celeriteLocal * (deltaT - p.elapsed);
      if (distanceCollision <= distanceToTravel) {
        V3 n = collideScene(p);
        V3 vecTranslation = sub(p.colPos, p.pos);
        p.elapsed += len(divs(vecTranslation, len(p.dir))) * deltaT;
        freeTranslation(p, vecTranslation);
        const TFace& F = s.tf[4 * p.tetra + p.idface];
        int sfi = F.sf;
        bool doInvertNormal = false;
        if (sfi >= 0) doInvertNormal = (dot(p.dir, n) <= -BEPS);
        if (sfi < 0 || ((s.senc[sfi] || (!s.mat[s.smat[sfi]].doubleSided && doInvertNormal)) && F.nb >= 0)) {
          traverse(p);
          collisionResolution = true;
        } else {
          const MatBand& m = s.mat[s.smat[sfi]];
          bool transmission = false;
          if (s.directCalc) {
            if (p.state == ALIVE) p.state = ABS_SURF;
            p.E = 0;
            return;
          }
          if (s.energetic) {
            if (m.absorption == 1) {
              if (!m.dotransmission || !s.transCalc) {
                if (p.state == ALIVE) p.state = ABS_SURF;
                p.E = 0;
                return;
              }
              transmission = true;
              p.E *= m.tau;
            } else if (m.absorption != 0) {
              if (m.dotransmission && m.tau != 0 && p.E * m.tau > p.eps && s.transCalc) {
                Particle c = p;
                c.E *= m.tau;
                c.rng.init(p.rng.k0, p.rng.k1, partIndex, nextChild++);
                traverse(c);
                if (c.E > c.eps) pushChild(c);
              }
              p.E *= (1 - m.absorption);
            }
          } else {
            if (p.rng.next() <= m.absorption) {
              if (s.transCalc && m.dotransmission && F.nb >= 0 && p.rng.next() * m.absorption <= m.tau) {
                transmission = true;
              } else {
                if (p.state == ALIVE) p.state = ABS_SURF;
                p.E = 0.;
                return;
              }
            }
          }
          if (p.E <= p.eps) {
            if (p.state == ALIVE) p.state = ABS_SURF;
            return;
          }
          if (transmission) {
            traverse(p);
            collisionResolution = true;
          } else {
            V3 nouv;
            if (m.diffusion == 1 || p.rng.next() < m.diffusion) {
              V3 fd = doInvertNormal ? n : neg(n);
              nouv = solveReflection(p, m.law, fd);
            } else {
              nouv = specular(p.dir, n);
            }
            p.dir = mul(nouv, distanceSurLePas);
            collisionResolution = true;
            setNextCollision(p);
          }
        }
      }
      if (iteration > 1000) {
        if (p.state == ALIVE) p.state = LOOP;
        p.E = 0;
        return;
      }
    }
    // A particle that died in the loop (LOST) has E = 0: SPPS still translates it, adding zeros only.
    if (p.state != ALIVE) return;
    if (p.elapsed == 0.f) {
      freeTranslation(p, p.dir);
    } else {
      freeTranslation(p, mul(p.dir, (deltaT - p.elapsed) / deltaT));
      p.elapsed = 0;
    }
  }
  // CalculationCore.cpp:41-111 Run, split so a run can stop between two steps and resume: start()
  // is its first collision search, advance() its step loop for at most `budget` steps. It returns true
  // when the run ended (the fate is then counted). Stopping between steps changes no arithmetic.
  SPG_HD void start(Particle& p) {
    rec.newParticle(p);
    setNextCollision(p);
  }
  SPG_HD bool advance(Particle& p, int& budget) {
    while (p.state == ALIVE && p.step < s.nbSteps) {
      if (budget <= 0) return false;
      budget--;
      if (s.absAtmo) {
        if (s.energetic) {
          p.E *= (double)s.densite;
          if (p.E <= p.eps) p.state = ABS_ATMO;
        } else {
          if (p.rng.next() >= s.densite) { p.E = 0; p.state = ABS_ATMO; }
        }
      }
      if (p.state == ALIVE) movement(p);
      if (p.state == ALIVE) {
        acc.addTotal(p.step, p.E);
        if (p.flag) rec.step(p);
        p.step++;
      }
    }
    acc.stat(p.state);
    rec.saveParticle();
    return true;
  }
  SPG_HD void run(Particle& p) {
    start(p);
    int big = 0x7FFFFFFF;
    advance(p, big);
  }
  SPG_HD bool popChild(Particle& c) {
    if (qn <= 0) return false;
    c = q[qh];
    qh = (qh + 1) % QCAP;
    qn--;
    return true;
  }
  // sppsNantes.cpp:97-140: a source's particle, its direction drawn (the family's child queue emptied)
  SPG_HD void initFamily(const SrcBand& sb, uint32_t idpart, int src, int flag, Particle& p) {
    p.pos = sb.pos;
    p.E = sb.energie;
    p.eps = sb.eps;
    p.elapsed = 0.f;
    p.tetra = sb.tetra;
    p.step = sb.startStep;
    p.state = ALIVE;
    p.src = src;
    p.reflOrder = 0;
    p.flag = flag;
    p.idface = -1;
    p.colPos = mk(0, 0, 0);
    p.rng.init(s.seed, (s.band << 16) | (uint32_t)src, idpart, 0u);
    partIndex = idpart;
    nextChild = 1;
    qh = 0; qn = 0;
    float n = sb.norm;
    // dotdistribution.cpp: phi then z for the sphere; one angle for the planes
    switch (sb.type) {
      case 1: p.dir = mul(sb.dirUnit, n); break;
      case 2: { float phi = p.rng.next() * TWOPI_F; p.dir = mk(n * f_cos(phi), n * f_sin(phi), 0); } break;
      case 3: { float th = p.rng.next() * TWOPI_F; p.dir = mk(0, n * f_cos(th), n * f_sin(th)); } break;
      case 4: { float th = p.rng.next() * TWOPI_F; p.dir = mk(n * f_cos(th), 0, n * f_sin(th)); } break;
      default: {
        float phi = p.rng.next() * TWOPI_F;
        float z = p.rng.next() * 2.0f - 1.0f;
        float costheta = sqrtf(1.0f - z * z);
        p.dir = mk(n * costheta * f_cos(phi), n * costheta * f_sin(phi), n * z);
      } break;
    }
  }
  // sppsNantes.cpp:97-155: one particle of a source, then its transmitted children, FIFO
  SPG_HD void family(const SrcBand& sb, uint32_t idpart, int src, int flag, uint32_t* outSteps, int* outState, double* outE, uint32_t* outChildren, uint64_t* outChildSteps) {
    Particle p;
    initFamily(sb, idpart, src, flag, p);
    int st0 = p.step;
    run(p);
    if (outSteps) { *outSteps = (uint32_t)(p.step - st0); *outState = p.state; *outE = p.E; }
    uint32_t nch = 0; uint64_t chs = 0;
    Particle c;
    while (popChild(c)) {
      int cs = c.step;
      run(c);
      nch++;
      chs += (uint64_t)(c.step - cs);
    }
    if (outChildren) { *outChildren = nch; *outChildSteps = chs; }
  }
};

struct NullRec {
  SPG_F void newParticle(const Particle&) {}
  SPG_F void saveParticle() {}
  SPG_F void step(const Particle&) {}
  SPG_F void surfHit(const Particle&, V3) {}
  SPG_F void rpHit(const Particle&, int, double) {}
};

}  // namespace spg
