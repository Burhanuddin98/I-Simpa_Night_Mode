// Oracle dumper for .csbin (surface receivers and cutting planes): upstream's own
// formatRSBIN::RSBIN::ImportBIN reads the file, and this prints the canonical dump of
// docs/formats/csbin.md from the t_ExchangeData it fills.
//
// ImportBIN checks nothing and always returns true: on a missing file it leaves the header
// uninitialised, and on a corrupt count it allocates whatever the count says and reads past the
// end of the file. So before calling it this dumper walks the file itself, with the struct sizes
// ImportBIN will use, and prints "error" for anything ImportBIN would misread. The values that
// are dumped all come from ImportBIN's t_ExchangeData, except the version and the five stored
// struct lengths, which ImportBIN reads into a local header and never exposes.
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

#include "common.hpp"
#include "input_output/exportRecepteurSurf/rsbin.h"

namespace {

bool slurp(const char* path, std::vector<unsigned char>& out) {
    FILE* f = std::fopen(path, "rb");
    if (!f) return false;
    unsigned char buf[65536];
    std::size_t n;
    while ((n = std::fread(buf, 1, sizeof buf, f)) > 0) out.insert(out.end(), buf, buf + n);
    bool ok = !std::ferror(f);
    std::fclose(f);
    return ok;
}

std::int32_t i32_at(const std::vector<unsigned char>& b, std::uint64_t at) {
    std::int32_t v;
    std::memcpy(&v, &b[static_cast<std::size_t>(at)], 4);
    return v;
}

std::uint32_t u32_at(const std::vector<unsigned char>& b, std::uint64_t at) {
    std::uint32_t v;
    std::memcpy(&v, &b[static_cast<std::size_t>(at)], 4);
    return v;
}

// The header ImportBIN reads (rsbin.cpp:39-52 defines it locally, so it is restated here).
const std::uint64_t HEADER_SIZE = 44;

// Walks the file with the struct sizes ImportBIN uses for a version-3 file (sizeof, not the
// stored lengths: rsbin.cpp:103,112-143 only honours those on a version conflict). True when
// every count ImportBIN will trust is non-negative and backed by bytes.
bool prescan(const std::vector<unsigned char>& b) {
    using namespace formatRSBIN;
    const std::uint64_t n = b.size();
    if (n < 4 || i32_at(b, 0) != static_cast<std::int32_t>(VERSION)) return false;
    if (n < HEADER_SIZE) return false;
    // The Rust reader steps by the stored lengths; ImportBIN steps by sizeof. They agree only
    // where the two are equal, which is every file upstream writes. Refuse the rest.
    const std::uint64_t native[5] = {HEADER_SIZE, sizeof(t_nodesPosition), sizeof(t_RecepteurS),
                                     sizeof(t_FaceRS), sizeof(t_faceValue)};
    for (int i = 0; i < 5; ++i)
        if (u32_at(b, 4 + 4 * i) != native[i]) return false;
    const std::int32_t quantNodes = i32_at(b, 24), quantRS = i32_at(b, 28);
    const std::int32_t nbTimeStep = i32_at(b, 32), recordType = i32_at(b, 40);
    if (quantNodes < 0 || quantRS < 0 || nbTimeStep < 0) return false;
    if (recordType < RECEPTEURS_RECORD_TYPE_SPL_STANDART || recordType > RECEPTEURS_RECORD_TYPE_STI)
        return false;
    std::uint64_t pos = HEADER_SIZE + static_cast<std::uint64_t>(quantNodes) * sizeof(t_nodesPosition);
    if (pos > n) return false;
    for (std::int32_t r = 0; r < quantRS; ++r) {
        if (pos + sizeof(t_RecepteurS) > n) return false;
        const std::int32_t quantFaces = i32_at(b, pos + offsetof(t_RecepteurS, quantFaces));
        pos += sizeof(t_RecepteurS);
        if (quantFaces < 0) return false;
        for (std::int32_t f = 0; f < quantFaces; ++f) {
            if (pos + sizeof(t_FaceRS) > n) return false;
            const std::int32_t nbRecords = i32_at(b, pos + offsetof(t_FaceRS, nbRecords));
            pos += sizeof(t_FaceRS);
            if (nbRecords < 0) return false;
            pos += static_cast<std::uint64_t>(nbRecords) * sizeof(t_faceValue);
            if (pos > n) return false;
        }
    }
    return true;
}

}  // namespace

