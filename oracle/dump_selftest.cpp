// Prints the canonical spellings of fixed values; crates/simpa-core/tests/dump_helpers.rs expects
// the identical text from the Rust helpers. Ignores its file argument.
#include <cmath>
#include <cstdio>
#include "common.hpp"

int dump_selftest(const char*) {
    std::printf("selftest 1\n");
    std::printf("f32 %s %s %s %s\n", f32_hex(1.0f).c_str(), f32_hex(-0.0f).c_str(),
                f32_hex(0.1f).c_str(), f32_hex(3.4028235e38f).c_str());
    std::printf("f64 %s %s\n", f64_hex(1.0).c_str(), f64_hex(0.1).c_str());
    const char s[] = "Plane Receiver\\\xe9";
    std::printf("str %s %s\n", str_token(s, sizeof s - 1).c_str(), str_token("", 0).c_str());
    return 0;
}
