// Canonical dump of a .cbin scene file through upstream's own reader,
// formatCoreBIN::CformatBIN::ImportBIN (lib_interface/input_output/bin.cpp). The grammar is in
// docs/formats/cbin.md; crates/simpa-core/src/formats/cbin.rs prints the identical text.
//
// Upstream's reader exposes an ioModel {faces, vertices}. The dump prints the vertex list first,
// then the face list, the order they sit in a file written by ExportBIN, so face lines follow the
// vertices they index. Group names, node padding and firstSon are read or skipped by upstream but
// never stored in ioModel, so they are not dumped.
#include <cstdio>

#include "common.hpp"
#include "input_output/bin.h"

int dump_cbin(const char* path) {
    formatCoreBIN::ioModel model;
    formatCoreBIN::CformatBIN reader;
    if (!reader.ImportBIN(model, path)) return oracle_fail();
    // ImportBIN accepts only version 1.0 (bin.cpp:134-137), so the version tokens are constant.
    std::printf("cbin 1 0\n");
    std::printf("vertices %zu\n", model.vertices.size());
    for (const formatCoreBIN::t_pos& v : model.vertices) {
        std::printf("%s %s %s\n", f32_hex(v[0]).c_str(), f32_hex(v[1]).c_str(), f32_hex(v[2]).c_str());
    }
    std::printf("faces %zu\n", model.faces.size());
    for (const formatCoreBIN::ioFace& f : model.faces) {
        std::printf("%u %u %u %u %d %d\n", f.a, f.b, f.c, f.idMat, f.idRs, f.idEn);
    }
    return 0;
}
