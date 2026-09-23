// Oracle for .mbin: upstream's formatMBIN::CMBIN::ImportBIN, the reader the solvers call
// (lib_interface/coreTypes.cpp:184), printing the canonical dump of docs/formats/mbin.md.
//
// One guard runs before upstream reads anything. ImportBIN never checks its stream
// (importExportMaillage/mbin.cpp:160-217): a file shorter than 8 bytes leaves the header counts
// uninitialised, a count larger than the file is passed straight to `new`, and a short body
// leaves fields unread. None of those is a result upstream can report, so the guard reports them
// as a failure: the file must hold at least 8 + 12*nodes + 100*tetrahedra bytes. Bytes after that
// are ignored, as upstream ignores them. Everything printed comes from ImportBIN's arrays.
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>

#include "common.hpp"
#include "input_output/importExportMaillage/mbin.h"

namespace {

// True when the file opens and is long enough for the counts in its own header.
bool mbin_size_guard(const char* path) {
    std::ifstream f(std::filesystem::u8path(path), std::ios::binary | std::ios::ate);
    if (!f.is_open()) return false;
    const std::streamoff size = f.tellg();
    if (size < 8) return false;
    f.seekg(0);
    unsigned char h[8];
    if (!f.read(reinterpret_cast<char*>(h), sizeof h)) return false;
    std::uint32_t quantTetra, quantNodes;  // t_FileHeader order: tetrahedra, then nodes
    std::memcpy(&quantTetra, h, 4);
    std::memcpy(&quantNodes, h + 4, 4);
    const std::uint64_t need = 8ull + 12ull * quantNodes + 100ull * quantTetra;
    return need <= static_cast<std::uint64_t>(size);
}

}  // namespace

int dump_mbin(const char* path) {
    using namespace formatMBIN;
    if (!mbin_size_guard(path)) return oracle_fail();

    bintetrahedre* tetra = nullptr;
    t_binNode* nodes = nullptr;
    unsigned int sizeTetra = 0;
    unsigned int sizeNodes = 0;
    CMBIN reader;
    if (!reader.ImportBIN(path, &tetra, &nodes, sizeTetra, sizeNodes)) {
        delete[] tetra;
        delete[] nodes;
        return oracle_fail();
    }

    std::printf("mbin\n");
    std::printf("nodes %u\n", sizeNodes);
    for (unsigned int i = 0; i < sizeNodes; ++i) {
        const t_binNode& n = nodes[i];
        std::printf("node %s %s %s\n", f32_hex(n.node[0]).c_str(), f32_hex(n.node[1]).c_str(),
                    f32_hex(n.node[2]).c_str());
    }
    std::printf("tetrahedra %u\n", sizeTetra);
    for (unsigned int i = 0; i < sizeTetra; ++i) {
        const bintetrahedre& t = tetra[i];
        std::printf("tetra %ld %ld %ld %ld %d", t.vertices[0], t.vertices[1], t.vertices[2],
                    t.vertices[3], static_cast<int>(t.idVolume));
        for (int f = 0; f < 4; ++f) {
            const bintetraface& face = t.tetrafaces[f];
            std::printf(" face %ld %ld %ld %d %d", face.vertices.a, face.vertices.b,
                        face.vertices.c, static_cast<int>(face.marker),
                        static_cast<int>(face.neighbor));
        }
        std::printf("\n");
    }
    delete[] tetra;
    delete[] nodes;
    return 0;
}
