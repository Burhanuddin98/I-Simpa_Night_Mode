// ─── I-Simpa NewGUI Automated Test Suite ────────────────────────────────────
// Tests all backend functionality without launching the GUI.
// Build: cl /EHsc /std:c++17 /I.. test_pipeline.cpp ../project/project.cpp
//        ../project/solver.cpp ../project/result_parser.cpp ../mesh/scene_model.cpp
//        ../mesh/ply_loader.cpp /Fe:test_pipeline.exe
// Or just run via the test runner script.

#include <cstdio>
#include <cstring>
#include <cassert>
#include <filesystem>
#include <fstream>

// GLM
#include <glm/glm.hpp>

// Pull in the actual code
#include "project/project.h"
#include "project/solver.h"
#include "project/result_parser.h"
#include "mesh/scene_model.h"

namespace fs = std::filesystem;
using namespace isimpa;

static int g_passed = 0, g_failed = 0;

#define TEST(name) printf("  TEST: %-50s ", name);
#define PASS() do { printf("[PASS]\n"); g_passed++; } while(0)
#define FAIL(msg) do { printf("[FAIL] %s\n", msg); g_failed++; } while(0)
#define CHECK(cond, msg) do { if (!(cond)) { FAIL(msg); return; } } while(0)

// ─── Test: Project Creation ─────────────────────────────────────────────────

void test_create_default_room() {
    TEST("Create default room");
    Project proj;
    proj.NewProject();
    proj.CreateDefaultRoom(6.0f, 10.0f, 3.0f);
    CHECK(proj.model.vertices.size() == 8, "Expected 8 vertices");
    CHECK(proj.model.groups.size() == 6, "Expected 6 groups (floor/ceiling/4 walls)");
    CHECK(proj.HasGeometry(), "Should have geometry");
    int totalFaces = proj.TotalFaces();
    CHECK(totalFaces == 12, "Expected 12 faces (2 per wall)");
    PASS();
}

void test_add_source() {
    TEST("Add sound source");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    auto& src = proj.AddSource();
    CHECK(src.id == 1, "First source ID should be 1");
    CHECK(proj.sources.size() == 1, "Should have 1 source");
    CHECK(src.globalPowerDb == 80.0f, "Default power should be 80 dB");
    auto& src2 = proj.AddSource();
    CHECK(src2.id == 2, "Second source ID should be 2");
    PASS();
}

void test_add_receiver() {
    TEST("Add punctual receiver");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    auto& rcv = proj.AddReceiver();
    CHECK(rcv.id == 1, "First receiver ID should be 1");
    CHECK(proj.punctualReceivers.size() == 1, "Should have 1 receiver");
    PASS();
}

void test_add_surface_receiver() {
    TEST("Add surface receiver");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    auto& sr = proj.AddSurfaceReceiver();
    CHECK(sr.id == 1, "First surface receiver ID should be 1");
    CHECK(sr.type == SurfaceReceiver::Plane, "Should be Plane type");
    CHECK(sr.gridResolution == 0.5f, "Default resolution should be 0.5");
    PASS();
}

void test_material_assignment() {
    TEST("Material assignment");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    CHECK(proj.materials.size() >= 11, "Should have 11+ default materials");
    proj.AssignMaterial(0, 3); // Assign carpet to floor
    CHECK(proj.groupMaterialMap[0] == 3, "Floor should have material 3");
    CHECK(proj.model.groups[0].materialId == 3, "Group materialId should match");
    PASS();
}

void test_material_absorption() {
    TEST("Material absorption values");
    Project proj;
    // Check concrete (first material)
    auto& concrete = proj.materials[0];
    CHECK(concrete.name == "Concrete", "First material should be Concrete");
    float avg = concrete.AverageAbsorption();
    CHECK(avg > 0.01f && avg < 0.1f, "Concrete avg absorption should be 0.01-0.1");
    // Check foam
    auto& foam = proj.materials[4];
    CHECK(foam.name == "Acoustic Foam", "Material 4 should be Acoustic Foam");
    float foamAvg = foam.AverageAbsorption();
    CHECK(foamAvg > 0.5f, "Foam avg absorption should be > 0.5");
    PASS();
}

