#include "project/project.h"
#include "viewport/viewport.h"
#include <cmath>
#include <cstdio>

namespace isimpa {

constexpr float AcousticMaterial::BAND_FREQS[27];

float AcousticMaterial::AverageAbsorption() const {
    float sum = 0;
    int count = 0;
    for (int i = 0; i < NUM_BANDS; i++) {
        sum += bands[i].absorption;
        count++;
    }
    return count > 0 ? sum / count : 0;
}

static Project g_project;
Project& GetProject() { return g_project; }

Project::Project() {
    LoadDefaultMaterials();
}

void Project::NewProject() {
    model.Clear();
    sources.clear();
    punctualReceivers.clear();
    surfaceReceivers.clear();
    encumbrances.clear();
    volumes.clear();
    directivityPatterns.clear();
    backgroundNoise = {};
    groupMaterialMap.clear();
    simProgress = {};
    lastResultDir.clear();
    m_nextSourceId = 1;
    m_nextReceiverId = 1;
    m_nextSurfRecId = 1;
    m_nextEncumbranceId = 1;
    m_nextVolumeId = 1;
}

void Project::CreateDefaultRoom(float w, float l, float h) {
    model = SceneModel::CreateBox(w, l, h);
    ViewportLoadModel(model);
}

void Project::LoadScenePLY(const std::string& path) {
    SceneModel newModel;
    if (LoadPLY(path, newModel)) {
        model = newModel;
        ViewportLoadModel(model);
    }
}

SoundSource& Project::AddSource() {
    SoundSource src;
    src.id = m_nextSourceId++;
    char name[32];
    snprintf(name, sizeof(name), "Source %d", src.id);
    src.name = name;

    // Place in center of room at 1.5m height
    if (!model.IsEmpty()) {
        src.position = model.center;
        src.position.y = 1.5f;
    }

    // Default white noise spectrum at globalPowerDb
    float perBand = src.globalPowerDb - 10.0f * log10f(27.0f);
    for (int i = 0; i < 27; i++) src.spectrum[i] = perBand;

    sources.push_back(src);
    return sources.back();
}

PunctualReceiver& Project::AddReceiver() {
    PunctualReceiver rcv;
    rcv.id = m_nextReceiverId++;
    char name[32];
    snprintf(name, sizeof(name), "Receiver %d", rcv.id);
    rcv.name = name;

    // Place offset from center at ear height
    if (!model.IsEmpty()) {
        rcv.position = model.center;
        rcv.position.y = 1.2f;
        rcv.position.x += 2.0f * (float)punctualReceivers.size();
    }

    punctualReceivers.push_back(rcv);
    return punctualReceivers.back();
}

SurfaceReceiver& Project::AddSurfaceReceiver() {
    SurfaceReceiver sr;
    sr.id = m_nextSurfRecId++;
    char name[32];
    snprintf(name, sizeof(name), "Surface Receiver %d", sr.id);
    sr.name = name;
    sr.type = SurfaceReceiver::Plane;
    sr.gridResolution = 0.5f;

    // Default: horizontal plane at ear height covering most of the room
    if (!model.IsEmpty()) {
        float margin = 0.5f;
        sr.vertexA = glm::vec3(model.bbMin.x + margin, 1.2f, model.bbMin.z + margin);
        sr.vertexB = glm::vec3(model.bbMax.x - margin, 1.2f, model.bbMin.z + margin);
        sr.vertexC = glm::vec3(model.bbMax.x - margin, 1.2f, model.bbMax.z - margin);
    } else {
        sr.vertexA = glm::vec3(0, 1.2f, 0);
        sr.vertexB = glm::vec3(5, 1.2f, 0);
        sr.vertexC = glm::vec3(5, 1.2f, 8);
    }

    surfaceReceivers.push_back(sr);
    return surfaceReceivers.back();
}

Encumbrance& Project::AddEncumbrance() {
    Encumbrance enc;
    enc.id = m_nextEncumbranceId++;
    char name[32];
    snprintf(name, sizeof(name), "Fitting Zone %d", enc.id);
    enc.name = name;

    // Default box in center of room
    if (!model.IsEmpty()) {
        glm::vec3 c = model.center;
        enc.boxMin = c - glm::vec3(1.0f, 0.0f, 1.0f);
        enc.boxMax = c + glm::vec3(1.0f, 1.5f, 1.0f);
    }

    encumbrances.push_back(enc);
    return encumbrances.back();
}

VolumeDefinition& Project::AddVolume() {
    VolumeDefinition vol;
    vol.id = m_nextVolumeId++;
    char name[32];
    snprintf(name, sizeof(name), "Volume %d", vol.id);
    vol.name = name;

    if (!model.IsEmpty()) {
        vol.boxMin = model.bbMin;
        vol.boxMax = model.bbMax;
    }

    volumes.push_back(vol);
    return volumes.back();
}

void Project::AssignMaterial(int groupIdx, int materialIdx) {
    if (groupIdx >= 0 && groupIdx < (int)model.groups.size() &&
        materialIdx >= 0 && materialIdx < (int)materials.size()) {
        groupMaterialMap[groupIdx] = materialIdx;
        model.groups[groupIdx].materialId = materialIdx;
        model.groups[groupIdx].color = materials[materialIdx].color;
        // Re-upload to GPU with new colors
        ViewportRefreshGPU();
    }
}

int Project::TotalFaces() const {
    int total = 0;
    for (auto& g : model.groups) total += (int)g.faces.size();
    return total;
}

void Project::LoadDefaultMaterials() {
    materials.clear();
    auto add = [&](const char* name, float r, float g, float b, float abs[7]) {
        AcousticMaterial mat;
        mat.id = (int)materials.size();
        mat.name = name;
        mat.color = {r, g, b};
        // Map 7 octave bands to 27 third-octave (interpolate)
        int octaveIdx[7] = {4, 7, 10, 13, 16, 19, 22}; // 125,250,500,1k,2k,4k,8k
        for (int i = 0; i < 7; i++) {
            mat.bands[octaveIdx[i]].absorption = abs[i];
        }
        // Interpolate between octave bands
        for (int i = 0; i < 27; i++) {
            if (mat.bands[i].absorption == 0) {
                int lo = -1, hi = -1;
                for (int j = i - 1; j >= 0; j--) { if (mat.bands[j].absorption > 0) { lo = j; break; } }
                for (int j = i + 1; j < 27; j++) { if (mat.bands[j].absorption > 0) { hi = j; break; } }
                if (lo >= 0 && hi >= 0) {
                    float t = (float)(i - lo) / (float)(hi - lo);
                    mat.bands[i].absorption = mat.bands[lo].absorption * (1-t) + mat.bands[hi].absorption * t;
                } else if (lo >= 0) {
                    mat.bands[i].absorption = mat.bands[lo].absorption;
                } else if (hi >= 0) {
                    mat.bands[i].absorption = mat.bands[hi].absorption;
                }
            }
        }
        materials.push_back(mat);
    };

    float concrete[]  = {0.01f, 0.01f, 0.02f, 0.02f, 0.03f, 0.04f, 0.05f};
    float wood[]      = {0.10f, 0.12f, 0.15f, 0.18f, 0.15f, 0.12f, 0.10f};
    float glass[]     = {0.18f, 0.06f, 0.04f, 0.03f, 0.02f, 0.02f, 0.02f};
    float carpet[]    = {0.08f, 0.24f, 0.57f, 0.69f, 0.71f, 0.73f, 0.73f};
    float foam[]      = {0.25f, 0.50f, 0.85f, 0.97f, 0.99f, 0.99f, 0.99f};
    float brick[]     = {0.02f, 0.02f, 0.03f, 0.04f, 0.05f, 0.07f, 0.07f};
    float plaster[]   = {0.01f, 0.02f, 0.02f, 0.03f, 0.04f, 0.05f, 0.05f};
    float curtain[]   = {0.07f, 0.31f, 0.49f, 0.75f, 0.70f, 0.60f, 0.60f};
    float abs10[]     = {0.10f, 0.10f, 0.10f, 0.10f, 0.10f, 0.10f, 0.10f};
    float abs20[]     = {0.20f, 0.20f, 0.20f, 0.20f, 0.20f, 0.20f, 0.20f};
    float abs30[]     = {0.30f, 0.30f, 0.30f, 0.30f, 0.30f, 0.30f, 0.30f};

    add("Concrete",       0.60f, 0.60f, 0.60f, concrete);
    add("Wood Panel",     0.55f, 0.35f, 0.15f, wood);
    add("Glass",          0.40f, 0.60f, 0.70f, glass);
    add("Carpet",         0.45f, 0.25f, 0.20f, carpet);
    add("Acoustic Foam",  0.30f, 0.30f, 0.35f, foam);
    add("Brick",          0.55f, 0.30f, 0.20f, brick);
    add("Plaster",        0.85f, 0.82f, 0.78f, plaster);
    add("Heavy Curtain",  0.40f, 0.15f, 0.20f, curtain);
    add("10% Absorbing",  0.50f, 0.50f, 0.55f, abs10);
    add("20% Absorbing",  0.45f, 0.50f, 0.50f, abs20);
    add("30% Absorbing",  0.40f, 0.45f, 0.50f, abs30);
}

// ─── Undo/Redo Manager ──────────────────────────────────────────────────────

static UndoManager g_undoManager;
UndoManager& GetUndoManager() { return g_undoManager; }

// Compact binary serialization of project state
// Format: [numVerts][verts...][numGroups][per-group: nameLen,name,numFaces,faces,matId,color]
//         [numMaterials][per-mat: id,nameLen,name,color,27 bands]
//         [groupMaterialMap][sources][receivers][surfReceivers][env][spps][tcr]

static void WriteStr(std::string& buf, const std::string& s) {
    uint32_t len = (uint32_t)s.size();
    buf.append((char*)&len, 4);
    buf.append(s.data(), len);
}
static void WriteF(std::string& buf, float v) { buf.append((char*)&v, 4); }
static void WriteI(std::string& buf, int32_t v) { buf.append((char*)&v, 4); }
static void WriteU(std::string& buf, uint32_t v) { buf.append((char*)&v, 4); }
static void WriteB(std::string& buf, bool v) { char c = v ? 1 : 0; buf.append(&c, 1); }
static void WriteV3(std::string& buf, const glm::vec3& v) { WriteF(buf, v.x); WriteF(buf, v.y); WriteF(buf, v.z); }

struct BufReader {
    const char* data; size_t pos; size_t size;
    std::string readStr() { uint32_t len; memcpy(&len, data+pos, 4); pos+=4; std::string s(data+pos, len); pos+=len; return s; }
    float readF() { float v; memcpy(&v, data+pos, 4); pos+=4; return v; }
    int32_t readI() { int32_t v; memcpy(&v, data+pos, 4); pos+=4; return v; }
    uint32_t readU() { uint32_t v; memcpy(&v, data+pos, 4); pos+=4; return v; }
    bool readB() { bool v = data[pos] != 0; pos++; return v; }
    glm::vec3 readV3() { float x=readF(), y=readF(), z=readF(); return {x,y,z}; }
    bool ok() const { return pos <= size; }
};

ProjectSnapshot UndoManager::Serialize(const Project& proj) {
    ProjectSnapshot snap;
    auto& buf = snap.data;
    buf.reserve(64 * 1024); // typical project ~few KB

    // Model vertices
    WriteU(buf, (uint32_t)proj.model.vertices.size());
    for (auto& v : proj.model.vertices) WriteV3(buf, v);

    // Model groups
    WriteU(buf, (uint32_t)proj.model.groups.size());
    for (auto& grp : proj.model.groups) {
        WriteStr(buf, grp.name);
        WriteV3(buf, grp.color);
        WriteI(buf, grp.materialId);
        WriteF(buf, grp.surfaceArea);
        WriteU(buf, (uint32_t)grp.faces.size());
        for (auto& f : grp.faces) {
            WriteU(buf, f.v[0]); WriteU(buf, f.v[1]); WriteU(buf, f.v[2]);
        }
    }

    // Materials
    WriteU(buf, (uint32_t)proj.materials.size());
    for (auto& mat : proj.materials) {
        WriteI(buf, mat.id);
        WriteStr(buf, mat.name);
        WriteV3(buf, mat.color);
        WriteF(buf, mat.density);
        WriteF(buf, mat.resistivity);
        WriteB(buf, mat.bilateral);
        for (int i = 0; i < 27; i++) {
            WriteF(buf, mat.bands[i].absorption);
            WriteF(buf, mat.bands[i].diffusion);
            WriteF(buf, mat.bands[i].transmission);
            WriteI(buf, mat.bands[i].diffusionLaw);
        }
    }

    // Group-material map
    WriteU(buf, (uint32_t)proj.groupMaterialMap.size());
    for (auto& [k, v] : proj.groupMaterialMap) { WriteI(buf, k); WriteI(buf, v); }

    // Sources
    WriteU(buf, (uint32_t)proj.sources.size());
    for (auto& src : proj.sources) {
        WriteI(buf, src.id); WriteStr(buf, src.name);
        WriteV3(buf, src.position); WriteV3(buf, src.direction);
        WriteB(buf, src.active); WriteF(buf, src.globalPowerDb);
        WriteI(buf, src.directivity); WriteF(buf, src.delaySeconds);
        for (int i = 0; i < 27; i++) WriteF(buf, src.spectrum[i]);
        WriteV3(buf, src.displayColor);
    }

    // Receivers
    WriteU(buf, (uint32_t)proj.punctualReceivers.size());
    for (auto& rcv : proj.punctualReceivers) {
        WriteI(buf, rcv.id); WriteStr(buf, rcv.name);
        WriteV3(buf, rcv.position); WriteV3(buf, rcv.direction);
        WriteB(buf, rcv.active); WriteI(buf, rcv.directivity);
        WriteV3(buf, rcv.displayColor);
    }

    // Surface receivers
    WriteU(buf, (uint32_t)proj.surfaceReceivers.size());
    for (auto& sr : proj.surfaceReceivers) {
        WriteI(buf, sr.id); WriteStr(buf, sr.name);
        WriteI(buf, (int)sr.type);
        WriteV3(buf, sr.vertexA); WriteV3(buf, sr.vertexB); WriteV3(buf, sr.vertexC);
        WriteF(buf, sr.gridResolution);
    }

    // Environment
    auto& env = proj.environment;
    WriteF(buf, env.temperature); WriteF(buf, env.pressure); WriteF(buf, env.humidity);
    WriteF(buf, env.z0); WriteI(buf, env.meteoProfile);

    return snap;
}

void UndoManager::Deserialize(const ProjectSnapshot& snap, Project& proj) {
    BufReader r{snap.data.data(), 0, snap.data.size()};

    proj.model.Clear();
    proj.sources.clear();
    proj.punctualReceivers.clear();
    proj.surfaceReceivers.clear();
    proj.groupMaterialMap.clear();

    // Vertices
    uint32_t nv = r.readU();
    proj.model.vertices.resize(nv);
    for (uint32_t i = 0; i < nv; i++) proj.model.vertices[i] = r.readV3();

    // Groups
    uint32_t ng = r.readU();
    proj.model.groups.resize(ng);
    for (uint32_t i = 0; i < ng; i++) {
        auto& grp = proj.model.groups[i];
        grp.name = r.readStr();
        grp.color = r.readV3();
        grp.materialId = r.readI();
        grp.surfaceArea = r.readF();
        uint32_t nf = r.readU();
        grp.faces.resize(nf);
        for (uint32_t j = 0; j < nf; j++) {
            grp.faces[j].v[0] = r.readU();
            grp.faces[j].v[1] = r.readU();
            grp.faces[j].v[2] = r.readU();
        }
    }

    // Materials
    uint32_t nm = r.readU();
    proj.materials.resize(nm);
    for (uint32_t i = 0; i < nm; i++) {
        auto& mat = proj.materials[i];
        mat.id = r.readI();
        mat.name = r.readStr();
        mat.color = r.readV3();
        mat.density = r.readF();
        mat.resistivity = r.readF();
        mat.bilateral = r.readB();
        for (int b = 0; b < 27; b++) {
            mat.bands[b].absorption = r.readF();
            mat.bands[b].diffusion = r.readF();
            mat.bands[b].transmission = r.readF();
            mat.bands[b].diffusionLaw = r.readI();
        }
    }

    // Group-material map
    uint32_t nmap = r.readU();
    for (uint32_t i = 0; i < nmap; i++) {
        int k = r.readI(), v = r.readI();
        proj.groupMaterialMap[k] = v;
    }

    // Sources
    uint32_t ns = r.readU();
    proj.sources.resize(ns);
    for (uint32_t i = 0; i < ns; i++) {
        auto& src = proj.sources[i];
        src.id = r.readI(); src.name = r.readStr();
        src.position = r.readV3(); src.direction = r.readV3();
        src.active = r.readB(); src.globalPowerDb = r.readF();
        src.directivity = r.readI(); src.delaySeconds = r.readF();
        for (int b = 0; b < 27; b++) src.spectrum[b] = r.readF();
        src.displayColor = r.readV3();
    }

    // Receivers
    uint32_t nr = r.readU();
    proj.punctualReceivers.resize(nr);
    for (uint32_t i = 0; i < nr; i++) {
        auto& rcv = proj.punctualReceivers[i];
        rcv.id = r.readI(); rcv.name = r.readStr();
        rcv.position = r.readV3(); rcv.direction = r.readV3();
        rcv.active = r.readB(); rcv.directivity = r.readI();
        rcv.displayColor = r.readV3();
    }

    // Surface receivers
    uint32_t nsr = r.readU();
    proj.surfaceReceivers.resize(nsr);
    for (uint32_t i = 0; i < nsr; i++) {
        auto& sr = proj.surfaceReceivers[i];
        sr.id = r.readI(); sr.name = r.readStr();
        sr.type = (SurfaceReceiver::Type)r.readI();
        sr.vertexA = r.readV3(); sr.vertexB = r.readV3(); sr.vertexC = r.readV3();
        sr.gridResolution = r.readF();
    }

    // Environment
    if (r.ok()) {
        proj.environment.temperature = r.readF();
        proj.environment.pressure = r.readF();
        proj.environment.humidity = r.readF();
        proj.environment.z0 = r.readF();
        proj.environment.meteoProfile = r.readI();
    }

    // Recompute normals and bounds
    proj.model.ComputeNormals();
    proj.model.ComputeBounds();
}

void UndoManager::SaveState(Project& proj, const char* actionName) {
    // Clear redo stack on new action
    m_redoStack.clear();

    Entry entry;
    entry.snapshot = Serialize(proj);
    entry.actionName = actionName ? actionName : "";

    m_undoStack.push_back(std::move(entry));
    if ((int)m_undoStack.size() > MAX_UNDO) m_undoStack.pop_front();
}

bool UndoManager::Undo(Project& proj) {
    if (m_undoStack.empty()) return false;

    // Save current state to redo
    Entry redoEntry;
    redoEntry.snapshot = Serialize(proj);
    redoEntry.actionName = m_undoStack.back().actionName;
    m_redoStack.push_back(std::move(redoEntry));

    // Restore from undo
    Deserialize(m_undoStack.back().snapshot, proj);
    m_undoStack.pop_back();
    return true;
}

bool UndoManager::Redo(Project& proj) {
    if (m_redoStack.empty()) return false;

    // Save current state to undo
    Entry undoEntry;
    undoEntry.snapshot = Serialize(proj);
    undoEntry.actionName = m_redoStack.back().actionName;
    m_undoStack.push_back(std::move(undoEntry));

    // Restore from redo
    Deserialize(m_redoStack.back().snapshot, proj);
    m_redoStack.pop_back();
    return true;
}

const char* UndoManager::UndoActionName() const {
    return m_undoStack.empty() ? nullptr : m_undoStack.back().actionName.c_str();
}

const char* UndoManager::RedoActionName() const {
    return m_redoStack.empty() ? nullptr : m_redoStack.back().actionName.c_str();
}

void UndoManager::Clear() {
    m_undoStack.clear();
    m_redoStack.clear();
}

} // namespace isimpa
