#pragma once

#include "mesh/scene_model.h"
#include <string>
#include <vector>
#include <map>
#include <deque>
#include <functional>

namespace isimpa {

// ─── Acoustic Material ───────────────────────────────────────────────────────

struct FreqBandData {
    float absorption = 0.0f;  // 0-1
    float diffusion  = 0.0f;  // 0-1
    float transmission = 0.0f; // dB loss
    int   diffusionLaw = 0;   // 0=Lambert, 1=Specular, 2=Uniform, 3=W2, 4=W3, 5=W4
};

struct AcousticMaterial {
    int id = -1;
    std::string name;
    glm::vec3 color{0.5f, 0.5f, 0.5f};
    float density = 0;
    float resistivity = 0;
    bool bilateral = true;

    // 27 third-octave bands: 50Hz to 20kHz
    static constexpr int NUM_BANDS = 27;
    static constexpr float BAND_FREQS[27] = {
        50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800,
        1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000,
        10000, 12500, 16000, 20000
    };
    FreqBandData bands[27] = {};

    float AverageAbsorption() const;
};

// ─── Sound Source ────────────────────────────────────────────────────────────

struct SoundSource {
    int id = 0;
    std::string name = "Source";
    glm::vec3 position{0, 1.5f, 0};
    glm::vec3 direction{0, 0, 1};
    bool active = true;
    float globalPowerDb = 80.0f;
    int directivity = 0;  // 0=omni, 1=uni, 2=XY, 3=YZ, 4=XZ
    float delaySeconds = 0;
    float spectrum[27] = {}; // per-band dB values
    glm::vec3 displayColor{1.0f, 0.6f, 0.1f};
};

// ─── Punctual Receiver ───────────────────────────────────────────────────────

struct PunctualReceiver {
    int id = 0;
    std::string name = "Receiver";
    glm::vec3 position{0, 1.2f, 0};
    glm::vec3 direction{0, 0, 1};
    bool active = true;
    int directivity = 0;  // 0=omni
    glm::vec3 displayColor{0.2f, 0.9f, 0.4f};
};

// ─── Surface Receiver ────────────────────────────────────────────────────────

struct SurfaceReceiver {
    int id = 0;
    std::string name = "Surface Receiver";
    enum Type { Scene, Plane } type = Scene;

    // For Plane type
    glm::vec3 vertexA{0}, vertexB{0}, vertexC{0};
    float gridResolution = 0.5f;

    // For Scene type
    std::vector<int> surfaceGroupIndices;
};

// ─── Encumbrance (Fitting Zone) ─────────────────────────────────────────────

struct EncumbranceFreqData {
    float absorption = 0.3f;   // alpha: energy absorbed per collision (0-1)
    float meanFreePath = 1.5f; // lambda: avg distance between objects (meters)
    int   diffusionLaw = 0;    // 0=Uniform, 1=Specular, 2=Lambert
};

struct Encumbrance {
    int id = 0;
    std::string name = "Fitting Zone";
    bool active = true;
    std::string description;

    // Geometry: box defined by two corners (cuboid type)
    enum Type { Cuboid, FaceGroup } type = Cuboid;
    glm::vec3 boxMin{0, 0, 0};
    glm::vec3 boxMax{2, 2, 2};

    // For FaceGroup type: indices into model groups that bound this volume
    std::vector<int> boundaryGroupIndices;

    // Per-frequency properties (27 third-octave bands)
    EncumbranceFreqData bands[27] = {};

    // Display
    glm::vec3 color{0.8f, 0.5f, 0.2f};
    float opacity = 0.3f;
    bool showLabel = true;

    Encumbrance() {
        // Default: moderate absorption, 1.5m mean free path
        for (int i = 0; i < 27; i++) {
            bands[i].absorption = 0.3f;
            bands[i].meanFreePath = 1.5f;
            bands[i].diffusionLaw = 0;
        }
    }
};

// ─── Volume Definition ──────────────────────────────────────────────────────

struct VolumeDefinition {
    int id = 0;
    std::string name = "Volume";
    glm::vec3 boxMin{0, 0, 0};
    glm::vec3 boxMax{5, 3, 5};
    float temperature = 20.0f;   // local override
    float humidity = 50.0f;
    bool  useCustomAtmo = false;
};

// ─── Background Noise ───────────────────────────────────────────────────────

struct BackgroundNoise {
    bool enabled = false;
    float spectrum[27] = {};  // dB per third-octave band

    BackgroundNoise() {
        // Default: NC-30 curve approximation
        float nc30[] = {57, 54, 51, 48, 44, 41, 39, 36, 34, 32, 31, 30, 29, 28, 27, 26,
                        25, 25, 24, 24, 23, 23, 22, 22, 21, 21, 20};
        for (int i = 0; i < 27; i++) spectrum[i] = nc30[i];
    }
};

// ─── Directivity Pattern ────────────────────────────────────────────────────

struct DirectivityPattern {
    int id = 0;
    std::string name;
    std::string filePath;  // .dir file path
    bool loaded = false;