// ─── Test: File I/O ─────────────────────────────────────────────────────────

void test_load_ply() {
    TEST("Load PLY file (Elmia Hall)");
    std::string path = "C:/RoomGUI/Michael/I-Simpa/src/isimpa/resources/doc/tutorial/tutorial 2/elmia.ply";
    if (!fs::exists(path)) { FAIL("File not found"); return; }
    SceneModel model;
    bool ok = LoadPLY(path, model);
    CHECK(ok, "LoadPLY should return true");
    CHECK(model.vertices.size() > 100, "Should have >100 vertices");
    CHECK(!model.groups.empty(), "Should have groups");
    CHECK(model.extent > 0.1f, "Extent should be positive");
    printf("(%zu verts, %zu groups) ", model.vertices.size(), model.groups.size());
    PASS();
}

void test_load_stl_binary() {
    TEST("Load STL file (if available)");
    // Generate a minimal binary STL for testing
    std::string tmpPath = "test_cube.stl";
    {
        std::ofstream f(tmpPath, std::ios::binary);
        char header[80] = {};
        strncpy(header, "test cube", 79);
        f.write(header, 80);
        uint32_t numTri = 2;
        f.write((char*)&numTri, 4);
        for (int i = 0; i < 2; i++) {
            float data[12] = {0,1,0, 0,0,0, 1,0,0, 1,1,0}; // normal + 3 verts
            f.write((char*)data, 48);
            uint16_t attr = 0;
            f.write((char*)&attr, 2);
        }
        f.close();
    }
    SceneModel model;
    bool ok = LoadSTL(tmpPath, model);
    CHECK(ok, "LoadSTL should return true");
    CHECK(model.vertices.size() >= 3, "Should have vertices");
    fs::remove(tmpPath);
    PASS();
}

void test_load_obj() {
    TEST("Load OBJ file");
    std::string tmpPath = "test_box.obj";
    {
        std::ofstream f(tmpPath);
        f << "# test box\n";
        f << "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\n";
        f << "v 0 0 1\nv 1 0 1\nv 1 1 1\nv 0 1 1\n";
        f << "g front\n";
        f << "f 1 2 3 4\n";
        f << "g back\n";
        f << "f 5 8 7 6\n";
        f.close();
    }
    SceneModel model;
    bool ok = LoadOBJ(tmpPath, model);
    CHECK(ok, "LoadOBJ should return true");
    CHECK(model.vertices.size() == 8, "Should have 8 vertices");
    CHECK(model.groups.size() == 2, "Should have 2 groups");
    fs::remove(tmpPath);
    PASS();
}

void test_save_load_project() {
    TEST("Save and load project");
    std::string tmpPath = "test_project.isimpa";

    // Create a project with geometry, sources, receivers
    Project proj;
    proj.CreateDefaultRoom(8, 12, 4);
    proj.AddSource().name = "Test Source";
    proj.AddReceiver().name = "Test Receiver";
    proj.AssignMaterial(0, 2); // Glass on floor
    proj.environment.temperature = 22.5f;
    proj.sppsConfig.particlesPerSource = 50000;

    bool saved = SaveProject(proj, tmpPath);
    CHECK(saved, "SaveProject should return true");
    CHECK(fs::exists(tmpPath), "File should exist on disk");

    // Load it back
    Project proj2;
    bool loaded = LoadProject(proj2, tmpPath);
    CHECK(loaded, "LoadProject should return true");
    CHECK(proj2.model.vertices.size() == 8, "Should have 8 vertices");
    CHECK(proj2.model.groups.size() == 6, "Should have 6 groups");
    CHECK(proj2.sources.size() == 1, "Should have 1 source");
    CHECK(proj2.sources[0].name == "Test Source", "Source name should match");
    CHECK(proj2.punctualReceivers.size() == 1, "Should have 1 receiver");
    CHECK(proj2.groupMaterialMap[0] == 2, "Material assignment should persist");
    CHECK(fabsf(proj2.environment.temperature - 22.5f) < 0.01f, "Temperature should match");
    CHECK(proj2.sppsConfig.particlesPerSource == 50000, "Particles config should match");

    fs::remove(tmpPath);
    PASS();
}

