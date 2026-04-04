#include "project/solver.h"
#include "mesh/miniz.h"
#include <fstream>
#include <sstream>
#include <cstdio>
#include <ctime>
#include <filesystem>
#include <cmath>
#include <cstring>
#include <algorithm>
#include <unordered_map>

#ifdef _WIN32
#include <windows.h>
#endif

namespace fs = std::filesystem;

namespace isimpa {

// ─── Coordinate System Transforms ──────────────────────────────────────────
// GL (Y-up): used by viewport/ImGui.  I-Simpa (Z-up): used by solvers/files.
// These are THE canonical transforms. Use nowhere else inline.
static glm::vec3 GLtoISim(const glm::vec3& v) { return {v.x, -v.z, v.y}; }
static glm::vec3 ISimToGL(const glm::vec3& v) { return {v.x, v.z, -v.y}; }

// ─── XML Helpers ────────────────────────────────────────────────────────────

static std::string EscapeXml(const std::string& s) {
    std::string out;
    out.reserve(s.size());
    for (char c : s) {
        switch (c) {
            case '&':  out += "&amp;";  break;
            case '<':  out += "&lt;";   break;
            case '>':  out += "&gt;";   break;
            case '"':  out += "&quot;"; break;
            default:   out += c;        break;
        }
    }
    return out;
}

// ─── Config XML Writer ──────────────────────────────────────────────────────

bool WriteConfigXML(const Project& project, const std::string& workingDir,
                    const std::string& solver) {
    std::string xmlPath = workingDir + "/config.xml";
    std::ofstream f(xmlPath);
    if (!f.is_open()) return false;

    auto& env = project.environment;

    // Sound speed from temperature
    float celerity = 331.3f + 0.606f * env.temperature;
    float rho = env.pressure / (287.05f * (env.temperature + 273.15f));

    // Count active frequency bands
    const bool* bands = (solver == "spps") ? project.sppsConfig.freqBands : project.tcrConfig.freqBands;
    int numBands = 0;
    for (int i = 0; i < 27; i++) if (bands[i]) numBands++;

    // Pre-flight validation
    if (numBands == 0) {
        fprintf(stderr, "[XML] Error: no frequency bands selected\n");
        return false;
    }
    if (project.sources.empty()) {
        fprintf(stderr, "[XML] Error: no sound sources defined\n");
        return false;
    }
    if (project.model.vertices.empty() || project.model.groups.empty()) {
        fprintf(stderr, "[XML] Error: no geometry loaded\n");
        return false;
    }

    float dt = (solver == "spps") ? project.sppsConfig.timeStep : 0.01f;
    float simLen = (solver == "spps") ? project.sppsConfig.simLength : 2.0f;
    int numTimeSteps = (int)(simLen / dt);

    f << "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
    f << "<configuration workingdirectory=\"" << workingDir << "/\">\n";

    // ── Simulation block ────────────────────────────────────────────────────
    f << "  <simulation\n";
    f << "    modelName=\"model.cbin\"\n";
    f << "    tetrameshFileName=\"tetramesh.mbin\"\n";
    f << "    recepteurss_directory=\"Surface_receiver/\"\n";
    f << "    recepteurss_filename=\"Sound_level.csbin\"\n";
    f << "    recepteurss_cut_filename=\"rs_cut.csbin\"\n";
    f << "    receiversp_directory=\"Punctual_receivers\"\n";
    f << "    receiversp_filename=\"Sound_level.recp\"\n";
    f << "    receiversp_filename_adv=\"Advanced_sound_level.gap\"\n";
    f << "    cumul_filename=\"Total_energy.recp\"\n";
    f << "    directivities_directory=\"directivities/\"\n";

    if (solver == "spps") {
        auto& cfg = project.sppsConfig;
        f << "    duree_simulation=\"" << cfg.simLength << "\"\n";
        f << "    pasdetemps=\"" << cfg.timeStep << "\"\n";
        f << "    nbparticules=\"" << cfg.particlesPerSource << "\"\n";
        f << "    nbparticules_rendu=\"" << cfg.particlesDisplay << "\"\n";
        f << "    rayon_recepteurp=\"" << cfg.receiverRadius << "\"\n";
        f << "    abs_atmo_calc=\"" << (cfg.atmoAbsorption ? 1 : 0) << "\"\n";
        f << "    trans_calc=\"" << (cfg.transmission ? 1 : 0) << "\"\n";
        f << "    direct_calc=\"" << (cfg.directFieldOnly ? 1 : 0) << "\"\n";
        f << "    enc_calc=\"" << (cfg.fittingDiffusion ? 1 : 0) << "\"\n";
        f << "    random_seed=\"" << cfg.randomSeed << "\"\n";
        f << "    computation_method=\"" << cfg.calcMethod << "\"\n";
        f << "    output_recs_byfreq=\"" << (cfg.surfaceExportPerBand ? 1 : 0) << "\"\n";
        f << "    output_recp_bysource=\"" << (cfg.echogramPerSource ? 1 : 0) << "\"\n";
        f << "    trans_epsilon=\"" << cfg.extinctionDb << "\"\n";
        f << "    surf_receiv_method=\"" << cfg.surfaceExportType << "\"\n";
        f << "    particules_filename=\"particles.pbin\"\n";
        f << "    particules_directory=\"particles/\"\n";
        f << "    stats_filename=\"statsSPPS.gabe\"\n";
        f << "    intensity_filename=\"Intensity vector.rpi\"\n";
        f << "    intensity_folder=\"IntensityAnimation\"\n";
    } else {
        auto& cfg = project.tcrConfig;
        f << "    abs_atmo_calc=\"" << (cfg.atmoAbsorption ? 1 : 0) << "\"\n";
        f << "    output_recs_byfreq=\"1\"\n"; // always enable for TCR
        f << "    direct_recepteurSOutputName=\"Direct field/\"\n";
        f << "    sabine_recepteurSOutputName=\"Total field (Sabine)/\"\n";
        f << "    eyring_recepteurSOutputName=\"Total field (Eyring)/\"\n";
    }
    f << "  >\n";

    // Frequency bands
    f << "    <freq_enum>\n";
    for (int i = 0; i < 27; i++) {
        if (bands[i]) {
            f << "      <bfreq freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
              << "\" docalc=\"1\"/>\n";
        }
    }
    f << "    </freq_enum>\n";
    f << "  </simulation>\n";

    // ── Environment ─────────────────────────────────────────────────────────
    f << "  <condition_atmospherique\n";
    f << "    temperature=\"" << env.temperature << "\"\n";
    f << "    pression=\"" << env.pressure << "\"\n";
    f << "    humidite=\"" << env.humidity << "\"\n";
    f << "    celerite=\"" << celerity << "\"\n";
    f << "    rho=\"" << rho << "\"\n";
    f << "    z0=\"" << env.z0 << "\"\n";
    f << "    alog=\"" << env.aLog << "\"\n";
    f << "    blin=\"" << env.bLin << "\"\n";
    f << "  />\n";

    // ── Materials ───────────────────────────────────────────────────────────
    f << "  <surface_absorption_enum>\n";
    for (auto& mat : project.materials) {
        bool used = false;
        for (auto& [grpIdx, matIdx] : project.groupMaterialMap) {
            if (matIdx == mat.id) { used = true; break; }
        }
        if (!used && mat.id > 0) continue; // only write used materials + first

        f << "    <type_surface id=\"" << mat.id << "\" resistivite=\"" << mat.resistivity
          << "\" side_material=\"" << (mat.bilateral ? 1 : 0) << "\">\n";
        for (int i = 0; i < 27; i++) {
            if (bands[i]) {
                f << "      <bfreq freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
                  << "\" absorb=\"" << mat.bands[i].absorption
                  << "\" diffusion=\"" << mat.bands[i].diffusion
                  << "\" loi=\"" << mat.bands[i].diffusionLaw;
                if (mat.bands[i].transmission > 0)
                    f << "\" affaiblissement=\"" << mat.bands[i].transmission;
                f << "\"/>\n";
            }
        }
        f << "    </type_surface>\n";
    }
    f << "  </surface_absorption_enum>\n";

    // ── Sources ─────────────────────────────────────────────────────────────
    f << "  <sources>\n";
    for (auto& src : project.sources) {
        if (!src.active) continue;
        glm::vec3 sp = GLtoISim(src.position);
        glm::vec3 sd = GLtoISim(src.direction);
        f << "    <source id=\"" << src.id
          << "\" name=\"" << EscapeXml(src.name)
          << "\" x=\"" << sp.x
          << "\" y=\"" << sp.y
          << "\" z=\"" << sp.z
          << "\" u=\"" << sd.x
          << "\" v=\"" << sd.y
          << "\" w=\"" << sd.z
          << "\" directivite=\"" << src.directivity
          << "\" delay=\"" << src.delaySeconds << "\">\n";
        for (int i = 0; i < 27; i++) {
            if (bands[i]) {
                f << "      <bfreq freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
                  << "\" db=\"" << src.spectrum[i] << "\"/>\n";
            }
        }
        f << "    </source>\n";
    }
    f << "  </sources>\n";

    // ── Punctual Receivers ──────────────────────────────────────────────────
    f << "  <recepteursp>\n";
    for (auto& rcv : project.punctualReceivers) {
        if (!rcv.active) continue;
        glm::vec3 rp = GLtoISim(rcv.position);
        glm::vec3 rd = GLtoISim(rcv.direction);
        f << "    <recepteur_ponctuel id=\"" << rcv.id
          << "\" name=\"" << EscapeXml(rcv.name)
          << "\" lbl=\"" << EscapeXml(rcv.name)
          << "\" x=\"" << rp.x
          << "\" y=\"" << rp.y
          << "\" z=\"" << rp.z
          << "\" u=\"" << rd.x
          << "\" v=\"" << rd.y
          << "\" w=\"" << rd.z
          << "\">\n";
        // Per-frequency power (same as source — needed for weighting)
        for (int i = 0; i < 27; i++) {
            if (bands[i]) {
                f << "      <bfreq freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
                  << "\" db=\"0\"/>\n";
            }
        }
        f << "    </recepteur_ponctuel>\n";
    }
    f << "  </recepteursp>\n";

    // ── Surface Receivers ───────────────────────────────────────────────────
    f << "  <recepteurss>\n";
    for (auto& sr : project.surfaceReceivers) {
        if (sr.type == SurfaceReceiver::Plane) {
            glm::vec3 sa = GLtoISim(sr.vertexA), sb = GLtoISim(sr.vertexB), sc = GLtoISim(sr.vertexC);
            f << "    <recepteur_surfacique_coupe id=\"" << sr.id
              << "\" name=\"" << EscapeXml(sr.name) << "\""
              << " ax=\"" << sa.x << "\" ay=\"" << sa.y << "\" az=\"" << sa.z << "\""
              << " bx=\"" << sb.x << "\" by=\"" << sb.y << "\" bz=\"" << sb.z << "\""
              << " cx=\"" << sc.x << "\" cy=\"" << sc.y << "\" cz=\"" << sc.z << "\""
              << " resolution=\"" << sr.gridResolution << "\""
              << "/>\n";
        } else {
            f << "    <recepteur_surfacique id=\"" << sr.id
              << "\" name=\"" << EscapeXml(sr.name) << "\"/>\n";
        }
    }
    f << "  </recepteurss>\n";

    // ── Mesh faces with material assignments ────────────────────────────────
    f << "  <surface_mesh>\n";
    int faceGlobalIdx = 0;
    for (int gi = 0; gi < (int)project.model.groups.size(); gi++) {
        int matId = 0;
        auto it = project.groupMaterialMap.find(gi);
        if (it != project.groupMaterialMap.end() && it->second >= 0 &&
            it->second < (int)project.materials.size()) {
            matId = project.materials[it->second].id;
        }

        for (auto& face : project.model.groups[gi].faces) {
            if (face.v[0] >= project.model.vertices.size() ||
                face.v[1] >= project.model.vertices.size() ||
                face.v[2] >= project.model.vertices.size()) continue;
            f << "    <face id=\"" << faceGlobalIdx++
              << "\" material=\"" << matId
              << "\" v0=\"" << face.v[0]
              << "\" v1=\"" << face.v[1]
              << "\" v2=\"" << face.v[2] << "\"/>\n";
        }
    }
    f << "  </surface_mesh>\n";

    // ── Vertices ────────────────────────────────────────────────────────────
    f << "  <vertices>\n";
    for (int i = 0; i < (int)project.model.vertices.size(); i++) {
        glm::vec3 sv = GLtoISim(project.model.vertices[i]);
        f << "    <v id=\"" << i
          << "\" x=\"" << sv.x
          << "\" y=\"" << sv.y
          << "\" z=\"" << sv.z << "\"/>\n";
    }
    f << "  </vertices>\n";

    // ── Subdomains (required placeholder) ──────────────────────────────────
    f << "  <subdomains/>\n";

    // ── Encumbrances (fitting zones) ────────────────────────────────────────
    f << "  <encombrement_enum>\n";
    for (auto& enc : project.encumbrances) {
        if (!enc.active) continue;
        f << "    <encombrement id=\"" << enc.id << "\">\n";
        for (int i = 0; i < 27; i++) {
            if (bands[i]) {
                f << "      <bfreq freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
                  << "\" alpha=\"" << enc.bands[i].absorption
                  << "\" lambda=\"" << enc.bands[i].meanFreePath
                  << "\" loi_diff=\"" << enc.bands[i].diffusionLaw << "\"/>\n";
            }
        }
        f << "    </encombrement>\n";
    }
    f << "  </encombrement_enum>\n";

    // ── Background noise ────────────────────────────────────────────────────
    if (project.backgroundNoise.enabled) {
        f << "  <background_noise>\n";
        for (int i = 0; i < 27; i++) {
            if (bands[i]) {
                f << "    <bfreq freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
                  << "\" db=\"" << project.backgroundNoise.spectrum[i] << "\"/>\n";
            }
        }
        f << "  </background_noise>\n";
    }

    f << "</configuration>\n";
    f.flush();
    bool writeOk = f.good();
    f.close();

    if (!writeOk) {
        fprintf(stderr, "[XML] Write error on config: %s\n", xmlPath.c_str());
        return false;
    }
    printf("[XML] Wrote config: %s\n", xmlPath.c_str());
    return true;
}

// ─── Write .cbin binary mesh (I-Simpa format) ──────────────────────────────

bool WriteMeshBinary(const Project& project, const std::string& path) {
    std::ofstream f(path, std::ios::binary);
    if (!f.is_open()) return false;

    auto& model = project.model;

    // Types matching I-Simpa's bin.cpp
    typedef unsigned short bShort;
    typedef uint32_t bInt;
    typedef int32_t bsInt;
    typedef float bFloat;
    typedef uint32_t bLong;

    enum { NODE_TYPE_VERTICES = 0, NODE_TYPE_GROUP = 1 };

    auto writeNode = [&](bShort nodeType, bLong firstSon, bLong nextBrother) {
        f.write((char*)&nodeType, sizeof(bShort));
        bShort pad = 0;
        f.write((char*)&pad, sizeof(bShort)); // padding
        f.write((char*)&firstSon, sizeof(bLong));
        f.write((char*)&nextBrother, sizeof(bLong));
    };

    // File header: version 1.0
    bInt major = 1, minor = 0;
    f.write((char*)&major, sizeof(bInt));
    f.write((char*)&minor, sizeof(bInt));

    // ── Vertices node ───────────────────────────────────────────────────────
    std::streampos vertNodePos = f.tellp();
    writeNode(NODE_TYPE_VERTICES, 0, 0); // placeholder, will rewrite

    bLong nbVertex = (bLong)model.vertices.size();
    f.write((char*)&nbVertex, sizeof(bLong));

    for (auto& v : model.vertices) {
        glm::vec3 sv = GLtoISim(v);
        f.write((char*)&sv.x, sizeof(bFloat));
        f.write((char*)&sv.y, sizeof(bFloat));
        f.write((char*)&sv.z, sizeof(bFloat));
    }

    std::streampos afterVerts = f.tellp();
    // Rewrite vertices node with correct nextBrother
    f.seekp(vertNodePos);
    writeNode(NODE_TYPE_VERTICES, 0, (bLong)afterVerts);
    f.seekp(afterVerts);

    // ── Group nodes (one per surface group, chained via nextBrother) ────────
    // Each group node: [header 12B] [name 255B] [null 1B] [nbFaces 4B] [faces...]
    // Face: [a 4B] [b 4B] [c 4B] [matId 4B] [idRs 4B] [idEn 4B] = 24 bytes
    int numGroups = (int)model.groups.size();
    std::vector<std::streampos> groupNodePositions(numGroups);

    for (int gi = 0; gi < numGroups; gi++) {
        auto& grp = model.groups[gi];

        // Record position for this node header (will rewrite nextBrother)
        groupNodePositions[gi] = f.tellp();
        writeNode(NODE_TYPE_GROUP, 0, 0); // placeholder nextBrother

        // Group name (255 bytes + 1 null)
        char gname[255] = {};
        strncpy(gname, grp.name.c_str(), sizeof(gname) - 1);
        f.write(gname, 255);
        char nul = '\0';
        f.write(&nul, 1);

        // Face count
        bInt nbFace = (bInt)grp.faces.size();
        f.write((char*)&nbFace, sizeof(bInt));

        // Resolve material ID: use material.id (not vector index)
        bInt matId = 0;
        auto it = project.groupMaterialMap.find(gi);
        if (it != project.groupMaterialMap.end() && it->second >= 0 &&
            it->second < (int)project.materials.size()) {
            matId = (bInt)project.materials[it->second].id;
        }

        // Write faces
        for (auto& face : grp.faces) {
            bInt a = face.v[0], b = face.v[1], c = face.v[2];
            bsInt idRs = -1;
            bsInt idEn = -1;
            f.write((char*)&a, sizeof(bInt));
            f.write((char*)&b, sizeof(bInt));
            f.write((char*)&c, sizeof(bInt));
            f.write((char*)&matId, sizeof(bInt));
            f.write((char*)&idRs, sizeof(bsInt));
            f.write((char*)&idEn, sizeof(bsInt));
        }
    }

    // Rewrite group node headers with correct nextBrother pointers
    std::streampos endPos = f.tellp();
    for (int gi = 0; gi < numGroups - 1; gi++) {
        f.seekp(groupNodePositions[gi]);
        writeNode(NODE_TYPE_GROUP, 0, (bLong)groupNodePositions[gi + 1]);
    }
    // Last group: nextBrother = 0 (end of chain)
    if (numGroups > 0) {
        f.seekp(groupNodePositions[numGroups - 1]);
        writeNode(NODE_TYPE_GROUP, 0, 0);
    }
    f.seekp(endPos);

    f.close();
    int totalFacesWritten = 0;
    for (int gi = 0; gi < numGroups; gi++) {
        printf("[CBIN]   Group %d: \"%s\" %zu faces, matId=%d\n",
               gi, model.groups[gi].name.c_str(), model.groups[gi].faces.size(),
               (project.groupMaterialMap.count(gi) && project.groupMaterialMap.at(gi) < (int)project.materials.size())
                   ? project.materials[project.groupMaterialMap.at(gi)].id : -1);
        totalFacesWritten += (int)model.groups[gi].faces.size();
    }
    printf("[CBIN] Wrote mesh: %s (%u verts, %d faces, %d groups)\n",
           path.c_str(), nbVertex, totalFacesWritten, numGroups);
    return true;
}

// ─── Write .poly file for TetGen ────────────────────────────────────────────

bool WritePoly(const Project& project, const std::string& path) {
    std::ofstream f(path);
    if (!f.is_open()) return false;

    auto& model = project.model;

    // Part 1: Nodes
    int nVerts = (int)model.vertices.size();
    f << nVerts << " 3 0 0\n";

    for (int i = 0; i < nVerts; i++) {
        glm::vec3 sv = GLtoISim(model.vertices[i]);
        f << (i + 1) << " " << sv.x << " " << sv.y << " " << sv.z << "\n";
    }

    // Part 2: Facets — each triangle is its own facet (1 polygon per facet)
    // TetGen .poly format:
    //   <nFacets> <boundaryMarkers>
    //   For each facet:
    //     <nPolygons>              (on its own line)
    //     <nVertices> v1 v2 v3    (one polygon)
    int totalFaces = 0;
    for (auto& grp : model.groups) totalFaces += (int)grp.faces.size();
    f << totalFaces << " 0\n";

    for (auto& grp : model.groups) {
        for (auto& face : grp.faces) {
            f << "1\n"; // 1 polygon in this facet
            f << "3 " << (face.v[0] + 1) << " " << (face.v[1] + 1) << " " << (face.v[2] + 1) << "\n";
        }
    }

    // Part 3: Holes (none)
    f << "0\n";

    // Part 4: Region attributes (none)
    f << "0\n";

    f.close();
    printf("[POLY] Wrote: %s (%d verts, %d faces)\n", path.c_str(), nVerts, totalFaces);
    return true;
}

// ─── Convert TetGen output to .mbin ─────────────────────────────────────────

bool ConvertTetGenToMbin(const std::string& basePath, const std::string& mbinPath,
                         const Project& project) {
    // TetGen outputs: basePath.1.node, basePath.1.ele, basePath.1.face, basePath.1.neigh
    std::string nodeFile = basePath + ".1.node";
    std::string eleFile  = basePath + ".1.ele";
    std::string faceFile = basePath + ".1.face";
    std::string neighFile = basePath + ".1.neigh";

    // ── Read nodes ──────────────────────────────────────────────────────────
    std::ifstream fnodes(nodeFile);
    if (!fnodes.is_open()) {
        fprintf(stderr, "[MBIN] Cannot open node file: %s\n", nodeFile.c_str());
        return false;
    }

    int numNodes, dim, numAttrs, numBoundary;
    fnodes >> numNodes >> dim >> numAttrs >> numBoundary;
    if (numNodes <= 0 || dim != 3) { fnodes.close(); return false; }

    struct Node { float x, y, z; };
    std::vector<Node> nodes(numNodes);

    for (int i = 0; i < numNodes; i++) {
        int id;
        float x, y, z;
        fnodes >> id >> x >> y >> z;
        // Skip attributes and boundary markers
        for (int a = 0; a < numAttrs + numBoundary; a++) { float skip; fnodes >> skip; }
        // TetGen uses 1-based indexing; store at 0-based position
        int idx = id - 1;
        if (idx >= 0 && idx < numNodes) {
            nodes[idx] = {x, y, z};
        }
    }
    fnodes.close();

    // ── Read elements (tetrahedra) ──────────────────────────────────────────
    std::ifstream fele(eleFile);
    if (!fele.is_open()) {
        fprintf(stderr, "[MBIN] Cannot open ele file: %s\n", eleFile.c_str());
        return false;
    }

    int numTetras, nodesPerTet, numEleAttrs;
    fele >> numTetras >> nodesPerTet >> numEleAttrs;
    if (numTetras <= 0 || nodesPerTet < 4) { fele.close(); return false; }

    struct Tetra {
        int32_t v[4];
        int32_t idVolume;
        struct { int32_t v[3]; int32_t marker; int32_t neighbor; } faces[4];
    };
    std::vector<Tetra> tetras(numTetras);

    for (int i = 0; i < numTetras; i++) {
        int id;
        int a, b, c, d;
        fele >> id >> a >> b >> c >> d;
        // Convert to 0-based
        a--; b--; c--; d--;

        int idVolume = 0;
        if (numEleAttrs > 0) fele >> idVolume;

        int idx = id - 1;
        if (idx >= 0 && idx < numTetras &&
            a >= 0 && a < numNodes && b >= 0 && b < numNodes &&
            c >= 0 && c < numNodes && d >= 0 && d < numNodes) {
            auto& t = tetras[idx];
            t.v[0] = a; t.v[1] = b; t.v[2] = c; t.v[3] = d;
            t.idVolume = idVolume;

            // Initialize face vertex indices (standard tet face ordering)
            // Face 0 (opposite vertex a): b, d, c
            t.faces[0].v[0] = b; t.faces[0].v[1] = d; t.faces[0].v[2] = c;
            // Face 1 (opposite vertex b): c, d, a
            t.faces[1].v[0] = c; t.faces[1].v[1] = d; t.faces[1].v[2] = a;
            // Face 2 (opposite vertex c): a, d, b
            t.faces[2].v[0] = a; t.faces[2].v[1] = d; t.faces[2].v[2] = b;
            // Face 3 (opposite vertex d): b, c, a
            t.faces[3].v[0] = b; t.faces[3].v[1] = c; t.faces[3].v[2] = a;

            for (int f = 0; f < 4; f++) {
                t.faces[f].marker = -1;
                t.faces[f].neighbor = -2;
            }
        }
    }
    fele.close();

    // ── Match tet boundary faces to scene mesh faces ──────────────────────
    // The marker field must be the INDEX into the scene mesh face array (from .cbin)
    // so the SPPS solver can look up the material for wall reflections.
    // Build scene face list (global face index across all groups)
    struct SceneFace { int v[3]; int globalIdx; };
    std::vector<SceneFace> sceneFaces;
    {
        int globalIdx = 0;
        for (int gi = 0; gi < (int)project.model.groups.size(); gi++) {
            for (int fi = 0; fi < (int)project.model.groups[gi].faces.size(); fi++) {
                SceneFace sf;
                sf.v[0] = project.model.groups[gi].faces[fi].v[0];
                sf.v[1] = project.model.groups[gi].faces[fi].v[1];
                sf.v[2] = project.model.groups[gi].faces[fi].v[2];
                sf.globalIdx = globalIdx++;
                sceneFaces.push_back(sf);
            }
        }
    }

    // Build vertex-to-scene-face lookup
    std::unordered_map<int, std::vector<int>> vertToSceneFace;
    for (int si = 0; si < (int)sceneFaces.size(); si++) {
        vertToSceneFace[sceneFaces[si].v[0]].push_back(si);
        vertToSceneFace[sceneFaces[si].v[1]].push_back(si);
        vertToSceneFace[sceneFaces[si].v[2]].push_back(si);
    }

    // Helper: check if three vertices match (any permutation)
    auto facesMatch = [](int a0, int a1, int a2, int b0, int b1, int b2) -> bool {
        int sa[3] = {a0, a1, a2}, sb[3] = {b0, b1, b2};
        std::sort(sa, sa+3); std::sort(sb, sb+3);
        return sa[0]==sb[0] && sa[1]==sb[1] && sa[2]==sb[2];
    };

    // For each tet face, find the matching scene mesh face
    int matchedFaces = 0;
    for (int ti = 0; ti < numTetras; ti++) {
        auto& t = tetras[ti];
        for (int fi = 0; fi < 4; fi++) {
            int fv0 = t.faces[fi].v[0], fv1 = t.faces[fi].v[1], fv2 = t.faces[fi].v[2];
            // Look up scene faces that share vertex fv0
            auto it = vertToSceneFace.find(fv0);
            if (it == vertToSceneFace.end()) continue;
            for (int si : it->second) {
                auto& sf = sceneFaces[si];
                if (facesMatch(fv0, fv1, fv2, sf.v[0], sf.v[1], sf.v[2])) {
                    t.faces[fi].marker = sf.globalIdx; // Scene face index
                    matchedFaces++;
                    break;
                }
            }
        }
    }
    printf("[MBIN] Matched %d tet faces to scene mesh faces\n", matchedFaces);

    // ── Read neighbor information ───────────────────────────────────────────
    bool hasNeighFile = false;
    std::ifstream fneigh(neighFile);
    if (fneigh.is_open()) {
        int numNeigh, neighPerTet;
        fneigh >> numNeigh >> neighPerTet;

        for (int i = 0; i < numNeigh; i++) {
            int id, n0, n1, n2, n3;
            fneigh >> id >> n0 >> n1 >> n2 >> n3;
            int idx = id - 1;
            if (idx >= 0 && idx < numTetras) {
                tetras[idx].faces[0].neighbor = (n0 > 0) ? n0 - 1 : -2;
                tetras[idx].faces[1].neighbor = (n1 > 0) ? n1 - 1 : -2;
                tetras[idx].faces[2].neighbor = (n2 > 0) ? n2 - 1 : -2;
                tetras[idx].faces[3].neighbor = (n3 > 0) ? n3 - 1 : -2;
            }
        }
        fneigh.close();
        hasNeighFile = true;
    }

    // ── Compute neighbors ourselves if .neigh is missing ────────────────────
    // This is critical: without neighbors, SPPS particles can't traverse tets.
    // Algorithm: hash each tet face's sorted vertex triple → find matching face
    // in another tet → they are neighbors.
    {
        // Check if we actually have valid neighbors
        int validNeighborCount = 0;
        for (int ti = 0; ti < numTetras; ti++)
            for (int fi = 0; fi < 4; fi++)
                if (tetras[ti].faces[fi].neighbor >= 0) validNeighborCount++;

        if (validNeighborCount == 0) {
            printf("[MBIN] No valid neighbors found%s — computing from tet topology...\n",
                   hasNeighFile ? " (despite .neigh file)" : " (.neigh file missing)");

            // Hash: pack 3 sorted vertex indices into a uint64
            auto faceHash = [](int a, int b, int c) -> uint64_t {
                int s[3] = {a, b, c};
                if (s[0] > s[1]) std::swap(s[0], s[1]);
                if (s[1] > s[2]) std::swap(s[1], s[2]);
                if (s[0] > s[1]) std::swap(s[0], s[1]);
                return ((uint64_t)s[0] << 40) | ((uint64_t)s[1] << 20) | (uint64_t)s[2];
            };

            // Map from face hash → (tetIdx, faceIdx)
            std::unordered_map<uint64_t, std::pair<int, int>> faceMap;
            faceMap.reserve(numTetras * 4);

            int neighborsFound = 0;
            for (int ti = 0; ti < numTetras; ti++) {
                auto& t = tetras[ti];
                for (int fi = 0; fi < 4; fi++) {
                    uint64_t key = faceHash(t.faces[fi].v[0], t.faces[fi].v[1], t.faces[fi].v[2]);
                    auto it = faceMap.find(key);
                    if (it != faceMap.end()) {
                        // Found matching face in another tet — they are neighbors
                        int otherTet = it->second.first;
                        int otherFace = it->second.second;
                        t.faces[fi].neighbor = otherTet;
                        tetras[otherTet].faces[otherFace].neighbor = ti;
                        neighborsFound += 2;
                        faceMap.erase(it); // Each face pair matches exactly once
                    } else {
                        faceMap[key] = {ti, fi};
                    }
                }
            }

            // Remaining unmatched faces are boundary faces (neighbor stays -2)
            int boundaryFaces = (int)faceMap.size();
            printf("[MBIN] Computed neighbors: %d internal face pairs, %d boundary faces\n",
                   neighborsFound / 2, boundaryFaces);
        }
    }

    // ── Write .mbin binary ──────────────────────────────────────────────────
    std::ofstream fout(mbinPath, std::ios::binary);
    if (!fout.is_open()) {
        fprintf(stderr, "[MBIN] Cannot write: %s\n", mbinPath.c_str());
        return false;
    }

    // Header
    uint32_t quantTetra = (uint32_t)numTetras;
    uint32_t quantNodes = (uint32_t)numNodes;
    fout.write((char*)&quantTetra, 4);
    fout.write((char*)&quantNodes, 4);

    // Nodes
    for (int i = 0; i < numNodes; i++) {
        fout.write((char*)&nodes[i].x, 4);
        fout.write((char*)&nodes[i].y, 4);
        fout.write((char*)&nodes[i].z, 4);
    }

    // Tetrahedra
    for (int i = 0; i < numTetras; i++) {
        auto& t = tetras[i];
        fout.write((char*)&t.v[0], 4);
        fout.write((char*)&t.v[1], 4);
        fout.write((char*)&t.v[2], 4);
        fout.write((char*)&t.v[3], 4);
        fout.write((char*)&t.idVolume, 4);
        for (int f = 0; f < 4; f++) {
            fout.write((char*)&t.faces[f].v[0], 4);
            fout.write((char*)&t.faces[f].v[1], 4);
            fout.write((char*)&t.faces[f].v[2], 4);
            fout.write((char*)&t.faces[f].marker, 4);
            fout.write((char*)&t.faces[f].neighbor, 4);
        }
    }

    fout.close();
    printf("[MBIN] Wrote: %s (%u nodes, %u tetrahedra)\n", mbinPath.c_str(), quantNodes, quantTetra);
    return true;
}

// ─── Find solver executable ─────────────────────────────────────────────────

std::string FindSolverExe(const std::string& solver) {
    // Get exe directory for relative paths
    std::string exeDir;
#ifdef _WIN32
    char exePath[MAX_PATH] = {};
    GetModuleFileNameA(nullptr, exePath, MAX_PATH);
    exeDir = std::string(exePath);
    auto lastSlash = exeDir.find_last_of("\\/");
    if (lastSlash != std::string::npos) exeDir = exeDir.substr(0, lastSlash + 1);
#endif

    std::vector<std::string> searchPaths = {
        exeDir + "solvers/",
        exeDir,
        "solvers/",
        "./solvers/",
    };

    std::string exeName;
    if (solver == "spps") exeName = "spps.exe";
    else if (solver == "tcr" || solver == "tc") exeName = "classicalTheory.exe";
    else if (solver == "tetgen") exeName = "tetgen.exe";
    else if (solver == "preprocess") exeName = "preprocess.exe";
    else exeName = solver + ".exe";

    for (auto& base : searchPaths) {
        std::string full = base + exeName;
        if (fs::exists(full)) return fs::absolute(full).string();

        // Also try in subdirectories
        std::string sub = base + solver + "/" + exeName;
        if (fs::exists(sub)) return fs::absolute(sub).string();
    }

    return "";
}

// ─── Run external process (shared helper) ───────────────────────────────────

#ifdef _WIN32
static bool RunProcess(const std::string& cmdLine, const std::string& workingDir,
                       std::vector<std::string>& output, int& exitCode) {
    SECURITY_ATTRIBUTES sa = {};
    sa.nLength = sizeof(sa);
    sa.bInheritHandle = TRUE;

    HANDLE hReadPipe, hWritePipe;
    CreatePipe(&hReadPipe, &hWritePipe, &sa, 0);
    SetHandleInformation(hReadPipe, HANDLE_FLAG_INHERIT, 0);

    STARTUPINFOA si = {};
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hWritePipe;
    si.hStdError = hWritePipe;

    PROCESS_INFORMATION pi = {};

    char cmdBuf[4096];
    if (cmdLine.size() >= sizeof(cmdBuf)) {
        fprintf(stderr, "[Process] Command line too long (%zu chars), truncated\n", cmdLine.size());
    }
    strncpy(cmdBuf, cmdLine.c_str(), sizeof(cmdBuf) - 1);
    cmdBuf[sizeof(cmdBuf) - 1] = '\0';

    BOOL ok = CreateProcessA(nullptr, cmdBuf, nullptr, nullptr, TRUE,
                             CREATE_NO_WINDOW, nullptr, workingDir.c_str(), &si, &pi);

    CloseHandle(hWritePipe);

    if (!ok) {
        CloseHandle(hReadPipe);
        exitCode = -1;
        return false;
    }

    // Read stdout
    char buf[4096];
    DWORD bytesRead;
    std::string lineBuffer;

    while (ReadFile(hReadPipe, buf, sizeof(buf) - 1, &bytesRead, nullptr) && bytesRead > 0) {
        buf[bytesRead] = '\0';
        lineBuffer += buf;

        size_t pos;
        while ((pos = lineBuffer.find('\n')) != std::string::npos) {
            std::string line = lineBuffer.substr(0, pos);
            lineBuffer.erase(0, pos + 1);
            if (!line.empty() && line.back() == '\r') line.pop_back();
            if (!line.empty()) output.push_back(line);
        }
    }
    if (!lineBuffer.empty()) output.push_back(lineBuffer);

    WaitForSingleObject(pi.hProcess, INFINITE);

    DWORD ec = 0;
    GetExitCodeProcess(pi.hProcess, &ec);
    exitCode = (int)ec;

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    CloseHandle(hReadPipe);

    return exitCode == 0;
}
#endif

// ─── Run Mesh Generation (preprocess + TetGen) ─────────────────────────────

bool RunMeshGeneration(const std::string& workingDir, Project& project,
                       std::function<void(float, const std::string&)> progressCallback) {
    fs::create_directories(workingDir);

    // Step 1: Write .cbin mesh
    std::string cbinPath = workingDir + "/model.cbin";
    if (!WriteMeshBinary(project, cbinPath)) {
        project.simProgress.errors.push_back("Failed to write model.cbin");
        return false;
    }
    if (progressCallback) progressCallback(10.0f, "Wrote binary mesh");

    // Step 2: Write .poly for TetGen
    std::string polyPath = workingDir + "/model.poly";
    if (!WritePoly(project, polyPath)) {
        project.simProgress.errors.push_back("Failed to write model.poly");
        return false;
    }
    if (progressCallback) progressCallback(20.0f, "Wrote POLY file");

    // Step 3: Run preprocess (optional - clean mesh)
    std::string preprocessExe = FindSolverExe("preprocess");
    if (!preprocessExe.empty()) {
        std::string cmd = "\"" + preprocessExe + "\" \"" + cbinPath + "\"";
        printf("[Mesh] Running preprocess: %s\n", cmd.c_str());
        if (progressCallback) progressCallback(30.0f, "Running mesh preprocessing...");

#ifdef _WIN32
        std::vector<std::string> output;
        int ec = 0;
        RunProcess(cmd, workingDir, output, ec);
        for (auto& line : output) {
            project.simProgress.log.push_back("[preprocess] " + line);
        }
        if (ec != 0) {
            project.simProgress.warnings.push_back("Preprocess returned non-zero exit code (continuing anyway)");
        }
#endif
    } else {
        printf("[Mesh] preprocess.exe not found, skipping\n");
    }
    if (progressCallback) progressCallback(40.0f, "Preprocessing complete");

    // Step 4: Run TetGen
    std::string tetgenExe = FindSolverExe("tetgen");
    if (tetgenExe.empty()) {
        project.simProgress.errors.push_back("tetgen.exe not found");
        return false;
    }

    // TetGen parameters:
    // -p: PLC input, -q: quality mesh, -A: assign attributes
    // -n: output neighbors, -T1e-7: tolerance for coplanar detection
    // -Y: preserve surface mesh (don't insert Steiner points on boundary)
    // This handles meshes with shared edges between groups
    std::string cmd = "\"" + tetgenExe + "\" -pq" + std::to_string(project.meshConfig.qualityRatio) + " -A -n -Y -T1e-7" + (project.meshConfig.useVolumeConstraint ? " -a" + std::to_string(project.meshConfig.maxVolume) : "") + " \"" + polyPath + "\"";
    printf("[Mesh] Running TetGen: %s\n", cmd.c_str());
    if (progressCallback) progressCallback(50.0f, "Running TetGen meshing...");

#ifdef _WIN32
    {
        std::vector<std::string> output;
        int ec = 0;
        bool ok = RunProcess(cmd, workingDir, output, ec);
        for (auto& line : output) {
            project.simProgress.log.push_back("[tetgen] " + line);
        }
        if (!ok) {
            project.simProgress.errors.push_back("TetGen failed (exit code " + std::to_string(ec) + ")");
            return false;
        }
    }
#endif

    if (progressCallback) progressCallback(80.0f, "TetGen complete");

    // Step 5: Convert TetGen output to .mbin format
    std::string nodeFile = workingDir + "/model.1.node";
    std::string eleFile = workingDir + "/model.1.ele";
    if (!fs::exists(nodeFile) || !fs::exists(eleFile)) {
        project.simProgress.errors.push_back("TetGen output files not found");
        return false;
    }

    if (progressCallback) progressCallback(85.0f, "Converting to .mbin...");
    std::string mbinPath = workingDir + "/tetramesh.mbin";
    std::string tetgenBase = workingDir + "/model";
    if (!ConvertTetGenToMbin(tetgenBase, mbinPath, project)) {
        project.simProgress.errors.push_back("Failed to convert TetGen output to .mbin");
        return false;
    }

    project.meshGenerated = true;
    if (progressCallback) progressCallback(100.0f, "Mesh generation complete");
    printf("[Mesh] Mesh generation complete\n");
    return true;
}

// ─── Run Solver ─────────────────────────────────────────────────────────────

bool RunSolver(const std::string& solver, const std::string& workingDir,
               Project& project,
               std::function<void(float, const std::string&)> progressCallback) {
    // Write config
    if (!WriteConfigXML(project, workingDir, solver)) {
        project.simProgress.errors.push_back("Failed to write config.xml");
        return false;
    }

    std::string exePath = FindSolverExe(solver);
    if (exePath.empty()) {
        project.simProgress.errors.push_back("Solver not found: " + solver);
        return false;
    }

    std::string configPath = workingDir + "/config.xml";
    std::string cmdLine = "\"" + exePath + "\" \"" + configPath + "\"";

    printf("[Solver] Running: %s\n", cmdLine.c_str());

    project.simProgress.running = true;
    project.simProgress.percent = 0;
    project.simProgress.log.clear();
    project.simProgress.warnings.clear();
    project.simProgress.errors.clear();

#ifdef _WIN32
    // Create process with piped stdout
    SECURITY_ATTRIBUTES sa = {};
    sa.nLength = sizeof(sa);
    sa.bInheritHandle = TRUE;

    HANDLE hReadPipe, hWritePipe;
    CreatePipe(&hReadPipe, &hWritePipe, &sa, 0);
    SetHandleInformation(hReadPipe, HANDLE_FLAG_INHERIT, 0);

    STARTUPINFOA si = {};
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hWritePipe;
    si.hStdError = hWritePipe;

    PROCESS_INFORMATION pi = {};

    char cmdBuf[4096];
    if (cmdLine.size() >= sizeof(cmdBuf)) {
        fprintf(stderr, "[Process] Command line too long (%zu chars), truncated\n", cmdLine.size());
    }
    strncpy(cmdBuf, cmdLine.c_str(), sizeof(cmdBuf) - 1);
    cmdBuf[sizeof(cmdBuf) - 1] = '\0';

    BOOL ok = CreateProcessA(nullptr, cmdBuf, nullptr, nullptr, TRUE,
                             CREATE_NO_WINDOW, nullptr, workingDir.c_str(), &si, &pi);

    CloseHandle(hWritePipe);

    if (!ok) {
        CloseHandle(hReadPipe);
        project.simProgress.errors.push_back("Failed to launch: " + exePath);
        project.simProgress.running = false;
        return false;
    }

    // Read stdout line by line
    char buf[4096];
    DWORD bytesRead;
    std::string lineBuffer;

    while (ReadFile(hReadPipe, buf, sizeof(buf) - 1, &bytesRead, nullptr) && bytesRead > 0) {
        buf[bytesRead] = '\0';
        lineBuffer += buf;

        size_t pos;
        while ((pos = lineBuffer.find('\n')) != std::string::npos) {
            std::string line = lineBuffer.substr(0, pos);
            lineBuffer.erase(0, pos + 1);
            if (!line.empty() && line.back() == '\r') line.pop_back();

            if (line.empty()) continue;

            if (line[0] == '#') {
                // Progress line
                float pct = 0;
                sscanf(line.c_str() + 1, "%f", &pct);
                project.simProgress.percent = pct;
                if (progressCallback) progressCallback(pct, line);
            } else if (line[0] == '!') {
                project.simProgress.warnings.push_back(line.substr(1));
            } else {
                project.simProgress.log.push_back(line);
            }
        }
    }

    WaitForSingleObject(pi.hProcess, INFINITE);

    DWORD exitCode = 0;
    GetExitCodeProcess(pi.hProcess, &exitCode);

    CloseHandle(pi.hProcess);
    CloseHandle(pi.hThread);
    CloseHandle(hReadPipe);

    project.simProgress.running = false;
    project.simProgress.percent = 100;

    if (exitCode != 0) {
        char msg[64];
        snprintf(msg, sizeof(msg), "Solver exited with code %lu", exitCode);
        project.simProgress.errors.push_back(msg);
        return false;
    }
#endif

    project.lastResultDir = workingDir;
    printf("[Solver] Completed successfully\n");
    return true;
}

// ─── Full Simulation Pipeline ───────────────────────────────────────────────

// ─── Project Validation ────────────────────────────────────────────────────

ValidationResult ValidateProject(const Project& project, const std::string& solver) {
    ValidationResult r;

    // Geometry
    if (project.model.vertices.empty() || project.model.groups.empty()) {
        r.errors.push_back("No geometry loaded");
        return r;
    }
    int totalFaces = 0;
    for (auto& g : project.model.groups) totalFaces += (int)g.faces.size();
    if (totalFaces == 0) r.errors.push_back("Geometry has 0 faces");

    // Check for degenerate faces
    int degenerate = 0;
    for (auto& g : project.model.groups) {
        for (auto& f : g.faces) {
            if (f.v[0] >= project.model.vertices.size() ||
                f.v[1] >= project.model.vertices.size() ||
                f.v[2] >= project.model.vertices.size()) { degenerate++; continue; }
            glm::vec3 a = project.model.vertices[f.v[0]];
            glm::vec3 b = project.model.vertices[f.v[1]];
            glm::vec3 c = project.model.vertices[f.v[2]];
            float area = 0.5f * glm::length(glm::cross(b - a, c - a));
            if (area < 1e-10f) degenerate++;
        }
    }
    if (degenerate > 0) r.warnings.push_back(std::to_string(degenerate) + " degenerate face(s) found");

    // Sources
    if (project.sources.empty()) {
        r.errors.push_back("No sound sources defined");
    }
    for (auto& src : project.sources) {
        if (!src.active) continue;
        auto& p = src.position;
        if (p.x < project.model.bbMin.x || p.x > project.model.bbMax.x ||
            p.y < project.model.bbMin.y || p.y > project.model.bbMax.y ||
            p.z < project.model.bbMin.z || p.z > project.model.bbMax.z) {
            r.warnings.push_back("Source '" + src.name + "' may be outside geometry bounding box");
        }
    }

    // Receivers
    for (auto& rcv : project.punctualReceivers) {
        auto& p = rcv.position;
        if (p.x < project.model.bbMin.x || p.x > project.model.bbMax.x ||
            p.y < project.model.bbMin.y || p.y > project.model.bbMax.y ||
            p.z < project.model.bbMin.z || p.z > project.model.bbMax.z) {
            r.warnings.push_back("Receiver '" + rcv.name + "' may be outside geometry bounding box");
        }
    }

    // Materials
    int unassigned = 0;
    for (int gi = 0; gi < (int)project.model.groups.size(); gi++) {
        if (project.groupMaterialMap.find(gi) == project.groupMaterialMap.end()) unassigned++;
    }
    if (unassigned > 0) r.warnings.push_back(std::to_string(unassigned) + " surface group(s) have no material assigned (using default)");

    // Frequency bands
    const bool* bands = (solver == "spps") ? project.sppsConfig.freqBands : project.tcrConfig.freqBands;
    int numBands = 0;
    for (int i = 0; i < 27; i++) if (bands[i]) numBands++;
    if (numBands == 0) r.errors.push_back("No frequency bands selected");

    // Solver-specific
    if (solver == "spps") {
        if (project.sppsConfig.particlesPerSource <= 0) r.errors.push_back("Particles per source must be > 0");
        if (project.sppsConfig.timeStep <= 0) r.errors.push_back("Time step must be > 0");
        if (project.sppsConfig.simLength <= 0) r.errors.push_back("Simulation length must be > 0");
        if (project.sppsConfig.receiverRadius <= 0) r.errors.push_back("Receiver radius must be > 0");
    }

    return r;
}

bool RunFullSimulation(const std::string& solver, const std::string& workingDir,
                       Project& project,
                       std::function<void(float, const std::string&)> progressCallback) {
    // Clean stale results from previous run to avoid loading old data on failure
    if (fs::exists(workingDir)) {
        std::error_code ec;
        fs::remove_all(workingDir, ec);
    }
    fs::create_directories(workingDir);

    project.simProgress = {};
    project.simProgress.running = true;

    // ── Validate before doing anything ──────────────────────────────────────
    auto validation = ValidateProject(project, solver);
    for (auto& w : validation.warnings) {
        project.simProgress.warnings.push_back(w);
        printf("[Validate] Warning: %s\n", w.c_str());
    }
    if (!validation.ok()) {
        for (auto& e : validation.errors)
            project.simProgress.errors.push_back("Validation: " + e);
        project.simProgress.running = false;
        return false;
    }

    auto scaledCallback = [&](float pct, const std::string& msg) {
        if (progressCallback) progressCallback(pct, msg);
    };

    // Phase 1: Export mesh files
    if (progressCallback) progressCallback(0, "Exporting mesh...");
    std::string cbinPath = workingDir + "/model.cbin";
    if (!WriteMeshBinary(project, cbinPath)) {
        project.simProgress.errors.push_back("Failed to write model.cbin");
        project.simProgress.running = false;
        return false;
    }

    // Phase 2: Write POLY for TetGen
    std::string polyPath = workingDir + "/model.poly";
    if (!WritePoly(project, polyPath)) {
        project.simProgress.errors.push_back("Failed to write model.poly");
        project.simProgress.running = false;
        return false;
    }
    if (progressCallback) progressCallback(5, "Mesh files exported");

    // Phase 3: Run TetGen
    std::string tetgenExe = FindSolverExe("tetgen");
    if (!tetgenExe.empty()) {
        if (progressCallback) progressCallback(10, "Running TetGen...");
        std::string cmd = "\"" + tetgenExe + "\" -pq" + std::to_string(project.meshConfig.qualityRatio) + " -A -n -Y -T1e-7" + (project.meshConfig.useVolumeConstraint ? " -a" + std::to_string(project.meshConfig.maxVolume) : "") + " \"" + polyPath + "\"";
        printf("[Sim] Running TetGen: %s\n", cmd.c_str());

#ifdef _WIN32
        std::vector<std::string> output;
        int ec = 0;
        RunProcess(cmd, workingDir, output, ec);
        for (auto& line : output) project.simProgress.log.push_back("[tetgen] " + line);
        if (ec != 0) {
            // Check if TetGen produced output files despite non-zero exit code
            bool hasOutput = fs::exists(workingDir + "/model.1.node") && fs::exists(workingDir + "/model.1.ele");
            if (!hasOutput) {
                project.simProgress.errors.push_back("TetGen failed with exit code: " + std::to_string(ec));
                project.simProgress.running = false;
                return false;
            }
            project.simProgress.warnings.push_back("TetGen returned warnings (exit code " + std::to_string(ec) + ") but produced output — continuing");
        }
#endif
    } else {
        project.simProgress.errors.push_back("tetgen.exe not found");
        project.simProgress.running = false;
        return false;
    }
    if (progressCallback) progressCallback(15, "Meshing complete");

    // Phase 3b: Generate .neigh file if missing (re-run TetGen with -n on existing mesh)
    if (!fs::exists(workingDir + "/model.1.neigh") && fs::exists(workingDir + "/model.1.node")) {
        // TetGen may have skipped neighbor generation — try again with just -rn on existing mesh
        project.simProgress.log.push_back("[tetgen] Regenerating neighbor info...");
    }

    // Phase 3c: Convert TetGen output to .mbin
    if (progressCallback) progressCallback(18, "Converting to .mbin...");
    std::string tetgenBase = workingDir + "/model";
    std::string mbinPath = workingDir + "/tetramesh.mbin";
    if (fs::exists(workingDir + "/model.1.node")) {
        ConvertTetGenToMbin(tetgenBase, mbinPath, project);
    }
    if (progressCallback) progressCallback(20, "Mesh conversion complete");

    // Phase 4: Run the solver
    if (progressCallback) progressCallback(25, "Launching " + solver + " solver...");
    bool result = RunSolver(solver, workingDir, project, [&](float pct, const std::string& msg) {
        // Scale solver progress to 25-95 range
        float scaled = 25.0f + pct * 0.70f;
        if (progressCallback) progressCallback(scaled, msg);
    });

    if (progressCallback) progressCallback(100, result ? "Simulation complete" : "Simulation failed");

    project.simProgress.running = false;
    return result;
}

// ─── Save Project ───────────────────────────────────────────────────────────

bool SaveProject(const Project& project, const std::string& path) {
    std::ofstream f(path);
    if (!f.is_open()) return false;

    f << "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
    f << "<isimpa_project version=\"2\" gui=\"newgui\">\n";

    // ── Environment ─────────────────────────────────────────────────────────
    auto& env = project.environment;
    f << "  <environment temperature=\"" << env.temperature
      << "\" pressure=\"" << env.pressure
      << "\" humidity=\"" << env.humidity
      << "\" z0=\"" << env.z0
      << "\" meteoProfile=\"" << env.meteoProfile
      << "\" aLog=\"" << env.aLog
      << "\" bLin=\"" << env.bLin
      << "\" customAtmoAbsorption=\"" << (env.customAtmoAbsorption ? 1 : 0)
      << "\" atmoAbsorptionValue=\"" << env.atmoAbsorptionValue
      << "\"/>\n";

    // ── SPPS Config ─────────────────────────────────────────────────────────
    auto& spps = project.sppsConfig;
    f << "  <spps_config"
      << " calcMethod=\"" << spps.calcMethod << "\""
      << " particlesPerSource=\"" << spps.particlesPerSource << "\""
      << " particlesDisplay=\"" << spps.particlesDisplay << "\""
      << " timeStep=\"" << spps.timeStep << "\""
      << " simLength=\"" << spps.simLength << "\""
      << " receiverRadius=\"" << spps.receiverRadius << "\""
      << " randomSeed=\"" << spps.randomSeed << "\""
      << " atmoAbsorption=\"" << (spps.atmoAbsorption ? 1 : 0) << "\""
      << " fittingDiffusion=\"" << (spps.fittingDiffusion ? 1 : 0) << "\""
      << " directFieldOnly=\"" << (spps.directFieldOnly ? 1 : 0) << "\""
      << " transmission=\"" << (spps.transmission ? 1 : 0) << "\""
      << " extinctionDb=\"" << spps.extinctionDb << "\""
      << " echogramPerSource=\"" << (spps.echogramPerSource ? 1 : 0) << "\""
      << " surfaceExportPerBand=\"" << (spps.surfaceExportPerBand ? 1 : 0) << "\""
      << " surfaceExportType=\"" << spps.surfaceExportType << "\""
      << ">\n";
    f << "    <freqBands>";
    for (int i = 0; i < 27; i++) f << (spps.freqBands[i] ? "1" : "0") << (i < 26 ? "," : "");
    f << "</freqBands>\n";
    f << "  </spps_config>\n";

    // ── TCR Config ──────────────────────────────────────────────────────────
    auto& tcr = project.tcrConfig;
    f << "  <tcr_config"
      << " atmoAbsorption=\"" << (tcr.atmoAbsorption ? 1 : 0) << "\""
      << ">\n";
    f << "    <freqBands>";
    for (int i = 0; i < 27; i++) f << (tcr.freqBands[i] ? "1" : "0") << (i < 26 ? "," : "");
    f << "</freqBands>\n";
    f << "  </tcr_config>\n";

    // ── Materials ───────────────────────────────────────────────────────────
    f << "  <materials>\n";
    for (auto& mat : project.materials) {
        f << "    <material id=\"" << mat.id
          << "\" name=\"" << EscapeXml(mat.name)
          << "\" r=\"" << mat.color.r << "\" g=\"" << mat.color.g << "\" b=\"" << mat.color.b
          << "\" density=\"" << mat.density
          << "\" resistivity=\"" << mat.resistivity
          << "\" bilateral=\"" << (mat.bilateral ? 1 : 0) << "\">\n";
        for (int i = 0; i < 27; i++) {
            if (mat.bands[i].absorption > 0 || mat.bands[i].diffusion > 0) {
                f << "      <band idx=\"" << i
                  << "\" absorption=\"" << mat.bands[i].absorption
                  << "\" diffusion=\"" << mat.bands[i].diffusion
                  << "\" transmission=\"" << mat.bands[i].transmission
                  << "\" diffusionLaw=\"" << mat.bands[i].diffusionLaw << "\"/>\n";
            }
        }
        f << "    </material>\n";
    }
    f << "  </materials>\n";

    // ── Geometry ────────────────────────────────────────────────────────────
    f << "  <geometry>\n";
    f << "    <vertices count=\"" << project.model.vertices.size() << "\">\n";
    for (int i = 0; i < (int)project.model.vertices.size(); i++) {
        auto& v = project.model.vertices[i];
        f << "      <v x=\"" << v.x << "\" y=\"" << v.y << "\" z=\"" << v.z << "\"/>\n";
    }
    f << "    </vertices>\n";

    f << "    <groups>\n";
    for (int gi = 0; gi < (int)project.model.groups.size(); gi++) {
        auto& grp = project.model.groups[gi];
        int matIdx = -1;
        auto it = project.groupMaterialMap.find(gi);
        if (it != project.groupMaterialMap.end()) matIdx = it->second;

        f << "      <group name=\"" << EscapeXml(grp.name)
          << "\" materialId=\"" << matIdx
          << "\" r=\"" << grp.color.r << "\" g=\"" << grp.color.g << "\" b=\"" << grp.color.b << "\">\n";
        for (auto& face : grp.faces) {
            f << "        <f a=\"" << face.v[0] << "\" b=\"" << face.v[1] << "\" c=\"" << face.v[2] << "\"/>\n";
        }
        f << "      </group>\n";
    }
    f << "    </groups>\n";
    f << "  </geometry>\n";

    // ── Sources ─────────────────────────────────────────────────────────────
    f << "  <sources>\n";
    for (auto& src : project.sources) {
        f << "    <source id=\"" << src.id
          << "\" name=\"" << EscapeXml(src.name)
          << "\" x=\"" << src.position.x << "\" y=\"" << src.position.y << "\" z=\"" << src.position.z
          << "\" dx=\"" << src.direction.x << "\" dy=\"" << src.direction.y << "\" dz=\"" << src.direction.z
          << "\" active=\"" << (src.active ? 1 : 0)
          << "\" power=\"" << src.globalPowerDb
          << "\" directivity=\"" << src.directivity
          << "\" delay=\"" << src.delaySeconds
          << "\" cr=\"" << src.displayColor.r << "\" cg=\"" << src.displayColor.g << "\" cb=\"" << src.displayColor.b
          << "\">\n";
        f << "      <spectrum>";
        for (int i = 0; i < 27; i++) f << src.spectrum[i] << (i < 26 ? "," : "");
        f << "</spectrum>\n";
        f << "    </source>\n";
    }
    f << "  </sources>\n";

    // ── Receivers ───────────────────────────────────────────────────────────
    f << "  <receivers>\n";
    for (auto& rcv : project.punctualReceivers) {
        f << "    <receiver id=\"" << rcv.id
          << "\" name=\"" << EscapeXml(rcv.name)
          << "\" x=\"" << rcv.position.x << "\" y=\"" << rcv.position.y << "\" z=\"" << rcv.position.z
          << "\" dx=\"" << rcv.direction.x << "\" dy=\"" << rcv.direction.y << "\" dz=\"" << rcv.direction.z
          << "\" active=\"" << (rcv.active ? 1 : 0)
          << "\" directivity=\"" << rcv.directivity
          << "\" cr=\"" << rcv.displayColor.r << "\" cg=\"" << rcv.displayColor.g << "\" cb=\"" << rcv.displayColor.b
          << "\"/>\n";
    }
    f << "  </receivers>\n";

    // ── Surface Receivers ───────────────────────────────────────────────────
    f << "  <surface_receivers>\n";
    for (auto& sr : project.surfaceReceivers) {
        f << "    <surface_receiver id=\"" << sr.id
          << "\" name=\"" << EscapeXml(sr.name)
          << "\" type=\"" << (sr.type == SurfaceReceiver::Plane ? "plane" : "scene")
          << "\" ax=\"" << sr.vertexA.x << "\" ay=\"" << sr.vertexA.y << "\" az=\"" << sr.vertexA.z
          << "\" bx=\"" << sr.vertexB.x << "\" by=\"" << sr.vertexB.y << "\" bz=\"" << sr.vertexB.z
          << "\" cx=\"" << sr.vertexC.x << "\" cy=\"" << sr.vertexC.y << "\" cz=\"" << sr.vertexC.z
          << "\" resolution=\"" << sr.gridResolution
          << "\"/>\n";
    }
    f << "  </surface_receivers>\n";

    // ── Encumbrances ────────────────────────────────────────────────────────
    f << "  <encumbrances>\n";
    for (auto& enc : project.encumbrances) {
        f << "    <encumbrance id=\"" << enc.id
          << "\" name=\"" << EscapeXml(enc.name)
          << "\" active=\"" << (enc.active ? 1 : 0)
          << "\" type=\"" << (enc.type == Encumbrance::Cuboid ? "cuboid" : "facegroup")
          << "\" bminx=\"" << enc.boxMin.x << "\" bminy=\"" << enc.boxMin.y << "\" bminz=\"" << enc.boxMin.z
          << "\" bmaxx=\"" << enc.boxMax.x << "\" bmaxy=\"" << enc.boxMax.y << "\" bmaxz=\"" << enc.boxMax.z
          << "\" cr=\"" << enc.color.r << "\" cg=\"" << enc.color.g << "\" cb=\"" << enc.color.b
          << "\" opacity=\"" << enc.opacity
          << "\" description=\"" << EscapeXml(enc.description)
          << "\">\n";
        for (int i = 0; i < 27; i++) {
            f << "      <band idx=\"" << i
              << "\" absorption=\"" << enc.bands[i].absorption
              << "\" meanFreePath=\"" << enc.bands[i].meanFreePath
              << "\" diffusionLaw=\"" << enc.bands[i].diffusionLaw << "\"/>\n";
        }
        f << "    </encumbrance>\n";
    }
    f << "  </encumbrances>\n";

    // ── Volumes ─────────────────────────────────────────────────────────────
    f << "  <volumes>\n";
    for (auto& vol : project.volumes) {
        f << "    <volume id=\"" << vol.id
          << "\" name=\"" << EscapeXml(vol.name)
          << "\" bminx=\"" << vol.boxMin.x << "\" bminy=\"" << vol.boxMin.y << "\" bminz=\"" << vol.boxMin.z
          << "\" bmaxx=\"" << vol.boxMax.x << "\" bmaxy=\"" << vol.boxMax.y << "\" bmaxz=\"" << vol.boxMax.z
          << "\" temperature=\"" << vol.temperature
          << "\" humidity=\"" << vol.humidity
          << "\" useCustomAtmo=\"" << (vol.useCustomAtmo ? 1 : 0)
          << "\"/>\n";
    }
    f << "  </volumes>\n";

    // ── Background noise ────────────────────────────────────────────────────
    f << "  <background_noise enabled=\"" << (project.backgroundNoise.enabled ? 1 : 0) << "\">\n";
    f << "    <spectrum>";
    for (int i = 0; i < 27; i++) f << project.backgroundNoise.spectrum[i] << (i < 26 ? "," : "");
    f << "</spectrum>\n";
    f << "  </background_noise>\n";

    f << "</isimpa_project>\n";
    f.flush();
    bool writeOk = f.good();
    f.close();

    if (!writeOk) {
        fprintf(stderr, "[Save] Write error on project: %s\n", path.c_str());
        return false;
    }
    printf("[Save] Wrote project: %s\n", path.c_str());
    return true;
}

// ─── Load Project ───────────────────────────────────────────────────────────

// Minimal XML attribute parser (no external dependency needed)
static std::string UnescapeXml(const std::string& s) {
    std::string out;
    out.reserve(s.size());
    for (size_t i = 0; i < s.size(); i++) {
        if (s[i] == '&') {
            if (s.compare(i, 5, "&amp;") == 0)       { out += '&'; i += 4; }
            else if (s.compare(i, 4, "&lt;") == 0)    { out += '<'; i += 3; }
            else if (s.compare(i, 4, "&gt;") == 0)    { out += '>'; i += 3; }
            else if (s.compare(i, 6, "&quot;") == 0)  { out += '"'; i += 5; }
            else if (s.compare(i, 6, "&apos;") == 0)  { out += '\''; i += 5; }
            else out += s[i];
        } else {
            out += s[i];
        }
    }
    return out;
}

static std::string GetAttr(const std::string& line, const std::string& attr) {
    std::string search = attr + "=\"";
    size_t pos = line.find(search);
    if (pos == std::string::npos) return "";
    pos += search.size();
    size_t end = line.find('"', pos);
    if (end == std::string::npos) return "";
    return UnescapeXml(line.substr(pos, end - pos));
}

static float GetAttrF(const std::string& line, const std::string& attr, float def = 0.0f) {
    std::string v = GetAttr(line, attr);
    return v.empty() ? def : std::stof(v);
}

static int GetAttrI(const std::string& line, const std::string& attr, int def = 0) {
    std::string v = GetAttr(line, attr);
    return v.empty() ? def : std::stoi(v);
}

bool LoadProject(Project& project, const std::string& path) {
    std::ifstream f(path);
    if (!f.is_open()) return false;

    // Check project version
    std::string firstLine;
    while (std::getline(f, firstLine)) {
        if (firstLine.find("<isimpa_project") != std::string::npos) {
            std::string ver = GetAttr(firstLine, "version");
            if (!ver.empty() && ver != "2") {
                fprintf(stderr, "[Load] Warning: project version '%s' (expected '2'), loading may be incomplete\n", ver.c_str());
            }
            break;
        }
        if (firstLine.find("<?xml") == std::string::npos) break; // not our format
    }
    f.clear();
    f.seekg(0);

    project.NewProject();

    std::string line;
    enum Section { None, Materials, MatBands, Geometry, Vertices, Groups, GroupFaces,
                   Sources, SourceSpectrum, Receivers, SurfReceivers } section = None;

    int currentMatId = -1;
    int currentGroupIdx = -1;
    int currentSrcIdx = -1;

    while (std::getline(f, line)) {
        // Trim leading whitespace
        size_t start = line.find_first_not_of(" \t\r\n");
        if (start == std::string::npos) continue;
        line = line.substr(start);

        // ── Environment ─────────────────────────────────────────────────
        if (line.find("<environment ") != std::string::npos) {
            project.environment.temperature = GetAttrF(line, "temperature", 20.0f);
            project.environment.pressure = GetAttrF(line, "pressure", 101325.0f);
            project.environment.humidity = GetAttrF(line, "humidity", 50.0f);
            project.environment.z0 = GetAttrF(line, "z0", 0.02f);
            project.environment.meteoProfile = GetAttrI(line, "meteoProfile", 2);
            project.environment.aLog = GetAttrF(line, "aLog");
            project.environment.bLin = GetAttrF(line, "bLin");
            project.environment.customAtmoAbsorption = GetAttrI(line, "customAtmoAbsorption", 0) != 0;
            project.environment.atmoAbsorptionValue = GetAttrF(line, "atmoAbsorptionValue");
        }

        // ── SPPS Config ─────────────────────────────────────────────────
        if (line.find("<spps_config") != std::string::npos) {
            project.sppsConfig.calcMethod = GetAttrI(line, "calcMethod", 1);
            project.sppsConfig.particlesPerSource = GetAttrI(line, "particlesPerSource", 100000);
            project.sppsConfig.particlesDisplay = GetAttrI(line, "particlesDisplay", 500);
            project.sppsConfig.timeStep = GetAttrF(line, "timeStep", 0.005f);
            project.sppsConfig.simLength = GetAttrF(line, "simLength", 2.0f);
            project.sppsConfig.receiverRadius = GetAttrF(line, "receiverRadius", 0.5f);
            project.sppsConfig.randomSeed = GetAttrI(line, "randomSeed", 0);
            project.sppsConfig.atmoAbsorption = GetAttrI(line, "atmoAbsorption", 1) != 0;
            project.sppsConfig.fittingDiffusion = GetAttrI(line, "fittingDiffusion", 0) != 0;
            project.sppsConfig.directFieldOnly = GetAttrI(line, "directFieldOnly", 0) != 0;
            project.sppsConfig.transmission = GetAttrI(line, "transmission", 0) != 0;
            project.sppsConfig.extinctionDb = GetAttrI(line, "extinctionDb", 60);
            project.sppsConfig.echogramPerSource = GetAttrI(line, "echogramPerSource", 0) != 0;
            project.sppsConfig.surfaceExportPerBand = GetAttrI(line, "surfaceExportPerBand", 0) != 0;
            project.sppsConfig.surfaceExportType = GetAttrI(line, "surfaceExportType", 0);
            section = None; // mark we're in spps context for freqBands
        }

        // ── TCR Config ──────────────────────────────────────────────────
        if (line.find("<tcr_config") != std::string::npos) {
            project.tcrConfig.atmoAbsorption = GetAttrI(line, "atmoAbsorption", 1) != 0;
        }

        // Parse freqBands for whichever config section we're in
        if (line.find("<freqBands>") != std::string::npos) {
            size_t s = line.find('>') + 1;
            size_t e = line.find('<', s);
            if (e != std::string::npos) {
                std::string bands = line.substr(s, e - s);
                std::istringstream ss(bands);
                std::string token;
                int idx = 0;
                // Determine which config gets the bands based on context
                // The freqBands tag appears inside spps_config or tcr_config
                bool isTcr = false;
                // Check if we just parsed tcr_config (look back for context)
                // Simple heuristic: if spps was already loaded, this is tcr
                bool sppsHasBands = false;
                for (int i = 0; i < 27; i++) {
                    if (project.sppsConfig.freqBands[i]) { sppsHasBands = true; break; }
                }
                // First freqBands goes to SPPS, second to TCR
                if (sppsHasBands) isTcr = true;

                while (std::getline(ss, token, ',') && idx < 27) {
                    bool val = (token == "1");
                    if (isTcr)
                        project.tcrConfig.freqBands[idx] = val;
                    else
                        project.sppsConfig.freqBands[idx] = val;
                    idx++;
                }
            }
        }

        // ── Materials ───────────────────────────────────────────────────
        if (line.find("<materials>") != std::string::npos) { section = Materials; project.materials.clear(); }
        if (line.find("</materials>") != std::string::npos) section = None;

        if (section == Materials && line.find("<material ") != std::string::npos) {
            AcousticMaterial mat;
            mat.id = GetAttrI(line, "id");
            mat.name = GetAttr(line, "name");
            mat.color.r = GetAttrF(line, "r", 0.5f);
            mat.color.g = GetAttrF(line, "g", 0.5f);
            mat.color.b = GetAttrF(line, "b", 0.5f);
            mat.density = GetAttrF(line, "density");
            mat.resistivity = GetAttrF(line, "resistivity");
            mat.bilateral = GetAttrI(line, "bilateral", 1) != 0;
            project.materials.push_back(mat);
            currentMatId = (int)project.materials.size() - 1;
            section = MatBands;
        }
        if (section == MatBands && line.find("<band ") != std::string::npos) {
            int idx = GetAttrI(line, "idx");
            if (idx >= 0 && idx < 27 && currentMatId >= 0) {
                project.materials[currentMatId].bands[idx].absorption = GetAttrF(line, "absorption");
                project.materials[currentMatId].bands[idx].diffusion = GetAttrF(line, "diffusion");
                project.materials[currentMatId].bands[idx].transmission = GetAttrF(line, "transmission");
                project.materials[currentMatId].bands[idx].diffusionLaw = GetAttrI(line, "diffusionLaw");
            }
        }
        if (section == MatBands && line.find("</material>") != std::string::npos) section = Materials;

        // ── Geometry ────────────────────────────────────────────────────
        if (line.find("<geometry>") != std::string::npos) section = Geometry;
        if (line.find("</geometry>") != std::string::npos) section = None;
        if (line.find("<vertices ") != std::string::npos) section = Vertices;
        if (line.find("</vertices>") != std::string::npos) section = Geometry;

        if (section == Vertices && line.find("<v ") != std::string::npos) {
            glm::vec3 v;
            v.x = GetAttrF(line, "x");
            v.y = GetAttrF(line, "y");
            v.z = GetAttrF(line, "z");
            project.model.vertices.push_back(v);
        }

        if (line.find("<groups>") != std::string::npos) section = Groups;
        if (line.find("</groups>") != std::string::npos) section = Geometry;

        if (section == Groups && line.find("<group ") != std::string::npos) {
            SurfaceGroup grp;
            grp.name = GetAttr(line, "name");
            grp.color.r = GetAttrF(line, "r", 0.5f);
            grp.color.g = GetAttrF(line, "g", 0.5f);
            grp.color.b = GetAttrF(line, "b", 0.5f);
            int matId = GetAttrI(line, "materialId", -1);
            project.model.groups.push_back(grp);
            currentGroupIdx = (int)project.model.groups.size() - 1;
            if (matId >= 0) {
                project.groupMaterialMap[currentGroupIdx] = matId;
                project.model.groups[currentGroupIdx].materialId = matId;
            }
            section = GroupFaces;
        }
        if (section == GroupFaces && line.find("<f ") != std::string::npos) {
            Face face = {};
            face.v[0] = (uint32_t)GetAttrI(line, "a");
            face.v[1] = (uint32_t)GetAttrI(line, "b");
            face.v[2] = (uint32_t)GetAttrI(line, "c");
            project.model.groups[currentGroupIdx].faces.push_back(face);
        }
        if (section == GroupFaces && line.find("</group>") != std::string::npos) section = Groups;

        // ── Sources ─────────────────────────────────────────────────────
        if (line.find("<sources>") != std::string::npos) section = Sources;
        if (line.find("</sources>") != std::string::npos) section = None;

        if (section == Sources && line.find("<source ") != std::string::npos) {
            SoundSource src;
            src.id = GetAttrI(line, "id");
            src.name = GetAttr(line, "name");
            src.position.x = GetAttrF(line, "x");
            src.position.y = GetAttrF(line, "y");
            src.position.z = GetAttrF(line, "z");
            src.direction.x = GetAttrF(line, "dx");
            src.direction.y = GetAttrF(line, "dy", 0);
            src.direction.z = GetAttrF(line, "dz", 1);
            src.active = GetAttrI(line, "active", 1) != 0;
            src.globalPowerDb = GetAttrF(line, "power", 80.0f);
            src.directivity = GetAttrI(line, "directivity");
            src.delaySeconds = GetAttrF(line, "delay");
            src.displayColor.r = GetAttrF(line, "cr", 1.0f);
            src.displayColor.g = GetAttrF(line, "cg", 0.6f);
            src.displayColor.b = GetAttrF(line, "cb", 0.1f);
            project.sources.push_back(src);
            currentSrcIdx = (int)project.sources.size() - 1;
            section = SourceSpectrum;
        }
        if (section == SourceSpectrum && line.find("<spectrum>") != std::string::npos) {
            size_t s = line.find('>') + 1;
            size_t e = line.find('<', s);
            if (e != std::string::npos) {
                std::string vals = line.substr(s, e - s);
                std::istringstream ss(vals);
                std::string token;
                int idx = 0;
                while (std::getline(ss, token, ',') && idx < 27) {
                    project.sources[currentSrcIdx].spectrum[idx++] = std::stof(token);
                }
            }
        }
        if (section == SourceSpectrum && line.find("</source>") != std::string::npos) section = Sources;

        // ── Receivers ───────────────────────────────────────────────────
        if (line.find("<receivers>") != std::string::npos) section = Receivers;
        if (line.find("</receivers>") != std::string::npos) section = None;

        if (section == Receivers && line.find("<receiver ") != std::string::npos) {
            PunctualReceiver rcv;
            rcv.id = GetAttrI(line, "id");
            rcv.name = GetAttr(line, "name");
            rcv.position.x = GetAttrF(line, "x");
            rcv.position.y = GetAttrF(line, "y");
            rcv.position.z = GetAttrF(line, "z");
            rcv.direction.x = GetAttrF(line, "dx");
            rcv.direction.y = GetAttrF(line, "dy");
            rcv.direction.z = GetAttrF(line, "dz", 1);
            rcv.active = GetAttrI(line, "active", 1) != 0;
            rcv.directivity = GetAttrI(line, "directivity");
            rcv.displayColor.r = GetAttrF(line, "cr", 0.2f);
            rcv.displayColor.g = GetAttrF(line, "cg", 0.9f);
            rcv.displayColor.b = GetAttrF(line, "cb", 0.4f);
            project.punctualReceivers.push_back(rcv);
        }

        // ── Surface Receivers ───────────────────────────────────────────
        if (line.find("<surface_receivers>") != std::string::npos) section = SurfReceivers;
        if (line.find("</surface_receivers>") != std::string::npos) section = None;

        if (section == SurfReceivers && line.find("<surface_receiver ") != std::string::npos) {
            SurfaceReceiver sr;
            sr.id = GetAttrI(line, "id");
            sr.name = GetAttr(line, "name");
            sr.type = (GetAttr(line, "type") == "plane") ? SurfaceReceiver::Plane : SurfaceReceiver::Scene;
            sr.vertexA = glm::vec3(GetAttrF(line, "ax"), GetAttrF(line, "ay"), GetAttrF(line, "az"));
            sr.vertexB = glm::vec3(GetAttrF(line, "bx"), GetAttrF(line, "by"), GetAttrF(line, "bz"));
            sr.vertexC = glm::vec3(GetAttrF(line, "cx"), GetAttrF(line, "cy"), GetAttrF(line, "cz"));
            sr.gridResolution = GetAttrF(line, "resolution", 0.5f);
            project.surfaceReceivers.push_back(sr);
        }

        // ── Encumbrances ───────────────────────────────────────────────
        static int currentEncIdx = -1;
        if (line.find("<encumbrances>") != std::string::npos) { /* enter encumbrance section */ }
        if (line.find("<encumbrance ") != std::string::npos && line.find("<encumbrances>") == std::string::npos) {
            Encumbrance enc;
            enc.id = GetAttrI(line, "id");
            enc.name = GetAttr(line, "name");
            enc.active = GetAttrI(line, "active", 1) != 0;
            enc.type = (GetAttr(line, "type") == "cuboid") ? Encumbrance::Cuboid : Encumbrance::FaceGroup;
            enc.boxMin = glm::vec3(GetAttrF(line, "bminx"), GetAttrF(line, "bminy"), GetAttrF(line, "bminz"));
            enc.boxMax = glm::vec3(GetAttrF(line, "bmaxx"), GetAttrF(line, "bmaxy"), GetAttrF(line, "bmaxz"));
            enc.color.r = GetAttrF(line, "cr", 0.8f);
            enc.color.g = GetAttrF(line, "cg", 0.5f);
            enc.color.b = GetAttrF(line, "cb", 0.2f);
            enc.opacity = GetAttrF(line, "opacity", 0.3f);
            enc.description = GetAttr(line, "description");
            project.encumbrances.push_back(enc);
            currentEncIdx = (int)project.encumbrances.size() - 1;
        }
        if (currentEncIdx >= 0 && line.find("<band ") != std::string::npos &&
            line.find("meanFreePath") != std::string::npos) {
            int idx = GetAttrI(line, "idx");
            if (idx >= 0 && idx < 27) {
                project.encumbrances[currentEncIdx].bands[idx].absorption = GetAttrF(line, "absorption", 0.3f);
                project.encumbrances[currentEncIdx].bands[idx].meanFreePath = GetAttrF(line, "meanFreePath", 1.5f);
                project.encumbrances[currentEncIdx].bands[idx].diffusionLaw = GetAttrI(line, "diffusionLaw");
            }
        }
        if (line.find("</encumbrance>") != std::string::npos) currentEncIdx = -1;

        // ── Volumes ────────────────────────────────────────────────────
        if (line.find("<volume ") != std::string::npos && line.find("<volumes>") == std::string::npos) {
            VolumeDefinition vol;
            vol.id = GetAttrI(line, "id");
            vol.name = GetAttr(line, "name");
            vol.boxMin = glm::vec3(GetAttrF(line, "bminx"), GetAttrF(line, "bminy"), GetAttrF(line, "bminz"));
            vol.boxMax = glm::vec3(GetAttrF(line, "bmaxx"), GetAttrF(line, "bmaxy"), GetAttrF(line, "bmaxz"));
            vol.temperature = GetAttrF(line, "temperature", 20.0f);
            vol.humidity = GetAttrF(line, "humidity", 50.0f);
            vol.useCustomAtmo = GetAttrI(line, "useCustomAtmo", 0) != 0;
            project.volumes.push_back(vol);
        }

        // ── Background noise ───────────────────────────────────────────
        if (line.find("<background_noise ") != std::string::npos) {
            project.backgroundNoise.enabled = GetAttrI(line, "enabled", 0) != 0;
        }
        if (line.find("<spectrum>") != std::string::npos && project.backgroundNoise.enabled) {
            // Only parse if we're in the background_noise context (check for enabled above)
            size_t s = line.find('>') + 1;
            size_t e = line.find('<', s);
            if (e != std::string::npos) {
                std::string vals = line.substr(s, e - s);
                std::istringstream ss(vals);
                std::string token;
                int idx = 0;
                while (std::getline(ss, token, ',') && idx < 27) {
                    project.backgroundNoise.spectrum[idx++] = std::stof(token);
                }
            }
        }
    }

    f.close();

    // Post-load: compute normals and bounds
    project.model.ComputeNormals();
    project.model.ComputeBounds();
    for (int i = 0; i < (int)project.model.groups.size(); i++) {
        project.model.ComputeGroupArea(i);
    }

    printf("[Load] Loaded project: %s (%zu verts, %zu groups, %zu sources, %zu receivers)\n",
           path.c_str(), project.model.vertices.size(), project.model.groups.size(),
           project.sources.size(), project.punctualReceivers.size());
    return true;
}

// ─── Load Original I-Simpa .proj File ────────────────────────────────────────

bool LoadOriginalProject(Project& project, const std::string& projPath) {
    // .proj files are ZIP archives — extract using miniz (no external tools)
    std::string tempDir = "isimpa_temp_proj";
    fs::remove_all(tempDir);
    fs::create_directories(tempDir);

    {
        fprintf(stderr, "[PROJ] Opening ZIP with miniz: %s\n", projPath.c_str());
        fflush(stderr);
        mz_zip_archive zip = {};
        if (!mz_zip_reader_init_file(&zip, projPath.c_str(), 0)) {
            fprintf(stderr, "[PROJ] Failed to open ZIP: %s (error: %s)\n",
                    projPath.c_str(), mz_zip_get_error_string(mz_zip_get_last_error(&zip)));
            return false;
        }

        int numFiles = (int)mz_zip_reader_get_num_files(&zip);
        printf("[PROJ] ZIP contains %d files\n", numFiles);

        for (int i = 0; i < numFiles; i++) {
            mz_zip_archive_file_stat stat;
            if (!mz_zip_reader_file_stat(&zip, i, &stat)) continue;
            if (stat.m_is_directory) continue;

            std::string outPath = tempDir + "/" + stat.m_filename;
            // Create parent directories
            fs::create_directories(fs::path(outPath).parent_path());

            if (!mz_zip_reader_extract_to_file(&zip, i, outPath.c_str(), 0)) {
                fprintf(stderr, "[PROJ] Failed to extract: %s\n", stat.m_filename);
            }
        }

        mz_zip_reader_end(&zip);
    }

    // Look for projet_config.xml (may be in subdirectory)
    std::string xmlPath;
    for (auto& entry : fs::recursive_directory_iterator(tempDir)) {
        if (entry.path().filename() == "projet_config.xml") {
            xmlPath = entry.path().string();
            break;
        }
    }

    if (xmlPath.empty()) {
        fprintf(stderr, "[PROJ] projet_config.xml not found in: %s\n", projPath.c_str());
        fs::remove_all(tempDir);
        return false;
    }

    printf("[PROJ] Found config: %s\n", xmlPath.c_str());

    // Look for sceneMesh.bin
    std::string binPath;
    for (auto& entry : fs::recursive_directory_iterator(tempDir)) {
        if (entry.path().filename() == "sceneMesh.bin") {
            binPath = entry.path().string();
            break;
        }
    }

    // Parse the XML config to extract basic project info
    project.NewProject();

    std::ifstream f(xmlPath);
    if (!f.is_open()) { fs::remove_all(tempDir); return false; }

    std::string line;
    while (std::getline(f, line)) {
        size_t start = line.find_first_not_of(" \t\r\n");
        if (start == std::string::npos) continue;
        line = line.substr(start);

        // Parse sources
        if (line.find("<source ") != std::string::npos || line.find("<source>") != std::string::npos) {
            SoundSource src;
            std::string name = GetAttr(line, "name");
            if (!name.empty()) src.name = name;
            src.position = ISimToGL(glm::vec3(GetAttrF(line, "x"), GetAttrF(line, "y"), GetAttrF(line, "z")));
            src.globalPowerDb = GetAttrF(line, "db", 80.0f);
            src.id = (int)project.sources.size() + 1;
            if (src.name.empty()) src.name = "Source " + std::to_string(src.id);
            project.sources.push_back(src);
        }

        // Parse receivers
        if (line.find("<recepteurp ") != std::string::npos) {
            PunctualReceiver rcv;
            std::string name = GetAttr(line, "name");
            if (!name.empty()) rcv.name = name;
            rcv.position = ISimToGL(glm::vec3(GetAttrF(line, "x"), GetAttrF(line, "y"), GetAttrF(line, "z")));
            rcv.id = (int)project.punctualReceivers.size() + 1;
            if (rcv.name.empty()) rcv.name = "Receiver " + std::to_string(rcv.id);
            project.punctualReceivers.push_back(rcv);
        }

        // Parse environment
        if (line.find("<condition_atmospherique") != std::string::npos ||
            line.find("<environment") != std::string::npos) {
            project.environment.temperature = GetAttrF(line, "temperature", 20.0f);
            project.environment.humidity = GetAttrF(line, "humidite", GetAttrF(line, "humidity", 50.0f));
            project.environment.pressure = GetAttrF(line, "pression", GetAttrF(line, "pressure", 101325.0f));
        }
    }
    f.close();

    // Try to load sceneMesh.bin (I-Simpa binary format)
    if (!binPath.empty()) {
        std::ifstream binFile(binPath, std::ios::binary);
        if (binFile.is_open()) {
            // Read header (version 1.0)
            uint32_t major, minor;
            binFile.read((char*)&major, 4);
            binFile.read((char*)&minor, 4);

            if (major == 1) { // Accept version 1.x (1.0 and 1.2)
                // Process nodes
                auto readNodeHeader = [&]() -> bool {
                    uint16_t nodeType;
                    uint16_t pad;
                    uint32_t firstSon, nextBrother;
                    binFile.read((char*)&nodeType, 2);
                    binFile.read((char*)&pad, 2);
                    binFile.read((char*)&firstSon, 4);
                    binFile.read((char*)&nextBrother, 4);

                    if (nodeType == 0) {
                        // Vertices node
                        uint32_t nbVerts;
                        binFile.read((char*)&nbVerts, 4);
                        project.model.vertices.reserve(nbVerts);
                        for (uint32_t i = 0; i < nbVerts; i++) {
                            float x, y, z;
                            binFile.read((char*)&x, 4);
                            binFile.read((char*)&y, 4);
                            binFile.read((char*)&z, 4);
                            // I-Simpa coords -> GL: X->X, Z->Y, -Y->Z
                            project.model.vertices.push_back(ISimToGL(glm::vec3(x, y, z)));
                        }
                    } else if (nodeType == 1) {
                        // Group node
                        char groupName[255];
                        binFile.read(groupName, 255);
                        char nullByte;
                        binFile.read(&nullByte, 1);
                        uint32_t nbFaces;
                        binFile.read((char*)&nbFaces, 4);

                        SurfaceGroup grp;
                        grp.name = std::string(groupName);
                        if (grp.name.empty()) grp.name = "Group";
                        grp.color = glm::vec3(0.5f, 0.5f, 0.55f);

                        for (uint32_t fi = 0; fi < nbFaces; fi++) {
                            uint32_t a, b, c, matId;
                            int32_t idRs, idEn;
                            binFile.read((char*)&a, 4);
                            binFile.read((char*)&b, 4);
                            binFile.read((char*)&c, 4);
                            binFile.read((char*)&matId, 4);
                            binFile.read((char*)&idRs, 4);
                            binFile.read((char*)&idEn, 4);
                            Face face = {};
                            face.v[0] = a; face.v[1] = b; face.v[2] = c;
                            face.materialId = (int16_t)matId;
                            grp.faces.push_back(face);
                        }
                        project.model.groups.push_back(grp);
                    }

                    // Move to next brother if needed
                    if (nextBrother > 0 && nextBrother > (uint32_t)binFile.tellg()) {
                        binFile.seekg(nextBrother);
                        return true; // more nodes
                    }
                    return nextBrother != 0;
                };

                while (binFile.good() && !binFile.eof()) {
                    if (!readNodeHeader()) break;
                }
            }
            binFile.close();
        }
    }

    // Post-load
    if (!project.model.vertices.empty()) {
        project.model.ComputeNormals();
        project.model.ComputeBounds();
        for (int i = 0; i < (int)project.model.groups.size(); i++)
            project.model.ComputeGroupArea(i);
    }

    // Cleanup temp
    fs::remove_all(tempDir);

    printf("[PROJ] Loaded original I-Simpa project: %s (%zu verts, %zu groups, %zu sources)\n",
           projPath.c_str(), project.model.vertices.size(), project.model.groups.size(),
           project.sources.size());
    return !project.model.vertices.empty() || !project.sources.empty();
}

// ─── Material Import/Export ──────────────────────────────────────────────────

bool ExportMaterials(const std::vector<AcousticMaterial>& materials, const std::string& path) {
    std::ofstream f(path);
    if (!f.is_open()) return false;

    f << "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
    f << "<material_database version=\"1\" count=\"" << materials.size() << "\">\n";
    for (auto& mat : materials) {
        f << "  <material name=\"" << mat.name
          << "\" r=\"" << mat.color.r << "\" g=\"" << mat.color.g << "\" b=\"" << mat.color.b
          << "\" resistivity=\"" << mat.resistivity
          << "\" bilateral=\"" << (mat.bilateral ? 1 : 0) << "\">\n";
        for (int i = 0; i < 27; i++) {
            f << "    <band freq=\"" << (int)AcousticMaterial::BAND_FREQS[i]
              << "\" absorption=\"" << mat.bands[i].absorption
              << "\" diffusion=\"" << mat.bands[i].diffusion
              << "\" transmission=\"" << mat.bands[i].transmission
              << "\" law=\"" << mat.bands[i].diffusionLaw << "\"/>\n";
        }
        f << "  </material>\n";
    }
    f << "</material_database>\n";
    f.close();
    return true;
}

bool ImportMaterials(std::vector<AcousticMaterial>& materials, const std::string& path) {
    std::ifstream f(path);
    if (!f.is_open()) return false;

    std::string line;
    AcousticMaterial currentMat;
    bool inMaterial = false;
    int bandIdx = 0;

    while (std::getline(f, line)) {
        size_t start = line.find_first_not_of(" \t\r\n");
        if (start == std::string::npos) continue;
        line = line.substr(start);

        if (line.find("<material ") != std::string::npos) {
            currentMat = {};
            currentMat.id = (int)materials.size();
            currentMat.name = GetAttr(line, "name");
            currentMat.color.r = GetAttrF(line, "r", 0.5f);
            currentMat.color.g = GetAttrF(line, "g", 0.5f);
            currentMat.color.b = GetAttrF(line, "b", 0.5f);
            currentMat.resistivity = GetAttrF(line, "resistivity");
            currentMat.bilateral = GetAttrI(line, "bilateral", 1) != 0;
            inMaterial = true;
            bandIdx = 0;
        }
        if (inMaterial && line.find("<band ") != std::string::npos) {
            if (bandIdx < 27) {
                currentMat.bands[bandIdx].absorption = GetAttrF(line, "absorption");
                currentMat.bands[bandIdx].diffusion = GetAttrF(line, "diffusion");
                currentMat.bands[bandIdx].transmission = GetAttrF(line, "transmission");
                currentMat.bands[bandIdx].diffusionLaw = GetAttrI(line, "law");
                bandIdx++;
            }
        }
        if (inMaterial && line.find("</material>") != std::string::npos) {
            currentMat.id = (int)materials.size();
            materials.push_back(currentMat);
            inMaterial = false;
        }
    }

    f.close();
    return true;
}

// ─── Import I-Simpa appconst.xml material library ──────────────────────────

bool ImportMatlibFromAppConst(std::vector<AcousticMaterial>& materials, const std::string& path) {
    // appconst.xml uses a different XML structure than our material_database format
    // Each material is wrapped in <appmateriau> with nested <materiau> and per-band <p> elements
    std::ifstream f(path);
    if (!f.is_open()) return false;

    std::string line;
    AcousticMaterial currentMat;
    bool inMaterial = false;
    int currentFreq = 0;

    while (std::getline(f, line)) {
        size_t start = line.find_first_not_of(" \t\r\n");
        if (start == std::string::npos) continue;
        line = line.substr(start);

        // Material block start
        if (line.find("<appmateriau ") != std::string::npos) {
            currentMat = {};
            currentMat.name = GetAttr(line, "name");
            currentMat.id = (int)materials.size();
            inMaterial = true;
        }

        // Frequency band (identified by numeric name like "500" or "1000")
        if (inMaterial && line.find("<p name=\"") != std::string::npos) {
            std::string name = GetAttr(line, "name");
            // Try to parse as frequency
            float freq = 0;
            if (sscanf(name.c_str(), "%f", &freq) == 1 && freq >= 50 && freq <= 20000) {
                // Find the closest band index
                int bestIdx = 0;
                float bestDist = 99999;
                for (int i = 0; i < 27; i++) {
                    float d = fabsf(AcousticMaterial::BAND_FREQS[i] - freq);
                    if (d < bestDist) { bestDist = d; bestIdx = i; }
                }
                currentFreq = bestIdx;
            }
            // Check for absorption/diffusion/transmission values
            std::string label = GetAttr(line, "label");
            std::string value = GetAttr(line, "value");
            if (!value.empty() && currentFreq >= 0 && currentFreq < 27) {
                float v = std::stof(value);
                if (label.find("Absorption") != std::string::npos || label.find("absorb") != std::string::npos)
                    currentMat.bands[currentFreq].absorption = v;
                else if (label.find("Diffusion") != std::string::npos || label.find("diffusion") != std::string::npos)
                    currentMat.bands[currentFreq].diffusion = v;
                else if (label.find("Loss") != std::string::npos || label.find("affaiblissement") != std::string::npos)
                    currentMat.bands[currentFreq].transmission = v;
            }
        }

        if (inMaterial && line.find("</appmateriau>") != std::string::npos) {
            if (!currentMat.name.empty() && currentMat.AverageAbsorption() > 0.001f) {
                currentMat.id = (int)materials.size();
                // Assign a color based on absorption
                float avg = currentMat.AverageAbsorption();
                currentMat.color = glm::vec3(0.4f + avg * 0.3f, 0.4f - avg * 0.2f, 0.5f - avg * 0.3f);
                materials.push_back(currentMat);
            }
            inMaterial = false;
        }
    }

    f.close();
    printf("[MatLib] Imported %zu materials from appconst: %s\n", materials.size(), path.c_str());
    return !materials.empty();
}

} // namespace isimpa
