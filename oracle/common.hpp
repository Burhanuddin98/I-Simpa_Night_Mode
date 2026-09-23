// Canonical-dump helpers for the oracle. Must spell values exactly as
// crates/simpa-core/src/formats/mod.rs does (f32_hex, f64_hex, str_token).
#pragma once
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

inline std::string f32_hex(float v) {
    std::uint32_t u;
    std::memcpy(&u, &v, sizeof u);
    char b[16];
    std::snprintf(b, sizeof b, "%08x", u);
    return b;
}

inline std::string f64_hex(double v) {
    std::uint64_t u;
    std::memcpy(&u, &v, sizeof u);
    char b[24];
    std::snprintf(b, sizeof b, "%016llx", static_cast<unsigned long long>(u));
    return b;
}

// Printable ASCII other than backslash stays; every other byte (space included) becomes \xHH.
inline std::string str_token(const char* s, std::size_t n) {
    std::string out;
    for (std::size_t i = 0; i < n; ++i) {
        unsigned char c = static_cast<unsigned char>(s[i]);
        if (c >= 0x21 && c <= 0x7e && c != '\\') {
            out.push_back(static_cast<char>(c));
        } else {
            char b[8];
            std::snprintf(b, sizeof b, "\\x%02x", c);
            out += b;
        }
    }
    return out.empty() ? std::string("\\x") : out;
}

inline std::string str_token(const std::string& s) { return str_token(s.data(), s.size()); }

// A dump function prints the canonical dump on stdout and returns 0, or prints "error" and
// returns 1 when upstream's reader reports failure.
inline int oracle_fail() {
    std::printf("error\n");
    return 1;
}
