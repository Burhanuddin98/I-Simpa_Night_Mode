// Canonical dump of a .pbin through upstream's own particleio::ParticuleIO (docs/formats/pbin.md).
//
// Upstream's reader cannot fail on content: a read past the end leaves memset zeros (particle
// headers) or a zero vec3 and uninitialised energy (steps) and still reports success, and bytes
// after the last declared particle are never looked at. The oracle therefore tracks the offset
// upstream's reader has reached and prints "error" where upstream would read past the end of the
// file or leave bytes unread, before any value from such a read is printed. It also prints "error"
// where the Rust reader refuses a file upstream would read by a different rule, so no agreement
// is claimed there:
//  - a formatVersion other than 1, which upstream re-strides by the header's length fields
//    (part_io.cpp:245-248, 265-271);
//  - length fields other than upstream's own sizeof(binaryFHeader/binaryPTimeStep/binaryPHeader),
//    which every upstream writer stores (part_io.cpp:60-62, spps reportmanager.cpp:137-139).
//    After a particle with no steps, NextParticle's skip test `currentstep < nbtimestep-1`
//    underflows and it seeks to header + particleHeaderInfoLength (part_io.cpp:229-233), so any
//    other value there moves upstream's reader off the layout.
//
// formatVersion, fileInfoLength, particleInfoLength and particleHeaderInfoLength are held privately
// by ParticuleIO (formatVersion only as a bool), so they are read here into upstream's own
// binaryFHeader exactly as OpenForRead does (part_io.cpp:189-192), and the three fields
// ParticuleIO does expose are checked against that read.
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

#include "common.hpp"
#include "input_output/particles/part_binary.h"
#include "input_output/particles/part_io.hpp"

static_assert(sizeof(binaryFHeader) == 28, "pbin oracle assumes the Windows (LLP64) layout");
static_assert(sizeof(binaryPHeader) == 8, "pbin oracle assumes the Windows (LLP64) layout");
static_assert(sizeof(binaryPTimeStep) == 16, "pbin oracle assumes a float vec3");

static void appendf(std::string& out, const char* fmt, ...) {
    char b[256];
    va_list ap;
    va_start(ap, fmt);
    std::vsnprintf(b, sizeof b, fmt, ap);
    va_end(ap);
    out += b;
}

int dump_pbin(const char* path) {
    // The file's size, and its header read as upstream's OpenForRead reads it.
    FILE* f = std::fopen(path, "rb");
    if (!f) return oracle_fail();
    binaryFHeader raw;
    std::memset(&raw, 0, sizeof raw);
    const std::size_t got = std::fread(&raw, 1, sizeof raw, f);
    _fseeki64(f, 0, SEEK_END);
    const unsigned long long size = static_cast<unsigned long long>(_ftelli64(f));
    std::fclose(f);
    if (got != sizeof raw) return oracle_fail();  // upstream would hand out a partly zeroed header
    if (raw.formatVersion != PARTICLE_BINARY_VERSION_INFORMATION) return oracle_fail();
    if (raw.fileInfoLength != sizeof(binaryFHeader) ||
        raw.particleInfoLength != sizeof(binaryPTimeStep) ||
        raw.particleHeaderInfoLength != sizeof(binaryPHeader))
        return oracle_fail();

    particleio::ParticuleIO io;
    if (!io.OpenForRead(path)) return oracle_fail();
    float timeStep = 0.f;
    unsigned long nbParticles = 0, nbStepMax = 0;
    io.GetHeaderData(timeStep, nbParticles, nbStepMax);
    if (nbParticles != raw.nbParticles || nbStepMax != raw.nbTimeStepMax ||
        std::memcmp(&timeStep, &raw.timeStep, sizeof timeStep) != 0) {
        std::fprintf(stderr, "pbin oracle: ParticuleIO header differs from binaryFHeader read\n");
        return 2;
    }

    std::string out;
    appendf(out, "pbin %lu\n", raw.formatVersion);
    appendf(out, "header %lu %lu %lu %lu %lu %lu %s\n", raw.nbParticles, raw.formatVersion,
            raw.fileInfoLength, raw.particleInfoLength, raw.particleHeaderInfoLength,
            raw.nbTimeStepMax, f32_hex(raw.timeStep).c_str());
    appendf(out, "particles %lu\n", nbParticles);

    unsigned long long offset = sizeof(binaryFHeader);  // where upstream's stream now stands
    for (unsigned long i = 0; i < nbParticles; ++i) {
        if (size - offset < sizeof(binaryPHeader)) return oracle_fail();
        unsigned long firstTimeStep = 0, nbTimeStep = 0;
        io.NextParticle(firstTimeStep, nbTimeStep);
        offset += sizeof(binaryPHeader);
        if (static_cast<unsigned long long>(nbTimeStep) * sizeof(binaryPTimeStep) > size - offset)
            return oracle_fail();
        appendf(out, "particle %lu %lu\n", nbTimeStep, firstTimeStep);
        for (unsigned long s = 0; s < nbTimeStep; ++s) {
            float x = 0.f, y = 0.f, z = 0.f, energy = 0.f;
            io.NextTimeStep(x, y, z, energy);
            offset += sizeof(binaryPTimeStep);
            appendf(out, "step %s %s %s %s\n", f32_hex(x).c_str(), f32_hex(y).c_str(),
                    f32_hex(z).c_str(), f32_hex(energy).c_str());
        }
    }
    if (offset != size) return oracle_fail();  // bytes upstream's reader never reads
    std::fputs(out.c_str(), stdout);
    return 0;
}