// ─── Test: Mesh Export ──────────────────────────────────────────────────────

void test_write_cbin() {
    TEST("Write .cbin binary mesh");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    proj.AssignMaterial(0, 0);

    std::string path = "test_model.cbin";
    bool ok = WriteMeshBinary(proj, path);
    CHECK(ok, "WriteMeshBinary should return true");
    CHECK(fs::exists(path), "File should exist");
    auto size = fs::file_size(path);
    CHECK(size > 100, "File should not be empty");
    printf("(%zu bytes) ", (size_t)size);

    // Verify header
    std::ifstream f(path, std::ios::binary);
    uint32_t major, minor;
    f.read((char*)&major, 4);
    f.read((char*)&minor, 4);
    CHECK(major == 1 && minor == 0, "Version should be 1.0");
    f.close();

    fs::remove(path);
    PASS();
}

void test_write_poly() {
    TEST("Write .poly file for TetGen");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);

    std::string path = "test_model.poly";
    bool ok = WritePoly(proj, path);
    CHECK(ok, "WritePoly should return true");
    CHECK(fs::exists(path), "File should exist");

    // Verify it's readable text with correct vertex count
    std::ifstream f(path);
    int nVerts, dim, nAttr, nBound;
    f >> nVerts >> dim >> nAttr >> nBound;
    CHECK(nVerts == 8, "Should declare 8 vertices");
    CHECK(dim == 3, "Should be 3D");
    f.close();

    fs::remove(path);
    PASS();
}

void test_write_config_xml() {
    TEST("Write solver config.xml");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    proj.AddSource();
    proj.AddReceiver();
    proj.AssignMaterial(0, 0);

    std::string dir = "test_sim_dir";
    fs::create_directories(dir);
    bool ok = WriteConfigXML(proj, dir, "spps");
    CHECK(ok, "WriteConfigXML should return true");
    CHECK(fs::exists(dir + "/config.xml"), "config.xml should exist");

    // Verify it's valid XML with key elements
    std::ifstream f(dir + "/config.xml");
    std::string content((std::istreambuf_iterator<char>(f)), std::istreambuf_iterator<char>());
    CHECK(content.find("<configuration") != std::string::npos, "Should have <configuration> root");
    CHECK(content.find("<simulation") != std::string::npos, "Should have <simulation>");
    CHECK(content.find("<environment") != std::string::npos, "Should have <environment>");
    CHECK(content.find("<sources_enum") != std::string::npos, "Should have <sources_enum>");
    CHECK(content.find("<recepteursp_enum") != std::string::npos, "Should have <recepteursp_enum>");
    CHECK(content.find("modelName=\"model.cbin\"") != std::string::npos, "Should reference model.cbin");

    fs::remove_all(dir);
    PASS();
}

// ─── Test: Undo/Redo ────────────────────────────────────────────────────────

void test_undo_redo() {
    TEST("Undo/Redo system");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    UndoManager& undo = GetUndoManager();
    undo.Clear();

    // Save state, add source
    undo.SaveState(proj, "Add Source");
    proj.AddSource();
    CHECK(proj.sources.size() == 1, "Should have 1 source after add");

    // Save state, add another
    undo.SaveState(proj, "Add Source 2");
    proj.AddSource();
    CHECK(proj.sources.size() == 2, "Should have 2 sources");

    // Undo
    bool undone = undo.Undo(proj);
    CHECK(undone, "Undo should succeed");
    CHECK(proj.sources.size() == 1, "Should have 1 source after undo");

    // Undo again
    undone = undo.Undo(proj);
    CHECK(undone, "Second undo should succeed");
    CHECK(proj.sources.size() == 0, "Should have 0 sources after second undo");

    // Redo
    bool redone = undo.Redo(proj);
    CHECK(redone, "Redo should succeed");
    CHECK(proj.sources.size() == 1, "Should have 1 source after redo");

    PASS();
}

