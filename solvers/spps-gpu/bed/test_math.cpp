// spps-gpu bed: walk.h's own sin, cos, acos, log, exp and pow against the C library, on the ranges the
// walk feeds them, and Philox-4x32-10 against Random123's published known-answer vectors.
// Build: cl /O2 /EHsc /std:c++17 test_math.cpp (from a VS 2022 x64 prompt), run, read the max errors.
#include <cmath>
#include <cstdio>
#include "../src/walk.h"

using namespace spg;

int main() {
  double eS = 0, eC = 0, eA = 0, eL = 0, eE = 0, eP = 0;
  for (int i = 0; i <= 2000000; i++) {
    double x = -10.0 + 20.0 * i / 2000000.0;   // the walk's angles lie in [-2 pi, 2 pi]
    eS = fmax(eS, fabs(d_sin(x) - sin(x)));
    eC = fmax(eC, fabs(d_cos(x) - cos(x)));
    double a = -1.0 + 2.0 * i / 2000000.0;
    eA = fmax(eA, fabs(d_acos(a) - acos(a)));
    double l = 1e-8 + 2.0 * i / 2000000.0;
    eL = fmax(eL, fabs(d_log(l) - log(l)) / fmax(1.0, fabs(log(l))));
    double y = -30.0 + 60.0 * i / 2000000.0;
    eE = fmax(eE, fabs(d_exp(y) - exp(y)) / exp(y));
    double b = (double)i / 2000000.0;
    for (int k = 1; k <= 5; k++) eP = fmax(eP, fabs(d_pow(b, 1.0 / k) - pow(b, 1.0 / k)));
  }
  printf("max abs error: sin %.3g cos %.3g acos %.3g pow(x,1/k) %.3g; max rel error: log %.3g exp %.3g\n", eS, eC, eA, eP, eL, eE);
  printf("acos(1 + 2^-23) = %g (NaN expected), acos(1) = %g, acos(-1) = %.17g\n", d_acos(1.0 + 1.0 / 8388608.0), d_acos(1.0), d_acos(-1.0));
  // Random123 kat_vectors: philox4x32 10 rounds
  struct { uint32_t c[4], k[2], r[4]; } kat[3] = {
      {{0, 0, 0, 0}, {0, 0}, {0x6627e8d5, 0xe169c58d, 0xbc57ac4c, 0x9b00dbd8}},
      {{0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff}, {0xffffffff, 0xffffffff}, {0x408f276d, 0x41c83b0e, 0xa20bc7c6, 0x6d5451fd}},
      {{0x243f6a88, 0x85a308d3, 0x13198a2e, 0x03707344}, {0xa4093822, 0x299f31d0}, {0xd16cfe09, 0x94fdcceb, 0x5001e420, 0x24126ea1}}};
  int ok = 0;
  for (auto& t : kat) {
    uint32_t out[4];
    philox4x32_10(t.c[0], t.c[1], t.c[2], t.c[3], t.k[0], t.k[1], out);
    bool same = out[0] == t.r[0] && out[1] == t.r[1] && out[2] == t.r[2] && out[3] == t.r[3];
    ok += same;
    printf("philox4x32-10 KAT: %08x %08x %08x %08x %s\n", out[0], out[1], out[2], out[3], same ? "match" : "MISMATCH");
  }
  return ok == 3 ? 0 : 1;
}