    // Balloon data: attenuation in dB indexed by [azimuth][elevation][freq]
    // Simplified: store as flat vectors, interpolated at runtime
    struct BalloonPoint {
        float azimuth;    // degrees 0-360
        float elevation;  // degrees -90 to +90
        float attenuation; // dB
    };
    std::vector<BalloonPoint> points;
};

// ─── Environment ─────────────────────────────────────────────────────────────

struct Environment {
    float temperature = 20.0f;      // Celsius
    float pressure = 101325.0f;     // Pa
    float humidity = 50.0f;         // %
    bool  customAtmoAbsorption = false;
    float atmoAbsorptionValue = 0;
    int   meteoProfile = 2;         // 0=very fav, 1=fav, 2=homogeneous, 3=unfav, 4=very unfav
    float aLog = 0, bLin = 0;
    float z0 = 0.02f;              // ground roughness
};

// ─── Solver Configuration ────────────────────────────────────────────────────

struct SPPSConfig {
    int  calcMethod = 1;        // 0=Random, 1=Energetic
    int  particlesPerSource = 100000;
    int  particlesDisplay = 500;
    float timeStep = 0.005f;
    float simLength = 2.0f;
    float receiverRadius = 0.5f;
    int  randomSeed = 0;
    bool atmoAbsorption = true;
    bool fittingDiffusion = false;
    bool directFieldOnly = false;
    bool transmission = false;
    int  extinctionDb = 60;
    bool echogramPerSource = false;
    bool surfaceExportPerBand = true;
    int  surfaceExportType = 0; // 0=intensity, 1=SPL
    bool freqBands[27] = {};    // which bands to compute

    SPPSConfig() {
        // Default: octave bands 125-4000
        for (int i = 0; i < 27; i++) freqBands[i] = false;
        freqBands[4] = true;  // 125
        freqBands[7] = true;  // 250
        freqBands[10] = true; // 500
        freqBands[13] = true; // 1000
        freqBands[16] = true; // 2000
        freqBands[19] = true; // 4000
    }
};

struct TCRConfig {
    bool atmoAbsorption = true;
    bool surfaceExportPerBand = true;
    bool freqBands[27] = {};

    TCRConfig() {
        for (int i = 0; i < 27; i++) freqBands[i] = false;
        freqBands[4] = true; freqBands[7] = true; freqBands[10] = true;
        freqBands[13] = true; freqBands[16] = true; freqBands[19] = true;
    }
};

struct MeshConfig {
    float maxVolume = 0.5f;        // TetGen -a flag: max tet volume (m³)
    float qualityRatio = 1.5f;     // TetGen -q flag: radius-edge ratio
    bool preprocess = true;        // Run mesh repair before TetGen
    bool useVolumeConstraint = false; // Enable -a flag
};

// ─── Simulation Progress ─────────────────────────────────────────────────────

struct SimProgress {
    bool running = false;
    float percent = 0;
    std::string currentBand;
    int particlesActive = 0;
    int particlesTotal = 0;
    std::string etaString;
    std::vector<std::string> log;
    std::vector<std::string> warnings;
    std::vector<std::string> errors;
};

// ─── Project ─────────────────────────────────────────────────────────────────

class Project {
public:
    Project();

    // Scene
    SceneModel model;
    std::vector<AcousticMaterial> materials;
    std::map<int, int> groupMaterialMap; // groupIdx -> materialIdx

    // Elements
    std::vector<SoundSource> sources;
    std::vector<PunctualReceiver> punctualReceivers;
    std::vector<SurfaceReceiver> surfaceReceivers;
    std::vector<Encumbrance> encumbrances;
    std::vector<VolumeDefinition> volumes;
    std::vector<DirectivityPattern> directivityPatterns;
    BackgroundNoise backgroundNoise;

    // Config
    Environment environment;
    SPPSConfig sppsConfig;
    TCRConfig  tcrConfig;
    MeshConfig meshConfig;

    // State
    SimProgress simProgress;
    std::string projectPath;
    std::string lastResultDir;
    bool meshGenerated = false;

    // Operations
    void NewProject();
    void CreateDefaultRoom(float w, float l, float h);
    void LoadScenePLY(const std::string& path);

    SoundSource& AddSource();
    PunctualReceiver& AddReceiver();
    SurfaceReceiver& AddSurfaceReceiver();
    Encumbrance& AddEncumbrance();
    VolumeDefinition& AddVolume();

    void AssignMaterial(int groupIdx, int materialIdx);

    // Validation
    bool HasGeometry() const { return !model.IsEmpty(); }
    bool HasSources() const { return !sources.empty(); }
    bool HasReceivers() const { return !punctualReceivers.empty(); }
    int  TotalFaces() const;

    // Material database
    void LoadDefaultMaterials();

private:
    int m_nextSourceId = 1;
    int m_nextReceiverId = 1;
    int m_nextSurfRecId = 1;
    int m_nextEncumbranceId = 1;
    int m_nextVolumeId = 1;
};

// Global project instance
Project& GetProject();

// ─── Undo/Redo Manager ──────────────────────────────────────────────────────

// Opaque snapshot of project state (serialized binary)
struct ProjectSnapshot {
    std::string data;  // serialized project state
};

class UndoManager {
public:
    // Call before any mutating operation
    void SaveState(Project& proj, const char* actionName = nullptr);

    // Undo: restore previous state, return true if successful
    bool Undo(Project& proj);

    // Redo: restore next state, return true if successful
    bool Redo(Project& proj);

    bool CanUndo() const { return !m_undoStack.empty(); }
    bool CanRedo() const { return !m_redoStack.empty(); }

    const char* UndoActionName() const;
    const char* RedoActionName() const;

    void Clear();

private:
    struct Entry {
        ProjectSnapshot snapshot;
        std::string actionName;
    };

    static const int MAX_UNDO = 50;
    std::deque<Entry> m_undoStack;
    std::deque<Entry> m_redoStack;

    ProjectSnapshot Serialize(const Project& proj);
    void Deserialize(const ProjectSnapshot& snap, Project& proj);
};

UndoManager& GetUndoManager();

} // namespace isimpa