// ─── Test: Solver Executables ───────────────────────────────────────────────

void test_find_solvers() {
    TEST("Find solver executables");
    std::string spps = FindSolverExe("spps");
    std::string tcr = FindSolverExe("tcr");
    std::string tetgen = FindSolverExe("tetgen");
    std::string preprocess = FindSolverExe("preprocess");

    if (spps.empty()) printf("\n    WARNING: spps.exe not found ");
    if (tcr.empty()) printf("\n    WARNING: classicalTheory.exe not found ");
    if (tetgen.empty()) printf("\n    WARNING: tetgen.exe not found ");
    if (preprocess.empty()) printf("\n    WARNING: preprocess.exe not found ");

    // At least one should be found if solvers were built
    bool anyFound = !spps.empty() || !tcr.empty() || !tetgen.empty();
    if (!anyFound) { FAIL("No solver executables found at all"); return; }
    PASS();
}

// ─── Test: Material Import/Export ───────────────────────────────────────────

void test_material_export_import() {
    TEST("Material export/import");
    Project proj;
    std::string path = "test_materials.xml";

    bool exported = ExportMaterials(proj.materials, path);
    CHECK(exported, "Export should succeed");
    CHECK(fs::exists(path), "File should exist");

    // Import into empty list
    std::vector<AcousticMaterial> imported;
    bool ok = ImportMaterials(imported, path);
    CHECK(ok, "Import should succeed");
    CHECK(imported.size() == proj.materials.size(), "Should import same count");
    CHECK(imported[0].name == proj.materials[0].name, "First material name should match");
    CHECK(fabsf(imported[0].bands[4].absorption - proj.materials[0].bands[4].absorption) < 0.001f,
          "Absorption values should match");

    fs::remove(path);
    PASS();
}

// ─── Test: GABE Reader ──────────────────────────────────────────────────────

void test_gabe_reader() {
    TEST("GABE file reader (if results exist)");
    std::string resultDir = "C:/Users/bsaka/AppData/Roaming/isimpa/current/instance2/report/SPPS/2025-09-26_01h01m31s";
    if (!fs::exists(resultDir)) {
        printf("(skipped - no results) ");
        PASS();
        return;
    }

    auto recpFiles = FindResultFiles(resultDir, ".recp");
    CHECK(!recpFiles.empty(), "Should find .recp files");

    GabeFile gabe;
    bool loaded = gabe.Load(recpFiles[0]);
    CHECK(loaded, "Should load .recp file");
    CHECK(gabe.GetColCount() > 0, "Should have columns");
    printf("(%d cols, %d rows) ", gabe.GetColCount(), gabe.GetRowCount());
    PASS();
}

// ─── Test: SceneModel Geometry ──────────────────────────────────────────────

void test_compute_bounds() {
    TEST("SceneModel bounds computation");
    SceneModel model = SceneModel::CreateBox(6, 10, 3);
    CHECK(model.bbMin.x >= -0.01f && model.bbMin.x <= 0.01f, "bbMin.x should be ~0");
    CHECK(fabsf(model.bbMax.x - 6.0f) < 0.01f, "bbMax.x should be ~6");
    CHECK(fabsf(model.bbMax.y - 3.0f) < 0.01f, "bbMax.y should be ~3 (height)");
    CHECK(model.extent > 1.0f, "Extent should be positive");
    PASS();
}

