#pragma once

#include <string>
#include <vector>
#include <cstdint>
#include <glm/glm.hpp>

namespace isimpa {

// ─── GABE File Reader ────────────────────────────────────────────────────────

enum GabeColType {
    GABE_FLOAT = 50,
    GABE_INT = 51,
    GABE_STRING = 52
};

struct GabeColumn {
    GabeColType type;
    std::string label;
    int rows = 0;
    std::vector<float> floatData;
    std::vector<int32_t> intData;
    std::vector<std::string> strData;
};

struct GabeFile {
    std::vector<GabeColumn> columns;
    bool readOnly = false;

    bool Load(const std::string& path);
    int GetColCount() const { return (int)columns.size(); }
    int GetRowCount() const { return columns.empty() ? 0 : columns[0].rows; }
    const GabeColumn* FindColumn(const std::string& label) const;
};

// ─── Parsed Acoustic Results ─────────────────────────────────────────────────

struct ReceiverResult {
    int receiverId = 0;
    std::string name;
    std::vector<float> frequencies;   // Hz
    std::vector<float> splDb;         // SPL per freq band
    std::vector<float> splDbA;        // SPL(A) per freq band
    float totalSplDb = 0;
    float totalSplDbA = 0;

    // Acoustic parameters
    float rt60 = 0, edt = 0;
    float c80 = 0, d50 = 0;
    float ts = 0, g = 0;
    float lf = 0, lfc = 0;
    float stEarly = 0;

    // Echogram
    std::vector<float> echoTime;
    std::vector<float> echoEnergy;

    // Schroeder
    std::vector<float> schroederTime;
    std::vector<float> schroederDb;
};

struct SimulationResults {
    std::string solverName; // "SPPS" or "TCR"
    std::string timestamp;
    std::vector<ReceiverResult> receivers;

    // Global results (TCR)
    float sabineRT = 0, eyringRT = 0;
    float diffuseFieldSPL = 0;
    float equivalentAbsorption = 0;

    bool loaded = false;
};

// Load results from a simulation working directory
bool LoadResults(const std::string& resultDir, SimulationResults& results);

// Scan result directory for all .gabe and .recp files
std::vector<std::string> FindResultFiles(const std::string& dir, const std::string& extension);

// ─── Surface Receiver Results (.csbin) ──────────────────────────────────────

struct SurfRecFace {
    uint32_t v[3];          // vertex indices
    float    energySum;     // summed/averaged energy for coloring
    std::vector<std::pair<int, float>> timeSteps; // (timeStepIndex, energy) per record
};

struct SurfaceRecResult {
    std::string name;
    std::vector<glm::vec3> nodes;       // vertex positions
    std::vector<SurfRecFace> faces;
    float minEnergy = 0, maxEnergy = 0;
    int recordType = 0;                 // 0=SPL, 2=TR, etc.
    int nbTimeSteps = 0;               // total time steps in simulation
    float timeStep = 0;                // seconds per time step
    bool loaded = false;

    // Time-step modes
    void SetTimeStep(int step);
    void SetCumulative(int step);
    void SetTotal();

    // Acoustic parameter maps: compute per-face values and store in energySum
    // Each sets energySum to the computed parameter for each face
    enum ParamType { PARAM_SPL=0, PARAM_RT60, PARAM_EDT, PARAM_C80, PARAM_D50, PARAM_TS };
    void ComputeParameter(ParamType param);
};

bool LoadCSBIN(const std::string& path, SurfaceRecResult& result);

// ─── Particle Trajectories (.pbin) ──────────────────────────────────────────

struct ParticleStep {
    glm::vec3 position;
    float energy;
};

struct Particle {
    uint16_t firstTimeStep;
    std::vector<ParticleStep> steps;
};

struct ParticleData {
    std::vector<Particle> particles;
    float timeStep = 0;
    int maxTimeSteps = 0;
    bool loaded = false;
};

bool LoadPBIN(const std::string& path, ParticleData& data);

} // namespace isimpa