int dump_csbin(const char* path) {
    using namespace formatRSBIN;
    std::vector<unsigned char> raw;
    if (!slurp(path, raw) || !prescan(raw)) return oracle_fail();

    t_ExchangeData d;
    RSBIN reader;
    if (!reader.ImportBIN(path, d)) return oracle_fail();

    // Range checks on ImportBIN's data, as the Rust reader makes them: upstream's consumers index
    // the node array and the time step arrays with these values unchecked
    // (isimpa/3dengine/Core/Recepteurs_surfacique.cpp:322-326, isimpa/data_manager/
    // projet_calculation.cpp:1036).
    for (int r = 0; r < d.tabRsSize; ++r) {
        const t_ExchangeData_Recepteurs& rs = d.tabRs[r];
        for (int f = 0; f < rs.dataRec.quantFaces; ++f) {
            const t_ExchangeData_Face& face = rs.dataFaces[f];
            for (int k = 0; k < 3; ++k) {
                const Intb v = face.dataFace.sommetsIndex[k];
                if (v < 0 || v >= d.tabNodesSize) return oracle_fail();
            }
            for (int t = 0; t < face.dataFace.nbRecords; ++t)
                if (static_cast<Intb>(face.tabTimeStep[t].timeStep) >= d.nbTimeStep) return oracle_fail();
        }
    }

    std::printf("csbin %d\n", i32_at(raw, 0));
    std::printf("lengths %u %u %u %u %u\n", u32_at(raw, 4), u32_at(raw, 8), u32_at(raw, 12),
                u32_at(raw, 16), u32_at(raw, 20));
    std::printf("timesteps %d %s\n", d.nbTimeStep, f32_hex(d.timeStep).c_str());
    std::printf("recordtype %d\n", static_cast<int>(d.recordType));
    std::printf("nodes %d\n", d.tabNodesSize);
    for (int i = 0; i < d.tabNodesSize; ++i) {
        const Floatb* p = d.tabNodes[i].node;
        std::printf("node %s %s %s\n", f32_hex(p[0]).c_str(), f32_hex(p[1]).c_str(), f32_hex(p[2]).c_str());
    }
    std::printf("receivers %d\n", d.tabRsSize);
    for (int r = 0; r < d.tabRsSize; ++r) {
        const t_ExchangeData_Recepteurs& rs = d.tabRs[r];
        const char* name = rs.dataRec.recepteurSName;
        std::size_t len = 0;
        while (len < STRING_SIZE && name[len] != 0) ++len;  // the name up to its first NUL
        std::printf("receiver %d %s\n", rs.dataRec.xmlIndex, str_token(name, len).c_str());
        std::printf("faces %d\n", rs.dataRec.quantFaces);
        for (int f = 0; f < rs.dataRec.quantFaces; ++f) {
            const t_ExchangeData_Face& face = rs.dataFaces[f];
            const Intb* v = face.dataFace.sommetsIndex;
            std::printf("face %d %d %d\n", v[0], v[1], v[2]);
            std::printf("records %d\n", face.dataFace.nbRecords);
            for (int t = 0; t < face.dataFace.nbRecords; ++t)
                std::printf("record %u %s\n", static_cast<unsigned>(face.tabTimeStep[t].timeStep),
                            f32_hex(face.tabTimeStep[t].energy).c_str());
        }
    }
    return 0;
}