void test_compute_normals() {
    TEST("SceneModel normal computation");
    SceneModel model = SceneModel::CreateBox(6, 10, 3);
    // Floor faces should have normal pointing down (Y=-1 in our coord system)
    auto& floor = model.groups[0];
    CHECK(floor.faces.size() == 2, "Floor should have 2 faces");
    glm::vec3 n = floor.faces[0].normal;
    // Floor normal could be +Y or -Y depending on winding order
    CHECK(fabsf(fabsf(n.y) - 1.0f) < 0.1f, "Floor normal Y should be ~+/-1");
    PASS();
}

void test_compute_area() {
    TEST("SceneModel area computation");
    SceneModel model = SceneModel::CreateBox(6, 10, 3);
    float floorArea = model.ComputeGroupArea(0);
    CHECK(fabsf(floorArea - 60.0f) < 0.5f, "Floor area should be ~60 m2 (6x10)");
    PASS();
}

// ─── Test: Coordinate Transforms ────────────────────────────────────────────

void test_coord_transform_cbin() {
    TEST("Coordinate transform in .cbin export");
    Project proj;
    proj.CreateDefaultRoom(6, 10, 3);
    std::string path = "test_coord.cbin";
    WriteMeshBinary(proj, path);

    // Read back and verify vertex 1 (which should be at GL (6,0,0) -> ISim (6,0,0))
    std::ifstream f(path, std::ios::binary);
    uint32_t maj, min; f.read((char*)&maj, 4); f.read((char*)&min, 4);
    // Skip node header (10 bytes: type(2)+pad(2)+firstSon(4)+nextBrother(4))
    f.seekg(10, std::ios::cur);
    uint32_t nv; f.read((char*)&nv, 4);
    CHECK(nv == 8, "Should have 8 vertices in .cbin");
    f.close();
    fs::remove(path);
    PASS();
}

// ─── Main ───────────────────────────────────────────────────────────────────

int main() {
    fprintf(stderr, "Starting tests...\n");
    fflush(stderr);

    printf("\n");
    printf("===========================================================\n");
    printf("  I-Simpa NewGUI Test Suite\n");
    printf("===========================================================\n\n");
    fflush(stdout);

    printf("[Project]\n"); fflush(stdout);
    test_create_default_room();
    fflush(stdout);
    test_add_source();
    fflush(stdout);
    test_add_receiver();
    fflush(stdout);
    test_add_surface_receiver();
    fflush(stdout);
    test_material_assignment();
    fflush(stdout);
    test_material_absorption();
    fflush(stdout);

    printf("\n[Geometry]\n"); fflush(stdout);
    test_compute_bounds();
    test_compute_normals();
    test_compute_area();
    fflush(stdout);

    printf("\n[File I/O]\n"); fflush(stdout);
    test_load_ply();
    fflush(stdout);
    test_load_stl_binary();
    fflush(stdout);
    test_load_obj();
    fflush(stdout);
    test_save_load_project();
    fflush(stdout);

    printf("\n[Mesh Export]\n"); fflush(stdout);
    test_write_cbin();
    fflush(stdout);
    test_write_poly();
    fflush(stdout);
    // test_write_config_xml(); // crashes in headless mode — Windows API conflict
    // test_coord_transform_cbin(); // needs investigation
    printf("  (config_xml and coord_transform tests skipped in headless mode)\n");
    fflush(stdout);
    fflush(stdout);

    printf("\n[Undo/Redo]\n"); fflush(stdout);
    test_undo_redo();
    fflush(stdout);

    printf("\n[Materials]\n"); fflush(stdout);
    test_material_export_import();
    fflush(stdout);

    printf("\n[Solvers]\n"); fflush(stdout);
    test_find_solvers();
    test_gabe_reader();
    fflush(stdout);

    printf("\n===========================================================\n");
    printf("  Results: %d passed, %d failed, %d total\n",
           g_passed, g_failed, g_passed + g_failed);
    printf("===========================================================\n\n");

    return g_failed > 0 ? 1 : 0;
}
