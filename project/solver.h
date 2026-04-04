#pragma once

#include "project/project.h"
#include <string>
#include <functional>

namespace isimpa {

// Write config.xml for solver
bool WriteConfigXML(const Project& project, const std::string& workingDir,
                    const std::string& solver); // "spps" or "tcr"

// Write surface mesh in I-Simpa binary format (.cbin)
bool WriteMeshBinary(const Project& project, const std::string& path);

// Write surface mesh in POLY format for TetGen
bool WritePoly(const Project& project, const std::string& path);

// Convert TetGen output (.1.node, .1.ele, .1.face, .1.neigh) to .mbin
bool ConvertTetGenToMbin(const std::string& basePath, const std::string& mbinPath,
                         const Project& project);

// Find solver executables in I-Simpa installation
std::string FindSolverExe(const std::string& solver); // returns full path or ""

// Run preprocess + TetGen meshing pipeline
// Returns true if mesh was generated successfully
bool RunMeshGeneration(const std::string& workingDir, Project& project,
                       std::function<void(float, const std::string&)> progressCallback = nullptr);

// Launch solver as subprocess
// progressCallback called with (percent, message)
// Returns true if solver completed successfully
bool RunSolver(const std::string& solver,
               const std::string& workingDir,
               Project& project,
               std::function<void(float, const std::string&)> progressCallback = nullptr);

// Full simulation pipeline: export mesh -> preprocess -> tetgen -> solver
bool RunFullSimulation(const std::string& solver,
                       const std::string& workingDir,
                       Project& project,
                       std::function<void(float, const std::string&)> progressCallback = nullptr);

// Validate project before simulation
struct ValidationResult {
    std::vector<std::string> errors;   // Fatal — prevent launch
    std::vector<std::string> warnings; // Non-fatal — allow with caution
    bool ok() const { return errors.empty(); }
};
ValidationResult ValidateProject(const Project& project, const std::string& solver);

// Save project to XML file
bool SaveProject(const Project& project, const std::string& path);

// Load project from XML file (.isimpa format)
bool LoadProject(Project& project, const std::string& path);

// Load original I-Simpa .proj file (ZIP archive with projet_config.xml + sceneMesh.bin)
bool LoadOriginalProject(Project& project, const std::string& projPath);

// Material database import/export
bool ExportMaterials(const std::vector<AcousticMaterial>& materials, const std::string& path);
bool ImportMaterials(std::vector<AcousticMaterial>& materials, const std::string& path);

// Import materials from original I-Simpa appconst.xml
bool ImportMatlibFromAppConst(std::vector<AcousticMaterial>& materials, const std::string& path);

} // namespace isimpa
