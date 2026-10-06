// spps-gpu: what SPPS reads (config.xml, mesh.cbin, tetramesh.mbin) and how it prepares the scene,
// host side. Receipts into upstream (B:/repos/I-Simpa-upstream/src) beside each function in model.cpp.
#pragma once
#include <cstdint>
#include <string>
#include <vector>
#include "walk.h"

namespace spg {

struct Band {
  int freq = 0;
  bool docalc = false;
  float absAtmo = 0;      // absorption_atmospherique (1/m)
  float densite = 1;      // densite_proba_absorption_atmospherique
};
struct Source {
  V3 pos{0, 0, 0}, direction{0, 0, 0};
  int type = 0;
  float delay = 0;
  std::string name;
  std::vector<float> db, w;   // per band index (sorted by @freq, mapped by position)
  int tetra = -1;
};
struct MatBandCfg { float absorption = 0, diffusion = 0, tau = 0, affaiblissement = 0; int dotrans = 0, law = 0; };
struct Material { uint32_t id = 0; bool doubleSided = true; std::vector<MatBandCfg> bands; };
struct PointRx {
  V3 pos{0, 0, 0}, orient{0, 0, 0};
  int xmlId = 0;
  std::string lbl;
  std::vector<float> noiseDb;
  float cdt_vol = 0;
  int tetra = -1;
  std::string pathRp;   // the folder SauveRecepteursPonctuels chose (with its duplicate counter)
};
struct SurfRx { int id = 0; std::string name; };
struct CutRx {
  std::string name;
  int id = 0;
  V3 A{0, 0, 0}, B{0, 0, 0}, C{0, 0, 0}, normal{0, 0, 0};
  int nu = 0, nv = 0;
  float usize = 0, vsize = 0;
};
struct Fitting { int id = 0; std::vector<float> alpha, lambda; std::vector<int> law; };

struct Config {
  std::string wd, modelName, tetraName, rssDir, rssFile, rssCutFile, rpDir, rpFile, rpFileAdv, cumulFile,
      partDir, partFile, statsFile, dirDir;
  float temperature = 0, humidite = 0, pression = 0, c = 0, rho = 0, z0 = 0, alog = 0, blin = 0;
  float dt = 0, duree = 0;
  int nbSteps = 0, ratio = 1, nbBins = 0;
  float binDt = 0;
  std::vector<Band> bands;
  int nbBandsCalc = 0;
  std::vector<Source> sources;
  std::vector<Material> materials;
  std::vector<PointRx> receivers;
  std::vector<SurfRx> surfRx;
  std::vector<CutRx> cuts;
  std::vector<Fitting> fittings;
  // SPPS settings (spps/data_manager/core_configuration.cpp)
  int nbPart = 1, nbPartRender = 0, absAtmoCalc = 0, bySource = 0, saveSurf = 1, saveRp = 1, directCalc = 0,
      encCalc = 0, method = 0, transCalc = 0, byFreq = 0, surfMode = 0;
  int64_t seed = 0;
  float radius = 0, transEps = 0;
  bool loaded = false;
};

struct SceneFace { uint32_t a, b, c; uint32_t idMat; int idRs, idEn; V3 normal; int mat = -1; int enc = -1; };
struct RxFace { int tetraFace; int ia, ib, ic; V3 p0, p1, p2; };   // r_Surf_Face

struct Model {
  std::vector<V3> sceneVerts;
  std::vector<SceneFace> faces;
  // tetrahedral mesh
  int T = 0;
  std::vector<V3> nodes;
  std::vector<int> corner, idVolume, fv, nb, sf, rf;
  std::vector<V3> fn;
  std::vector<int> volEnc;            // fitting index per tetra, -1
  // links
  std::vector<std::vector<int>> rpByTetra, cutByTetra;
  std::vector<std::vector<RxFace>> rxFaces;  // per surface receiver, in InitRecepteurS order
  std::vector<int> rxFaceBase;               // global index of each receiver's first face
  int nbRxFaces = 0;
};

// Loading and preparation. Each prints what SPPS prints and returns false where SPPS stops.
bool loadConfig(const std::string& path, Config& cfg);
bool loadScene(const Config& cfg, Model& m);          // initMesh (coreinitialisation.cpp:387-448)
bool loadTetra(const Config& cfg, Model& m);          // t_TetraMesh::LoadFile (coreTypes.cpp:168-244)
void prepare(Config& cfg, Model& m);                  // initTetraMesh's links, then ExpandPunctual... and the source move
bool checkSourcePosition(const Config& cfg, const Model& m);  // sppsInitialisation.cpp:95-111
// The rules A2 refuses: empty when the run can go, else one "code: message" per refusal.
std::vector<std::string> refusals(const Config& cfg, const Model& m);

}  // namespace spg
