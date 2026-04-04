#include "mesh/scene_model.h"
#include <fstream>
#include <sstream>
#include <iostream>
#include <cctype>
#include <algorithm>
#include <unordered_map>
#include <functional>

namespace isimpa {

// Minimal PLY parser supporting ASCII and binary_little_endian
// Compatible with I-Simpa PLY files (vertices + faces + optional layers)

struct PlyProperty {
    std::string name;
    std::string type;        // float, double, int, uchar, etc.
    bool isList = false;
    std::string countType;   // for list properties
    std::string elemType;    // for list properties
};

struct PlyElement {
    std::string name;
    int count = 0;
    std::vector<PlyProperty> properties;
};

static int TypeSize(const std::string& t) {
    if (t == "float" || t == "float32" || t == "int" || t == "int32" || t == "uint") return 4;
    if (t == "double" || t == "float64") return 8;
    if (t == "uchar" || t == "uint8" || t == "char" || t == "int8") return 1;
    if (t == "short" || t == "int16" || t == "ushort" || t == "uint16") return 2;
    return 4;
}

static float ReadFloatAscii(std::istringstream& ss) {
    float v = 0; ss >> v; return v;
}

static int ReadIntAscii(std::istringstream& ss) {
    int v = 0; ss >> v; return v;
}

bool LoadPLY(const std::string& path, SceneModel& model) {
    std::ifstream file(path, std::ios::binary);
    if (!file.is_open()) {
        fprintf(stderr, "[PLY] Cannot open: %s\n", path.c_str());
        return false;
    }

    // Parse header
    std::string line;
    std::getline(file, line);
    if (line.find("ply") == std::string::npos) {
        fprintf(stderr, "[PLY] Not a PLY file: %s\n", path.c_str());
        return false;
    }

    std::string format = "ascii";
    std::vector<PlyElement> elements;

    while (std::getline(file, line)) {
        // Strip \r
        if (!line.empty() && line.back() == '\r') line.pop_back();

        std::istringstream ss(line);
        std::string token;
        ss >> token;

        if (token == "end_header") break;

        if (token == "format") {
            ss >> format; // ascii, binary_little_endian, binary_big_endian
        }
        else if (token == "element") {
            PlyElement elem;
            ss >> elem.name >> elem.count;
            elements.push_back(elem);
        }
        else if (token == "property") {
            if (elements.empty()) continue;
            PlyProperty prop;
            std::string typeOrList;
            ss >> typeOrList;

            if (typeOrList == "list") {
                prop.isList = true;
                ss >> prop.countType >> prop.elemType >> prop.name;
            } else {
                prop.type = typeOrList;
                ss >> prop.name;
            }
            elements.back().properties.push_back(prop);
        }
    }

    // Find vertex and face elements
    int vertexElemIdx = -1, faceElemIdx = -1, layerElemIdx = -1;
    for (int i = 0; i < (int)elements.size(); i++) {
        if (elements[i].name == "vertex") vertexElemIdx = i;
        else if (elements[i].name == "face") faceElemIdx = i;
        else if (elements[i].name == "layer") layerElemIdx = i;
    }

    if (vertexElemIdx < 0 || faceElemIdx < 0) {
        fprintf(stderr, "[PLY] Missing vertex or face element\n");
        return false;
    }

    model.Clear();

    // Find property indices for x, y, z in vertex element
    auto& vertElem = elements[vertexElemIdx];
    int xIdx = -1, yIdx = -1, zIdx = -1;
    for (int i = 0; i < (int)vertElem.properties.size(); i++) {
        if (vertElem.properties[i].name == "x") xIdx = i;
        if (vertElem.properties[i].name == "y") yIdx = i;
        if (vertElem.properties[i].name == "z") zIdx = i;
    }

    // Find layer_id property in face element
    auto& faceElem = elements[faceElemIdx];
    int layerIdIdx = -1;
    for (int i = 0; i < (int)faceElem.properties.size(); i++) {
        if (faceElem.properties[i].name == "layer_id") layerIdIdx = i;
    }

    // ASCII parsing (most common for I-Simpa exports)
    if (format == "ascii" || format.find("ascii") != std::string::npos) {

        // Read elements in order
        for (int ei = 0; ei < (int)elements.size(); ei++) {
            auto& elem = elements[ei];

            if (ei == vertexElemIdx) {
                model.vertices.reserve(elem.count);
                for (int i = 0; i < elem.count; i++) {
                    std::getline(file, line);
                    if (!line.empty() && line.back() == '\r') line.pop_back();
                    std::istringstream ss(line);

                    std::vector<float> vals(elem.properties.size());
                    for (int p = 0; p < (int)elem.properties.size(); p++) {
                        ss >> vals[p];
                    }

                    float x = (xIdx >= 0) ? vals[xIdx] : 0;
                    float y = (yIdx >= 0) ? vals[yIdx] : 0;
                    float z = (zIdx >= 0) ? vals[zIdx] : 0;

                    // I-Simpa coord transform: real X -> X, real Z -> Y, real -Y -> Z
                    model.vertices.push_back(glm::vec3(x, z, -y));
                }
            }
            else if (ei == faceElemIdx) {
                // Temporary face storage with layer indices
                struct TempFace { uint32_t v[3]; int layerId; };
                std::vector<TempFace> tempFaces;
                tempFaces.reserve(elem.count);

                for (int i = 0; i < elem.count; i++) {
                    std::getline(file, line);
                    if (!line.empty() && line.back() == '\r') line.pop_back();
                    std::istringstream ss(line);

                    int facesBeforeThisPoly = (int)tempFaces.size();
                    int layerId = 0;
                    for (int p = 0; p < (int)elem.properties.size(); p++) {
                        if (elem.properties[p].isList) {
                            int count = 0;
                            ss >> count;
                            if (count < 0 || count > 1000000) count = 0;
                            std::vector<uint32_t> indices(count);
                            for (int j = 0; j < count; j++) ss >> indices[j];

                            bool valid = true;
                            for (int j = 0; j < count; j++) {
                                if (indices[j] >= model.vertices.size()) { valid = false; break; }
                            }

                            if (valid) {
                                for (int j = 1; j + 1 < count; j++) {
                                    TempFace f;
                                    f.v[0] = indices[0];
                                    f.v[1] = indices[j];
                                    f.v[2] = indices[j + 1];
                                    f.layerId = layerId;
                                    tempFaces.push_back(f);
                                }
                            }
                        } else {
                            int val;
                            ss >> val;
                            if (p == layerIdIdx) layerId = val;
                        }
                    }

                    // Fix layer ID for faces added by this polygon
                    // (layerId property may appear after the vertex list property)
                    for (int j = facesBeforeThisPoly; j < (int)tempFaces.size(); j++) {
                        tempFaces[j].layerId = layerId;
                    }
                }

                // Read layer names if present
                std::vector<std::string> layerNames;
                if (layerElemIdx >= 0 && layerElemIdx > faceElemIdx) {
                    // Will be read in the layer element pass below
                }

                // Organize into groups
                if (layerIdIdx >= 0) {
                    // Determine max layer ID (capped to prevent excessive allocation)
                    int maxLayer = 0;
                    for (auto& f : tempFaces) maxLayer = std::max(maxLayer, f.layerId);
                    if (maxLayer > 10000) {
                        fprintf(stderr, "[PLY] Warning: layer_id %d too large, capping to 10000\n", maxLayer);
                        maxLayer = 10000;
                        for (auto& f : tempFaces) { if (f.layerId > maxLayer) f.layerId = 0; }
                    }

                    model.groups.resize(maxLayer + 1);
                    for (int i = 0; i <= maxLayer; i++) {
                        model.groups[i].name = "Group " + std::to_string(i);
                        // Assign distinct colors
                        float hue = (float)i / (float)(maxLayer + 1);
                        model.groups[i].color = glm::vec3(
                            0.3f + 0.4f * sinf(hue * 6.28f),
                            0.3f + 0.4f * sinf(hue * 6.28f + 2.09f),
                            0.3f + 0.4f * sinf(hue * 6.28f + 4.19f)
                        );
                    }

                    for (auto& tf : tempFaces) {
                        Face f;
                        f.v[0] = tf.v[0]; f.v[1] = tf.v[1]; f.v[2] = tf.v[2];
                        model.groups[tf.layerId].faces.push_back(f);
                    }
                } else {
                    // Single group
                    SurfaceGroup grp;
                    grp.name = "model";
                    grp.color = glm::vec3(0.5f, 0.5f, 0.55f);
                    for (auto& tf : tempFaces) {
                        Face f;
                        f.v[0] = tf.v[0]; f.v[1] = tf.v[1]; f.v[2] = tf.v[2];
                        grp.faces.push_back(f);
                    }
                    model.groups.push_back(grp);
                }
            }
            else if (ei == layerElemIdx) {
                // Read layer names
                for (int i = 0; i < elem.count; i++) {
                    std::getline(file, line);
                    if (!line.empty() && line.back() == '\r') line.pop_back();
                    // Layer name: may be "length char char ..." (I-Simpa encoded) or plain string
                    std::string layerName = line;
                    while (!layerName.empty() && std::isspace(layerName.back())) layerName.pop_back();

                    // Decode I-Simpa char-list format: "7 99 101 105 108 105 110 103" -> "ceiling"
                    {
                        std::istringstream lss(layerName);
                        int strLen = 0;
                        if (lss >> strLen && strLen > 0 && strLen < 256) {
                            std::string decoded;
                            bool allChars = true;
                            for (int ci = 0; ci < strLen; ci++) {
                                int charVal;
                                if (!(lss >> charVal) || charVal < 32 || charVal > 126) {
                                    allChars = false; break;
                                }
                                decoded += (char)charVal;
                            }
                            if (allChars && !decoded.empty()) layerName = decoded;
                        }
                    }

                    if (i < (int)model.groups.size()) {
                        model.groups[i].name = layerName;
                    }
                }
            }
            else {
                // Skip unknown elements
                for (int i = 0; i < elem.count; i++) {
                    std::getline(file, line);
                }
            }
        }
    }
    else {
        // Binary little-endian parsing
        for (int ei = 0; ei < (int)elements.size(); ei++) {
            auto& elem = elements[ei];

            if (ei == vertexElemIdx) {
                model.vertices.reserve(elem.count);
                for (int i = 0; i < elem.count; i++) {
                    std::vector<float> vals(elem.properties.size(), 0);
                    for (int p = 0; p < (int)elem.properties.size(); p++) {
                        auto& prop = elem.properties[p];
                        int sz = TypeSize(prop.type);
                        if (sz == 4) {
                            float fv; file.read((char*)&fv, 4);
                            vals[p] = fv;
                        } else if (sz == 8) {
                            double dv; file.read((char*)&dv, 8);
                            vals[p] = (float)dv;
                        } else if (sz == 1) {
                            uint8_t bv; file.read((char*)&bv, 1);
                            vals[p] = (float)bv;
                        } else if (sz == 2) {
                            int16_t sv; file.read((char*)&sv, 2);
                            vals[p] = (float)sv;
                        }
                    }
                    float x = (xIdx >= 0) ? vals[xIdx] : 0;
                    float y = (yIdx >= 0) ? vals[yIdx] : 0;
                    float z = (zIdx >= 0) ? vals[zIdx] : 0;
                    model.vertices.push_back(glm::vec3(x, z, -y));
                }
            }
            else if (ei == faceElemIdx) {
                SurfaceGroup grp;
                grp.name = "model";
                grp.color = glm::vec3(0.5f, 0.5f, 0.55f);

                for (int i = 0; i < elem.count; i++) {
                    for (int p = 0; p < (int)elem.properties.size(); p++) {
                        auto& prop = elem.properties[p];
                        if (prop.isList) {
                            int32_t count = 0;
                            int csz = TypeSize(prop.countType);
                            if (csz == 1) { uint8_t c; file.read((char*)&c, 1); count = c; }
                            else if (csz == 4) { file.read((char*)&count, 4); }
                            if (count < 0 || count > 1000000) count = 0;

                            std::vector<uint32_t> indices(count);
                            int esz = TypeSize(prop.elemType);
                            for (int j = 0; j < count; j++) {
                                if (esz == 4) { int32_t v; file.read((char*)&v, 4); indices[j] = (uint32_t)v; }
                                else if (esz == 2) { uint16_t v; file.read((char*)&v, 2); indices[j] = v; }
                                else if (esz == 1) { uint8_t v; file.read((char*)&v, 1); indices[j] = v; }
                            }

                            // Validate indices and triangulate
                            bool valid = true;
                            for (int j = 0; j < count; j++) {
                                if (indices[j] >= model.vertices.size()) { valid = false; break; }
                            }
                            if (valid) {
                                for (int j = 1; j + 1 < count; j++) {
                                    Face f;
                                    f.v[0] = indices[0]; f.v[1] = indices[j]; f.v[2] = indices[j+1];
                                    grp.faces.push_back(f);
                                }
                            }
                        } else {
                            // Skip non-list properties
                            int sz = TypeSize(prop.type);
                            file.seekg(sz, std::ios::cur);
                        }
                    }
                }
                model.groups.push_back(grp);
            }
            else {
                // Skip unknown binary elements
                for (int i = 0; i < elem.count; i++) {
                    for (auto& prop : elem.properties) {
                        if (prop.isList) {
                            uint8_t count = 0;
                            int csz = TypeSize(prop.countType);
                            file.read((char*)&count, csz);
                            file.seekg(count * TypeSize(prop.elemType), std::ios::cur);
                        } else {
                            file.seekg(TypeSize(prop.type), std::ios::cur);
                        }
                    }
                }
            }
        }
    }

    model.ComputeBounds();
    model.ComputeNormals();
    for (int i = 0; i < (int)model.groups.size(); i++) model.ComputeGroupArea(i);

    printf("[PLY] Loaded: %zu vertices, %zu groups from %s\n",
           model.vertices.size(), model.groups.size(), path.c_str());
    return true;
}

// ─── STL File Loader ────────────────────────────────────────────────────────

bool LoadSTL(const std::string& path, SceneModel& model) {
    // Try binary STL first (most common), fall back to ASCII
    std::ifstream file(path, std::ios::binary);
    if (!file.is_open()) {
        fprintf(stderr, "[STL] Cannot open: %s\n", path.c_str());
        return false;
    }

    model.Clear();

    // Read 80-byte header
    char header[80];
    file.read(header, 80);
    if (!file.good()) return false;

    // Read triangle count
    uint32_t numTriangles = 0;
    file.read((char*)&numTriangles, 4);
    if (!file.good()) return false;

    // Check if this is actually ASCII by checking if header starts with "solid"
    // and the file size doesn't match binary expectation
    file.seekg(0, std::ios::end);
    auto fileSize = file.tellg();
    auto expectedBinarySize = 84 + numTriangles * 50;

    bool isBinary = ((std::streamoff)fileSize == (std::streamoff)expectedBinarySize) &&
                    numTriangles > 0 && numTriangles < 50000000;

    if (isBinary) {
        // ── Binary STL ──────────────────────────────────────────────────────
        file.seekg(84);

        // Use a vertex dedup map for indexed mesh
        struct Vec3Hash {
            size_t operator()(const glm::vec3& v) const {
                auto h1 = std::hash<float>()(v.x);
                auto h2 = std::hash<float>()(v.y);
                auto h3 = std::hash<float>()(v.z);
                return h1 ^ (h2 << 11) ^ (h3 << 22);
            }
        };
        struct Vec3Eq {
            bool operator()(const glm::vec3& a, const glm::vec3& b) const {
                return fabsf(a.x - b.x) < 1e-6f && fabsf(a.y - b.y) < 1e-6f && fabsf(a.z - b.z) < 1e-6f;
            }
        };
        std::unordered_map<glm::vec3, uint32_t, Vec3Hash, Vec3Eq> vertMap;

        SurfaceGroup grp;
        grp.name = "STL model";
        grp.color = glm::vec3(0.5f, 0.5f, 0.55f);

        for (uint32_t i = 0; i < numTriangles; i++) {
            float normalBuf[3], v1[3], v2[3], v3[3];
            uint16_t attrByteCount;

            file.read((char*)normalBuf, 12);
            file.read((char*)v1, 12);
            file.read((char*)v2, 12);
            file.read((char*)v3, 12);
            file.read((char*)&attrByteCount, 2);

            if (!file.good()) break;

            // Convert I-Simpa coords: X -> X, Z -> Y, -Y -> Z
            glm::vec3 verts[3] = {
                {v1[0], v1[2], -v1[1]},
                {v2[0], v2[2], -v2[1]},
                {v3[0], v3[2], -v3[1]}
            };

            Face face = {};
            for (int j = 0; j < 3; j++) {
                auto it = vertMap.find(verts[j]);
                if (it != vertMap.end()) {
                    face.v[j] = it->second;
                } else {
                    uint32_t idx = (uint32_t)model.vertices.size();
                    model.vertices.push_back(verts[j]);
                    vertMap[verts[j]] = idx;
                    face.v[j] = idx;
                }
            }
            grp.faces.push_back(face);
        }

        model.groups.push_back(grp);
    } else {
        // ── ASCII STL ───────────────────────────────────────────────────────
        file.seekg(0);
        file.close();

        std::ifstream afile(path);
        if (!afile.is_open()) return false;

        SurfaceGroup grp;
        grp.name = "STL model";
        grp.color = glm::vec3(0.5f, 0.5f, 0.55f);

        std::string line;
        while (std::getline(afile, line)) {
            // Trim
            size_t s = line.find_first_not_of(" \t\r\n");
            if (s == std::string::npos) continue;
            line = line.substr(s);

            if (line.rfind("facet normal", 0) == 0) {
                // Read 3 vertex lines inside the facet
                float verts[3][3];
                int vcount = 0;

                while (std::getline(afile, line) && vcount < 3) {
                    s = line.find_first_not_of(" \t\r\n");
                    if (s == std::string::npos) continue;
                    line = line.substr(s);

                    if (line.rfind("vertex", 0) == 0) {
                        std::istringstream ss(line.substr(6));
                        ss >> verts[vcount][0] >> verts[vcount][1] >> verts[vcount][2];
                        vcount++;
                    }
                    if (line.rfind("endfacet", 0) == 0) break;
                }

                if (vcount == 3) {
                    Face face = {};
                    for (int j = 0; j < 3; j++) {
                        glm::vec3 v(verts[j][0], verts[j][2], -verts[j][1]);
                        face.v[j] = (uint32_t)model.vertices.size();
                        model.vertices.push_back(v);
                    }
                    grp.faces.push_back(face);
                }
            }
        }
        afile.close();
        model.groups.push_back(grp);
    }

    if (model.vertices.empty()) {
        fprintf(stderr, "[STL] No geometry found in: %s\n", path.c_str());
        return false;
    }

    model.ComputeBounds();
    model.ComputeNormals();
    for (int i = 0; i < (int)model.groups.size(); i++) model.ComputeGroupArea(i);

    printf("[STL] Loaded: %zu vertices, %zu faces from %s\n",
           model.vertices.size(), model.groups[0].faces.size(), path.c_str());
    return true;
}

// ─── 3DS File Loader ────────────────────────────────────────────────────────
// Minimal 3DS (3D Studio) binary format loader
// Reads MAIN_CHUNK -> EDITOR_CHUNK -> OBJECT_BLOCK -> TRIMESH -> vertices + faces

bool Load3DS(const std::string& path, SceneModel& model) {
    std::ifstream file(path, std::ios::binary);
    if (!file.is_open()) {
        fprintf(stderr, "[3DS] Cannot open: %s\n", path.c_str());
        return false;
    }

    model.Clear();

    // 3DS chunk IDs
    enum : uint16_t {
        CHUNK_MAIN     = 0x4D4D,
        CHUNK_EDITOR   = 0x3D3D,
        CHUNK_OBJECT   = 0x4000,
        CHUNK_TRIMESH  = 0x4100,
        CHUNK_VERTLIST = 0x4110,
        CHUNK_FACELIST = 0x4120
    };

    auto readU16 = [&]() -> uint16_t { uint16_t v = 0; file.read((char*)&v, 2); return v; };
    auto readU32 = [&]() -> uint32_t { uint32_t v = 0; file.read((char*)&v, 4); return v; };
    auto readFloat = [&]() -> float { float v = 0; file.read((char*)&v, 4); return v; };

    auto readString = [&]() -> std::string {
        std::string s;
        char c;
        while (file.get(c) && c != '\0') s += c;
        return s;
    };

    // Track vertex offset for multiple objects
    uint32_t vertexOffset = 0;

    std::function<void(uint32_t)> parseChunks = [&](uint32_t endPos) {
        while ((uint32_t)file.tellg() < endPos && file.good()) {
            uint16_t chunkId = readU16();
            uint32_t chunkLen = readU32();
            uint32_t chunkEnd = (uint32_t)file.tellg() - 6 + chunkLen;

            switch (chunkId) {
                case CHUNK_MAIN:
                case CHUNK_EDITOR:
                    parseChunks(chunkEnd);
                    break;

                case CHUNK_OBJECT: {
                    std::string objName = readString();
                    vertexOffset = (uint32_t)model.vertices.size();
                    parseChunks(chunkEnd);
                    break;
                }

                case CHUNK_TRIMESH:
                    parseChunks(chunkEnd);
                    break;

                case CHUNK_VERTLIST: {
                    uint16_t numVerts = readU16();
                    for (uint16_t i = 0; i < numVerts; i++) {
                        float x = readFloat();
                        float y = readFloat();
                        float z = readFloat();
                        // 3DS uses Z-up, same as I-Simpa internally
                        // Convert to GL (Y-up): X->X, Z->Y, -Y->Z
                        model.vertices.push_back(glm::vec3(x, z, -y));
                    }
                    break;
                }

                case CHUNK_FACELIST: {
                    uint16_t numFaces = readU16();
                    SurfaceGroup grp;
                    grp.name = "Object " + std::to_string(model.groups.size());
                    float hue = (float)model.groups.size() * 0.37f;
                    grp.color = glm::vec3(
                        0.3f + 0.4f * sinf(hue * 6.28f),
                        0.3f + 0.4f * sinf(hue * 6.28f + 2.09f),
                        0.3f + 0.4f * sinf(hue * 6.28f + 4.19f));

                    for (uint16_t i = 0; i < numFaces; i++) {
                        uint16_t a = readU16();
                        uint16_t b = readU16();
                        uint16_t c = readU16();
                        uint16_t flags = readU16(); // face flags (edge visibility)
                        (void)flags;

                        uint32_t va = vertexOffset + a;
                        uint32_t vb = vertexOffset + b;
                        uint32_t vc = vertexOffset + c;
                        if (va < model.vertices.size() && vb < model.vertices.size() && vc < model.vertices.size()) {
                            Face face = {};
                            face.v[0] = va;
                            face.v[1] = vb;
                            face.v[2] = vc;
                            grp.faces.push_back(face);
                        }
                    }

                    model.groups.push_back(grp);

                    // Skip remaining sub-chunks (material assignments etc.)
                    if ((uint32_t)file.tellg() < chunkEnd)
                        file.seekg(chunkEnd);
                    break;
                }

                default:
                    // Skip unknown chunk
                    file.seekg(chunkEnd);
                    break;
            }

            if ((uint32_t)file.tellg() > chunkEnd) break;
        }
    };

    // Get file size
    file.seekg(0, std::ios::end);
    uint32_t fileSize = (uint32_t)file.tellg();
    file.seekg(0);

    parseChunks(fileSize);
    file.close();

    if (model.vertices.empty()) {
        fprintf(stderr, "[3DS] No geometry found in: %s\n", path.c_str());
        return false;
    }

    // If no groups were created, create one default
    if (model.groups.empty()) {
        SurfaceGroup grp;
        grp.name = "3DS model";
        grp.color = glm::vec3(0.5f, 0.5f, 0.55f);
        model.groups.push_back(grp);
    }

    model.ComputeBounds();
    model.ComputeNormals();
    for (int i = 0; i < (int)model.groups.size(); i++) model.ComputeGroupArea(i);

    int totalFaces = 0;
    for (auto& g : model.groups) totalFaces += (int)g.faces.size();
    printf("[3DS] Loaded: %zu vertices, %d faces, %zu objects from %s\n",
           model.vertices.size(), totalFaces, model.groups.size(), path.c_str());
    return true;
}

// ─── OBJ (Wavefront) File Loader ────────────────────────────────────────────

bool LoadOBJ(const std::string& path, SceneModel& model) {
    std::ifstream file(path);
    if (!file.is_open()) {
        fprintf(stderr, "[OBJ] Cannot open: %s\n", path.c_str());
        return false;
    }

    model.Clear();

    // Current group
    SurfaceGroup currentGroup;
    currentGroup.name = "default";
    currentGroup.color = glm::vec3(0.5f, 0.5f, 0.55f);
    int groupCount = 0;

    std::string line;
    while (std::getline(file, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (line.empty() || line[0] == '#') continue;

        std::istringstream ss(line);
        std::string token;
        ss >> token;

        if (token == "v") {
            float x, y, z;
            ss >> x >> y >> z;
            // Store raw for now, will detect up-axis after parsing
            model.vertices.push_back(glm::vec3(x, y, z));
        }
        else if (token == "f") {
            // Parse face - handles "v", "v/vt", "v/vt/vn", "v//vn"
            std::vector<uint32_t> indices;
            std::string vertToken;
            while (ss >> vertToken) {
                int vi = 0;
                sscanf(vertToken.c_str(), "%d", &vi);
                if (vi < 0) vi = (int)model.vertices.size() + vi; // relative index (OBJ: -1 = last vertex)
                else vi = vi - 1; // 1-based to 0-based
                if (vi >= 0 && vi < (int)model.vertices.size()) {
                    indices.push_back((uint32_t)vi);
                }
            }
            // Triangulate (fan from first vertex)
            for (size_t i = 1; i + 1 < indices.size(); i++) {
                Face face = {};
                face.v[0] = indices[0];
                face.v[1] = indices[i];
                face.v[2] = indices[i + 1];
                currentGroup.faces.push_back(face);
            }
        }
        else if (token == "g" || token == "o") {
            // New group/object - flush current
            if (!currentGroup.faces.empty()) {
                model.groups.push_back(currentGroup);
                groupCount++;
            }
            currentGroup.faces.clear();
            ss >> currentGroup.name;
            if (currentGroup.name.empty()) currentGroup.name = "Group " + std::to_string(groupCount);
            float hue = (float)groupCount * 0.37f;
            currentGroup.color = glm::vec3(
                0.3f + 0.4f * sinf(hue * 6.28f),
                0.3f + 0.4f * sinf(hue * 6.28f + 2.09f),
                0.3f + 0.4f * sinf(hue * 6.28f + 4.19f));
        }
        // Skip vt, vn, mtllib, usemtl etc.
    }
    file.close();

    // Flush last group
    if (!currentGroup.faces.empty()) {
        model.groups.push_back(currentGroup);
    }

    if (model.vertices.empty() || model.groups.empty()) {
        fprintf(stderr, "[OBJ] No geometry found in: %s\n", path.c_str());
        return false;
    }

    // Detect up-axis: if Y extent > Z extent, assume Y-up (already GL convention)
    // Otherwise assume Z-up and convert: X->X, Z->Y, -Y->Z
    glm::vec3 mn = model.vertices[0], mx = model.vertices[0];
    for (auto& v : model.vertices) { mn = glm::min(mn, v); mx = glm::max(mx, v); }
    float yExtent = mx.y - mn.y;
    float zExtent = mx.z - mn.z;

    // Z-up detection: check if Z range is larger AND most vertices have small Y values
    // relative to Z (a room is wider/deeper in XZ and tall in Z if Z-up)
    float xExtent = mx.x - mn.x;
    bool zUp = (zExtent > yExtent * 1.2f) && (zExtent > xExtent * 0.3f);
    if (zUp) {
        printf("[OBJ] Detected Z-up coordinate system, converting\n");
        for (auto& v : model.vertices) {
            float x = v.x, y = v.y, z = v.z;
            v = glm::vec3(x, z, -y);
        }
    } else {
        printf("[OBJ] Detected Y-up coordinate system (native GL)\n");
    }

    model.ComputeBounds();
    model.ComputeNormals();
    for (int i = 0; i < (int)model.groups.size(); i++) model.ComputeGroupArea(i);

    int totalFaces = 0;
    for (auto& g : model.groups) totalFaces += (int)g.faces.size();
    printf("[OBJ] Loaded: %zu vertices, %d faces, %zu groups from %s\n",
           model.vertices.size(), totalFaces, model.groups.size(), path.c_str());
    return true;
}

} // namespace isimpa
