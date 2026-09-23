// Canonical dump of a TetGen .poly file through upstream's own reader,
// formatPOLY::CPoly::ImportPOLY (lib_interface/input_output/poly/poly.cpp), which build.ps1
// already compiles. Grammar: docs/formats/poly.md. Fields follow formatPOLY::t_model's order.
// ImportPOLY fails only when the file cannot be opened; every other defect is skipped silently,
// so this prints whatever the reader kept.
#include <cstdio>
#include <vector>

#include "common.hpp"
#include "input_output/poly/poly.h"

namespace {
void dump_faces(const char* name, const std::vector<formatPOLY::t_face>& faces) {
    std::printf("%s %zu\n", name, faces.size());
    for (const formatPOLY::t_face& f : faces) {
        std::printf("%ld %ld %ld %u\n", f.indicesSommets.a, f.indicesSommets.b, f.indicesSommets.c,
                    f.faceIndex);
    }
}
}  // namespace

int dump_poly(const char* path) {
    formatPOLY::t_model model;
    formatPOLY::CPoly reader;
    if (!reader.ImportPOLY(model, path)) return oracle_fail();
    std::printf("poly\n");
    std::printf("save_face_index %d\n", model.saveFaceIndex ? 1 : 0);
    dump_faces("user_faces", model.userDefinedFaces);
    dump_faces("faces", model.modelFaces);
    std::printf("vertices %zu\n", model.modelVertices.size());
    for (const auto& v : model.modelVertices) {
        std::printf("%s %s %s\n", f64_hex(v.x).c_str(), f64_hex(v.y).c_str(), f64_hex(v.z).c_str());
    }
    std::printf("regions %zu\n", model.modelRegions.size());
    for (const formatPOLY::t_region& r : model.modelRegions) {
        std::printf("%d %s %s %s %s\n", r.regionIndex, f32_hex(r.dotInRegion.x).c_str(),
                    f32_hex(r.dotInRegion.y).c_str(), f32_hex(r.dotInRegion.z).c_str(),
                    f32_hex(r.regionRefinement).c_str());
    }
    return 0;
}
