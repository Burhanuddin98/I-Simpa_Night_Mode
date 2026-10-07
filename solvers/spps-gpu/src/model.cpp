// spps-gpu: reading config.xml, mesh.cbin and tetramesh.mbin, and preparing the scene, as SPPS does.
// Receipts are paths under B:/repos/I-Simpa-upstream/src (tag v1.4.0_snapshot_14_01_2026) with
// our patch 0001 (the surface-receiver time bin).
#include "model.h"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include "../third_party/tinyxml2/tinyxml2.h"

namespace spg {

using tinyxml2::XMLElement;

// ---------------------------------------------------------------------------------------------
// cxml.cpp:108-118 GetProperty: a missing attribute prints a line and reads as "".
static std::string prop(const XMLElement* e, const char* name) {
  const char* v = e ? e->Attribute(name) : nullptr;
  if (!v) {
    std::cout << "Xml Property " << name << " doesn't exist !" << std::endl;
    return std::string();
  }
  return v;
}
static bool has(const XMLElement* e, const char* name) { return e && e->Attribute(name); }
// coreString.cpp:84-105: atoi; atof after the first ',' becomes '.', stored as float
static int toInt(const std::string& s) { return atoi(s.c_str()); }
static float toFloat(const std::string& s) {
  std::string t = s;
  size_t p = t.find(',');
  if (p != std::string::npos) t.replace(p, 1, ".");
  return (float)atof(t.c_str());
}
static const XMLElement* child(const XMLElement* e, const char* name) { return e ? e->FirstChildElement(name) : nullptr; }
static std::vector<const XMLElement*> items(const XMLElement* e) {
  std::vector<const XMLElement*> v;
  if (!e) return v;
  for (const XMLElement* c = e->FirstChildElement(); c; c = c->NextSiblingElement()) v.push_back(c);
  return v;
}
// cxml.cpp:130-156 OrderChildsByProperty: sorted by atoi of the property
static std::vector<const XMLElement*> sortedBy(const XMLElement* e, const char* name) {
  std::vector<const XMLElement*> v = items(e);
  std::stable_sort(v.begin(), v.end(), [&](const XMLElement* a, const XMLElement* b) {
    const char* x = a->Attribute(name);
    const char* y = b->Attribute(name);
    return atoi(x ? x : "") < atoi(y ? y : "");
  });
  return v;
}

// data_manager/data_calculation: c_son, masse_vol, Coef_Att_Atmos (double)
static double c_son(double K) { return 343.2 * sqrt(K / 293.15); }
static double masse_vol(double P, double K) { return P * 28.9644 / (8314.32 * K); }
static double coefAttAtmos(double F, double H, double P, double K) {
  const double Pref = 101325, Kref = 293.15, FmolO = 0.209, FmolN = 0.781, KvibO = 2239.1, KvibN = 3352.0, K01 = 273.16;
  double cson = c_son(K);
  double C = -6.8346 * pow(K01 / K, 1.261) + 4.6151;
  double Ps = Pref * pow(10., C);
  double hmol = H * Ps / Pref;
  double Acr = (Pref / P) * (1.60E-10) * sqrt(K / Kref) * pow(F, 2);
  double Fr = (P / Pref) * (24. + 4.04E4 * hmol * (0.02 + hmol) / (0.391 + hmol));
  double Am = 1.559 * FmolO * exp(-KvibO / K) * pow(KvibO / K, 2);
  double AvibO = Am * (F / cson) * 2. * (F / Fr) / (1 + pow(F / Fr, 2));
  Fr = (P / Pref) * sqrt(Kref / K) * (9. + 280. * hmol * exp(-4.170 * (pow(K / Kref, -1. / 3.) - 1)));
  Am = 1.559 * FmolN * exp(-KvibN / K) * pow(KvibN / K, 2);
  double AvibN = Am * (F / cson) * 2. * (F / Fr) / (1 + pow(F / Fr, 2));
  return Acr + AvibO + AvibN;
}

// ---------------------------------------------------------------------------------------------
// base_core_configuration.cpp:46-331 LoadCfgFile (with patch 0001) and spps/data_manager/core_configuration.cpp:5-69
bool loadConfig(const std::string& path, Config& cfg) {
  tinyxml2::XMLDocument doc;
  FILE* f = nullptr;
#ifdef _WIN32
  f = _wfopen(std::filesystem::u8path(path).wstring().c_str(), L"rb");
#else
  f = fopen(path.c_str(), "rb");
#endif
  if (!f) return false;
  tinyxml2::XMLError err = doc.LoadFile(f);
  fclose(f);
  if (err != tinyxml2::XML_SUCCESS) return false;
  const XMLElement* root = doc.RootElement();
  if (!root) return false;
  cfg.wd = prop(root, "workingdirectory");
  bool forceAbs = false;
  float absAtmo = 0;
  const XMLElement* atmo = child(root, "condition_atmospherique");
  if (atmo) {
    cfg.humidite = toFloat(prop(atmo, "humidite"));
    cfg.pression = toFloat(prop(atmo, "pression"));
    cfg.temperature = toFloat(prop(atmo, "temperature"));
    cfg.c = (float)c_son(cfg.temperature + 273.15);
    cfg.rho = (float)masse_vol(cfg.pression, cfg.temperature + 273.15);
    cfg.z0 = toFloat(prop(atmo, "z0"));
    cfg.alog = toFloat(prop(atmo, "alog"));
    cfg.blin = toFloat(prop(atmo, "blin"));
    forceAbs = toInt(prop(atmo, "disable_absatmo_computation")) == 1;
    absAtmo = toFloat(prop(atmo, "absatmo"));
  }
  const XMLElement* simu = child(root, "simulation");
  if (simu) {
    cfg.rssDir = prop(simu, "recepteurss_directory");
    cfg.rssFile = prop(simu, "recepteurss_filename");
    cfg.rssCutFile = has(simu, "recepteurss_cut_filename") ? prop(simu, "recepteurss_cut_filename") : "rs_cut.csbin";
    cfg.rpDir = prop(simu, "receiversp_directory");
    cfg.rpFile = prop(simu, "receiversp_filename");
    cfg.rpFileAdv = prop(simu, "receiversp_filename_adv");
    cfg.cumulFile = prop(simu, "cumul_filename");
    cfg.modelName = prop(simu, "modelName");
    cfg.tetraName = prop(simu, "tetrameshFileName");
    cfg.dt = toFloat(prop(simu, "pasdetemps"));
    cfg.duree = toFloat(prop(simu, "duree_simulation"));
    cfg.nbSteps = (int)(ceilf(cfg.duree / cfg.dt));
    // patch 0001: the sound maps' time bin
    {
      float timeStep = cfg.dt, surfaceStep = timeStep;
      if (has(simu, "recepteurs_surfaciques_pas_temps")) surfaceStep = toFloat(prop(simu, "recepteurs_surfaciques_pas_temps"));
      int ratio = 1;
      if (timeStep > 0 && surfaceStep > timeStep) ratio = (int)floor(surfaceStep / timeStep + 0.5f);
      if (ratio < 1) ratio = 1;
      cfg.binDt = timeStep * (float)ratio;
      cfg.ratio = ratio;
      cfg.nbBins = (cfg.nbSteps + ratio - 1) / ratio;
    }
    cfg.dirDir = prop(simu, "directivities_directory");
    const XMLElement* fe = child(simu, "freq_enum");
    if (fe) {
      for (const XMLElement* b : sortedBy(fe, "freq")) {
        Band bd;
        bd.docalc = (prop(b, "docalc") == "1");
        if (bd.docalc) cfg.nbBandsCalc++;
        bd.freq = toInt(prop(b, "freq"));
        if (forceAbs) bd.absAtmo = absAtmo;
        else bd.absAtmo = (float)coefAttAtmos(bd.freq, cfg.humidite, cfg.pression, cfg.temperature + 273.15) * (logf(10.f) / 10.f);
        bd.densite = expf(-bd.absAtmo * (cfg.c * cfg.dt));
        cfg.bands.push_back(bd);
      }
    }
  }
  const size_t nbf = cfg.bands.size();
  const XMLElement* srcs = child(root, "sources");
  for (const XMLElement* e : items(srcs)) {
    Source s;
    float x = toFloat(prop(e, "x")), y = toFloat(prop(e, "y")), z = toFloat(prop(e, "z"));
    s.pos = mk(x, y, z);
    s.type = toInt(prop(e, "directivite"));
    s.delay = toFloat(prop(e, "delay"));
    s.name = prop(e, "name");
    if (s.type == 1 || s.type == 5) {
      float u = toFloat(prop(e, "u")), v = toFloat(prop(e, "v")), w = toFloat(prop(e, "w"));
      V3 uvw = mk(u, v, w);
      s.direction = divs(uvw, len(uvw));
    }
    s.db.assign(nbf, 0.f);
    s.w.assign(nbf, 0.f);
    size_t i = 0;
    for (const XMLElement* b : sortedBy(e, "freq")) {
      if (i < nbf) {
        s.db[i] = toFloat(prop(b, "db"));
        s.w[i] = (float)(pow(10., -12.) * pow(10, s.db[i] / 10));
        i++;
      }
    }
    if (s.type == 5) (void)prop(e, "directivity_file");
    cfg.sources.push_back(s);
  }
  const XMLElement* surfs = child(root, "surface_absorption_enum");
  for (const XMLElement* e : items(surfs)) {
    Material m;
    m.id = (uint32_t)toInt(prop(e, "id"));
    if (has(e, "side_material")) m.doubleSided = (toInt(prop(e, "side_material")) == 1);
    m.bands.assign(nbf, MatBandCfg());
    size_t i = 0;
    for (const XMLElement* b : sortedBy(e, "freq")) {
      if (i < nbf) {
        MatBandCfg& mb = m.bands[i];
        mb.absorption = toFloat(prop(b, "absorb"));
        mb.diffusion = toFloat(prop(b, "diffusion"));
        if (has(b, "affaiblissement")) {
          mb.affaiblissement = toFloat(prop(b, "affaiblissement"));
          mb.tau = (float)pow(10, -mb.affaiblissement / 10.);
          mb.dotrans = 1;
        }
        mb.law = toInt(prop(b, "loi"));
        i++;
      }
    }
    cfg.materials.push_back(m);
  }
  const XMLElement* rps = child(root, "recepteursp");
  for (const XMLElement* e : items(rps)) {
    PointRx r;
    float x = toFloat(prop(e, "x")), y = toFloat(prop(e, "y")), z = toFloat(prop(e, "z"));
    r.pos = mk(x, y, z);
    r.xmlId = toInt(prop(e, "id"));
    r.lbl = prop(e, "lbl");
    float u = toFloat(prop(e, "u")), v = toFloat(prop(e, "v")), w = toFloat(prop(e, "w"));
    V3 uvw = mk(u, v, w);
    r.orient = divs(uvw, len(uvw));
    r.noiseDb.assign(nbf, 0.f);
    size_t i = 0;
    for (const XMLElement* b : sortedBy(e, "freq")) {
      if (i < nbf) { r.noiseDb[i] = toFloat(prop(b, "db")); i++; }
    }
    cfg.receivers.push_back(r);
  }
  const XMLElement* rss = child(root, "recepteurss");
  for (const XMLElement* e : items(rss)) {
    std::string nm = e->Name();
    if (nm == "recepteur_surfacique") {
      SurfRx s;
      s.id = toInt(prop(e, "id"));
      s.name = prop(e, "name");
      cfg.surfRx.push_back(s);
    } else if (nm == "recepteur_surfacique_coupe") {
      CutRx c;
      c.name = prop(e, "name");
      c.id = toInt(prop(e, "id"));
      { float a = toFloat(prop(e, "ax")), b = toFloat(prop(e, "ay")), d = toFloat(prop(e, "az")); c.A = mk(a, b, d); }
      { float a = toFloat(prop(e, "bx")), b = toFloat(prop(e, "by")), d = toFloat(prop(e, "bz")); c.B = mk(a, b, d); }
      { float a = toFloat(prop(e, "cx")), b = toFloat(prop(e, "cy")), d = toFloat(prop(e, "cz")); c.C = mk(a, b, d); }
      // FaceNormal (mathlib.h:391): Cross(vp1 - vp2, vp2 - vp3), normalised
      {
        V3 a = sub(c.A, c.B), b = sub(c.B, c.C);
        V3 n = mk(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x);
        float l = len(n);
        if (!(l < EPS)) { float inv = 1.0f / l; n = mul(n, inv); }
        c.normal = n;
      }
      float resolution = toFloat(prop(e, "resolution"));
      V3 BC = sub(c.C, c.B), BA = sub(c.A, c.B);
      c.nu = (int)(uint32_t)ceilf(len(BC) / resolution);
      c.nv = (int)(uint32_t)ceilf(len(BA) / resolution);
      c.usize = len(BC) / c.nu;
      c.vsize = len(BA) / c.nv;
      cfg.cuts.push_back(c);
    }
  }
  const XMLElement* encs = child(root, "encombrement_enum");
  for (const XMLElement* e : items(encs)) {
    Fitting ft;
    ft.id = toInt(prop(e, "id"));
    ft.alpha.assign(nbf, 0.f); ft.lambda.assign(nbf, 0.f); ft.law.assign(nbf, 0);
    size_t i = 0;
    for (const XMLElement* b : sortedBy(e, "freq")) {
      if (i < nbf) {
        ft.alpha[i] = toFloat(prop(b, "alpha"));
        ft.lambda[i] = toFloat(prop(b, "lambda"));
        ft.law[i] = toInt(prop(b, "loi_diff"));
        i++;
      }
    }
    cfg.fittings.push_back(ft);
  }
  // spps/data_manager/core_configuration.cpp
  if (simu) {
    cfg.partFile = prop(simu, "particules_filename");
    cfg.partDir = prop(simu, "particules_directory");
    cfg.statsFile = prop(simu, "stats_filename");
    int nbpart = toInt(prop(simu, "nbparticules"));
    if (nbpart < 1) nbpart = 1;
    int nbrender = toInt(prop(simu, "nbparticules_rendu"));
    if (nbrender < 0) nbrender = 0;
    cfg.nbPart = nbpart;
    cfg.nbPartRender = nbrender;
    cfg.absAtmoCalc = toInt(prop(simu, "abs_atmo_calc"));
    cfg.bySource = has(simu, "output_recp_bysource") ? toInt(prop(simu, "output_recp_bysource")) : 0;
    cfg.seed = has(simu, "random_seed") ? toInt(prop(simu, "random_seed")) : 0;
    cfg.saveSurf = has(simu, "save_surface_intersection") ? toInt(prop(simu, "save_surface_intersection")) : 1;
    cfg.saveRp = has(simu, "save_receivers_intersection") ? toInt(prop(simu, "save_receivers_intersection")) : 1;
    cfg.directCalc = toInt(prop(simu, "direct_calc"));
    cfg.encCalc = toInt(prop(simu, "enc_calc"));
    cfg.method = toInt(prop(simu, "computation_method"));
    cfg.radius = toFloat(prop(simu, "rayon_recepteurp"));
    cfg.transEps = toFloat(prop(simu, "trans_epsilon"));
    cfg.transCalc = toInt(prop(simu, "trans_calc"));
    cfg.byFreq = toInt(prop(simu, "output_recs_byfreq"));
    cfg.surfMode = toInt(prop(simu, "surf_receiv_method"));
  }
  cfg.loaded = true;
  return true;
}

// ---------------------------------------------------------------------------------------------
// FaceNormal (mathlib.h:391-397)
static V3 faceNormal(V3 p1, V3 p2, V3 p3) {
  V3 a = sub(p1, p2), b = sub(p2, p3);
  V3 n = mk(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x);
  float l = len(n);
  if (l < EPS) return n;
  float inv = 1.0f / l;
  return mul(n, inv);
}

static bool readFile(const std::string& path, std::vector<unsigned char>& buf) {
  std::ifstream in(std::filesystem::u8path(path), std::ios::binary);
  if (!in) return false;
  buf.assign(std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
  return true;
}
template <class T> static bool rd(const std::vector<unsigned char>& b, size_t& o, T& v) {
  if (o + sizeof(T) > b.size()) return false;
  memcpy(&v, &b[o], sizeof(T));
  o += sizeof(T);
  return true;
}

// lib_interface/input_output/bin.cpp:117-147, 236-320 (CformatBIN::ImportBIN and its node walk),
// then coreinitialisation.cpp:387-448 initMesh
bool loadScene(const Config& cfg, Model& m) {
  std::string path = cfg.wd + cfg.modelName;
  std::vector<unsigned char> b;
  bool ok = readFile(path, b);
  size_t o = 0;
  uint32_t major = 0, minor = 0;
  if (ok) ok = rd(b, o, major) && rd(b, o, minor) && major == 1 && minor == 0;
  while (ok) {
    uint16_t type; uint32_t son, next;
    if (!rd(b, o, type)) { ok = false; break; }
    o += 2;
    if (!rd(b, o, son) || !rd(b, o, next)) { ok = false; break; }
    if (type == 0) {
      uint32_t nv;
      if (!rd(b, o, nv)) { ok = false; break; }
      for (uint32_t i = 0; i < nv; i++) {
        float x, y, z;
        if (!rd(b, o, x) || !rd(b, o, y) || !rd(b, o, z)) { ok = false; break; }
        m.sceneVerts.push_back(mk(x, y, z));
      }
    } else if (type == 1) {
      o += 255 + 1;
      uint32_t nf;
      if (!rd(b, o, nf)) { ok = false; break; }
      for (uint32_t i = 0; i < nf; i++) {
        SceneFace f;
        if (!rd(b, o, f.a) || !rd(b, o, f.b) || !rd(b, o, f.c) || !rd(b, o, f.idMat) || !rd(b, o, f.idRs) || !rd(b, o, f.idEn)) { ok = false; break; }
        m.faces.push_back(f);
      }
    } else {
      ok = false;
    }
    if (!ok || next == 0) break;
    if (next > o) o = next;
  }
  if (!ok || m.sceneVerts.empty()) {
    std::cout << "Unable to read the scene mesh file :" << std::endl << path << std::endl;
    return false;
  }
  for (auto& f : m.faces) {
    if (f.a >= m.sceneVerts.size() || f.b >= m.sceneVerts.size() || f.c >= m.sceneVerts.size()) {
      std::cerr << "spps-gpu: mesh_invalid: a scene face indexes a vertex outside the .cbin" << std::endl;
      return false;
    }
  }
  // materials by id, with upstream's "last id used" cache: id 0 is looked up once before the loop
  auto matByIndex = [&](uint32_t id) -> int {
    for (size_t i = 0; i < cfg.materials.size(); i++) if (cfg.materials[i].id == id) return (int)i;
    return -1;
  };
  auto fitByIndex = [&](int id) -> int {
    for (size_t i = 0; i < cfg.fittings.size(); i++) if (cfg.fittings[i].id == id) return (int)i;
    return -1;
  };
  int lastMat = matByIndex(0);
  uint32_t lastMatId = 0;
  int lastEnc = -1, lastEncId = -1;
  for (auto& f : m.faces) {
    f.normal = faceNormal(m.sceneVerts[f.a], m.sceneVerts[f.b], m.sceneVerts[f.c]);
    if (f.idMat != lastMatId) {
      lastMatId = f.idMat;
      lastMat = matByIndex(lastMatId);
      if (lastMat < 0) {
        std::cerr << "Wrong project configuration, a face is defined but no materials are attached to it ! Check your group/material associations" << std::endl;
        exit(-1);
      }
    }
    f.mat = lastMat;   // may stay -1 for leading idMat 0 faces with no material 0 (SPPS dereferences NULL there)
    if (f.idEn != lastEncId && f.idEn != -1) { lastEncId = f.idEn; lastEnc = fitByIndex(lastEncId); }
    f.enc = (f.idEn != -1) ? lastEnc : -1;
  }
  return true;
}

// coreTypes.cpp:168-244 t_TetraMesh::LoadFile with mbin.cpp:160-215 CMBIN::ImportBIN
bool loadTetra(const Config& cfg, Model& m) {
  std::string path = cfg.wd + cfg.tetraName;
  std::vector<unsigned char> b;
  uint32_t T = 0, N = 0;
  size_t o = 0;
  bool ok = readFile(path, b) && rd(b, o, T) && rd(b, o, N) && b.size() >= 8 + 12ull * N + 100ull * T;
  if (!ok) {
    std::cout << "Unable to read the tetrahedalization of the scene mesh file, calculation canceled." << std::endl;
    return false;
  }
  if (T == 0 || N == 0) {
    std::cout << path << std::endl << "Tetrahedron file is empty, the calculation can't be done !" << std::endl;
    std::cout << "Unable to read the tetrahedalization of the scene mesh file, calculation canceled." << std::endl;
    return false;
  }
  m.T = (int)T;
  m.nodes.resize(N);
  for (uint32_t i = 0; i < N; i++) { float x, y, z; rd(b, o, x); rd(b, o, y); rd(b, o, z); m.nodes[i] = mk(x, y, z); }
  m.corner.resize(4 * T); m.idVolume.resize(T); m.fv.resize(12 * T); m.nb.resize(4 * T); m.sf.resize(4 * T);
  m.rf.assign(4 * T, -1); m.fn.resize(4 * T); m.volEnc.assign(T, -1);
  for (uint32_t t = 0; t < T; t++) {
    int32_t v[25];
    for (int k = 0; k < 25; k++) rd(b, o, v[k]);
    for (int k = 0; k < 4; k++) {
      if (v[k] < 0 || (uint32_t)v[k] >= N) {
        std::cerr << "spps-gpu: mesh_invalid: tetrahedron " << t << " indexes a node outside the .mbin" << std::endl;
        return false;
      }
      m.corner[4 * t + k] = v[k];
    }
    for (int i = 0; i < 4; i++) for (int j = 0; j < 4; j++)
      if (i != j && v[i] == v[j]) {
        fprintf(stderr, "Error in input mesh, a tetrahedra have at least the same two vertices idTetra:%i vertices:%li %li %li %li",
                (int)t, (long)v[0], (long)v[1], (long)v[2], (long)v[3]);
        exit(1);
      }
    m.idVolume[t] = v[4];
    for (int f = 0; f < 4; f++) {
      const int32_t* fd = v + 5 + 5 * f;
      int k = 4 * t + f;
      for (int j = 0; j < 3; j++) {
        if (fd[j] < 0 || (uint32_t)fd[j] >= N) {
          std::cerr << "spps-gpu: mesh_invalid: tetrahedron " << t << " face " << f << " indexes a node outside the .mbin" << std::endl;
          return false;
        }
        m.fv[3 * k + j] = fd[j];
      }
      int marker = fd[3], neighbor = fd[4];
      if (marker >= 0 && (size_t)marker >= m.faces.size()) {
        std::cerr << "spps-gpu: mesh_invalid: tetrahedron " << t << " face " << f << " names scene face " << marker << ", beyond the .cbin's " << m.faces.size() << std::endl;
        return false;
      }
      if (neighbor >= 0 && (uint32_t)neighbor >= T) {
        std::cerr << "spps-gpu: mesh_invalid: tetrahedron " << t << " face " << f << " names neighbour " << neighbor << ", beyond " << T << std::endl;
        return false;
      }
      m.sf[k] = marker >= 0 ? marker : -1;
      m.nb[k] = neighbor >= 0 ? neighbor : -1;
      m.fn[k] = faceNormal(m.nodes[fd[0]], m.nodes[fd[1]], m.nodes[fd[2]]);
    }
  }
  return true;
}

// ---------------------------------------------------------------------------------------------
// mathlib.h:878-1067 ClosestDistanceBetweenDotAndTriangle (squared distance), float
static float closestDistSq(V3 va, V3 vb, V3 vc, V3 P) {
  V3 E0 = sub(vb, va), E1 = sub(vc, va), kDiff = sub(va, P);
  float fA00 = dot(E0, E0), fA01 = dot(E0, E1), fA11 = dot(E1, E1), fB0 = dot(kDiff, E0), fB1 = dot(kDiff, E1), fC = dot(kDiff, kDiff);
  float fDet = (float)fabs(fA00 * fA11 - fA01 * fA01);
  float fS = fA01 * fB1 - fA11 * fB0;
  float fT = fA01 * fB0 - fA00 * fB1;
  float fSqrDist;
  if (fabs(fDet) < 0.00000001f) return 100000000.0f;
  if (fS + fT <= fDet) {
    if (fS < 0.0f) {
      if (fT < 0.0f) {
        if (fB0 < 0.0f) {
          fT = 0.0f;
          if (-fB0 >= fA00) { fS = 1.0f; fSqrDist = fA00 + 2.0f * fB0 + fC; }
          else { fS = -fB0 / fA00; fSqrDist = fB0 * fS + fC; }
        } else {
          fS = 0.0f;
          if (fB1 >= 0.0f) { fT = 0.0f; fSqrDist = fC; }
          else if (-fB1 >= fA11) { fT = 1.0f; fSqrDist = fA11 + 2.0f * fB1 + fC; }
          else { fT = -fB1 / fA11; fSqrDist = fB1 * fT + fC; }
        }
      } else {
        fS = 0.0f;
        if (fB1 >= 0.0f) { fT = 0.0f; fSqrDist = fC; }
        else if (-fB1 >= fA11) { fT = 1.0f; fSqrDist = fA11 + 2.0f * fB1 + fC; }
        else { fT = -fB1 / fA11; fSqrDist = fB1 * fT + fC; }
      }
    } else if (fT < 0.0f) {
      fT = 0.0f;
      if (fB0 >= 0.0f) { fS = 0.0f; fSqrDist = fC; }
      else if (-fB0 >= fA00) { fS = 1.0f; fSqrDist = fA00 + 2.0f * fB0 + fC; }
      else { fS = -fB0 / fA00; fSqrDist = fB0 * fS + fC; }
    } else {
      float fInvDet = 1.0f / fDet;
      fS *= fInvDet;
      fT *= fInvDet;
      fSqrDist = fS * (fA00 * fS + fA01 * fT + 2.0f * fB0) + fT * (fA01 * fS + fA11 * fT + 2.0f * fB1) + fC;
    }
  } else {
    float fTmp0, fTmp1, fNumer, fDenom;
    if (fS < 0.0f) {
      fTmp0 = fA01 + fB0;
      fTmp1 = fA11 + fB1;
      if (fTmp1 > fTmp0) {
        fNumer = fTmp1 - fTmp0;
        fDenom = fA00 - 2.0f * fA01 + fA11;
        if (fNumer >= fDenom) { fS = 1.0f; fT = 0.0f; fSqrDist = fA00 + 2.0f * fB0 + fC; }
        else { fS = fNumer / fDenom; fT = 1.0f - fS; fSqrDist = fS * (fA00 * fS + fA01 * fT + 2.0f * fB0) + fT * (fA01 * fS + fA11 * fT + 2.0f * fB1) + fC; }
      } else {
        fS = 0.0f;
        if (fTmp1 <= 0.0f) { fT = 1.0f; fSqrDist = fA11 + 2.0f * fB1 + fC; }
        else if (fB1 >= 0.0f) { fT = 0.0f; fSqrDist = fC; }
        else { fT = -fB1 / fA11; fSqrDist = fB1 * fT + fC; }
      }
    } else if (fT < 0.0f) {
      fTmp0 = fA01 + fB1;
      fTmp1 = fA00 + fB0;
      if (fTmp1 > fTmp0) {
        fNumer = fTmp1 - fTmp0;
        fDenom = fA00 - 2.0f * fA01 + fA11;
        if (fNumer >= fDenom) { fT = 1.0f; fS = 0.0f; fSqrDist = fA11 + 2.0f * fB1 + fC; }
        else { fT = fNumer / fDenom; fS = 1.0f - fT; fSqrDist = fS * (fA00 * fS + fA01 * fT + 2.0f * fB0) + fT * (fA01 * fS + fA11 * fT + 2.0f * fB1) + fC; }
      } else {
        fT = 0.0f;
        if (fTmp1 <= 0.0f) { fS = 1.0f; fSqrDist = fA00 + 2.0f * fB0 + fC; }
        else if (fB0 >= 0.0f) { fS = 0.0f; fSqrDist = fC; }
        else { fS = -fB0 / fA00; fSqrDist = fB0 * fS + fC; }
      }
    } else {
      fNumer = fA11 + fB1 - fA01 - fB0;
      if (fNumer <= 0.0f) { fS = 0.0f; fT = 1.0f; fSqrDist = fA11 + 2.0f * fB1 + fC; }
      else {
        fDenom = fA00 - 2.0f * fA01 + fA11;
        if (fNumer >= fDenom) { fS = 1.0f; fT = 0.0f; fSqrDist = fA00 + 2.0f * fB0 + fC; }
        else { fS = fNumer / fDenom; fT = 1.0f - fS; fSqrDist = fS * (fA00 * fS + fA01 * fT + 2.0f * fB0) + fT * (fA01 * fS + fA11 * fT + 2.0f * fB1) + fC; }
      }
    }
  }
  return (float)fabs(fSqrDist);
}

// collision.h:243-341 tetrahedron_intersect_parallelogram
static bool vecInf(const float a[3], const float b[3]) { return a[0] < b[0] || a[1] < b[1] || a[2] < b[2]; }
static bool tetraCrossesParallelogram(V3 A, V3 B, V3 C, V3 D, V3 v0, V3 v1, V3 v2) {
  float mnT[3] = {A.x, A.y, A.z}, mxT[3] = {A.x, A.y, A.z};
  for (V3 p : {B, C, D}) {
    float q[3] = {p.x, p.y, p.z};
    for (int i = 0; i < 3; i++) { mnT[i] = std::min(mnT[i], q[i]); mxT[i] = std::max(mxT[i], q[i]); }
  }
  float mnP[3] = {v0.x, v0.y, v0.z}, mxP[3] = {v0.x, v0.y, v0.z};
  for (V3 p : {v1, v2}) {
    float q[3] = {p.x, p.y, p.z};
    for (int i = 0; i < 3; i++) { mnP[i] = std::min(mnP[i], q[i]); mxP[i] = std::max(mxP[i], q[i]); }
  }
  if (vecInf(mxP, mnT)) return false;
  if (!vecInf(mnP, mxT)) return false;
  if (vecInf(mxT, mnP)) return false;
  if (!vecInf(mnT, mxP)) return false;
  float t, u, v;
  auto edge = [&](V3 o, V3 e) { return intersect_parallelogram(o, sub(e, o), v0, v1, v2, &t, &u, &v) && t >= 0 && t <= 1.; };
  if (edge(A, B) || edge(A, C) || edge(A, D) || edge(B, D) || edge(B, C) || edge(C, D)) return true;
  auto tri = [&](V3 o, V3 d) {
    if (intersect_triangle(o, d, A, B, C, &t, &u, &v) && t >= 0 && t <= 1.) return true;
    if (intersect_triangle(o, d, A, C, D, &t, &u, &v) && t >= 0 && t <= 1.) return true;
    if (intersect_triangle(o, d, A, B, D, &t, &u, &v) && t >= 0 && t <= 1.) return true;
    if (intersect_triangle(o, d, B, C, D, &t, &u, &v) && t >= 0 && t <= 1.) return true;
    return false;
  };
  if (tri(v0, sub(v2, v0))) return true;
  V3 v3 = add(sub(v0, v2), v1);
  if (tri(v3, sub(v1, v3))) return true;
  return false;
}

static bool barelyEqual(V3 a, V3 b) { return fabs(a.x - b.x) < BEPS && fabs(a.y - b.y) < BEPS && fabs(a.z - b.z) < BEPS; }

void prepare(Config& cfg, Model& m) {
  const int T = m.T;
  // coreinitialisation.cpp:71-96 InitSourcesTetraLocalisation
  for (auto& s : cfg.sources) {
    for (int t = 0; t < T && s.tetra < 0; t++) {
      bool inside = true;
      for (int f = 0; f < 4 && inside; f++) {
        int k = 4 * t + f;
        V3 pa = sub(m.nodes[m.fv[3 * k]], s.pos);
        if (dot(pa, m.fn[k]) > 0) inside = false;
      }
      if (inside) s.tetra = t;
    }
  }
  // coreinitialisation.cpp:178-213 InitRecepteurPTetraLocalisation: the first tetrahedron holding it
  m.rpByTetra.assign(T, {});
  for (size_t r = 0; r < cfg.receivers.size(); r++) {
    PointRx& rx = cfg.receivers[r];
    for (int t = 0; t < T; t++) {
      bool inside = true;
      for (int f = 0; f < 4 && inside; f++) {
        int k = 4 * t + f;
        V3 pa = sub(m.nodes[m.fv[3 * k]], rx.pos);
        if (dot(pa, m.fn[k]) > 0) inside = false;
      }
      if (inside) {
        m.rpByTetra[t].push_back((int)r);
        rx.cdt_vol = cfg.c * cfg.rho;
        rx.tetra = t;
        break;
      }
    }
  }
  // coreinitialisation.cpp:215-239 InitCutMapTetraLocalisation (planeA = B, planeB = A, planeC = C)
  m.cutByTetra.assign(T, {});
  for (size_t ci = 0; ci < cfg.cuts.size(); ci++) {
    const CutRx& c = cfg.cuts[ci];
    for (int t = 0; t < T; t++) {
      const int* k = &m.corner[4 * t];
      if (tetraCrossesParallelogram(m.nodes[k[0]], m.nodes[k[1]], m.nodes[k[2]], m.nodes[k[3]], c.B, c.A, c.C))
        m.cutByTetra[t].push_back((int)ci);
    }
  }
  // coreinitialisation.cpp:151-176 InitEncombrementTetraLocalisation
  {
    int lastPt = -1, lastIdx = 0;
    for (int t = 0; t < T; t++) {
      if (m.idVolume[t] != lastIdx && m.idVolume[t] != 0) {
        lastIdx = m.idVolume[t];
        lastPt = -1;
        for (size_t i = 0; i < cfg.fittings.size(); i++) if (cfg.fittings[i].id == lastIdx) { lastPt = (int)i; break; }
      }
      if (m.idVolume[t] != 0) {
        m.volEnc[t] = lastPt;
        for (int f = 0; f < 4; f++) if (m.sf[4 * t + f] >= 0) m.faces[m.sf[4 * t + f]].enc = lastPt;
      }
    }
  }
  // coreinitialisation.cpp:271-340 InitRecepteurS (tetrahedral variant)
  m.rxFaces.assign(cfg.surfRx.size(), {});
  {
    struct Pending { int rs; int idx; int face; int other; };
    std::vector<Pending> pend;
    int lastLocal = -1, lastXml = -1;
    for (int t = 0; t < T; t++) {
      for (int i = 0; i < 4; i++) {
        int k = 4 * t + i;
        if (m.sf[k] < 0) continue;
        const SceneFace& sfc = m.faces[m.sf[k]];
        if (sfc.idRs != -1 && barelyEqual(sfc.normal, neg(m.fn[k]))) {
          if (lastXml != sfc.idRs) {
            lastLocal = -1;
            for (size_t r = 0; r < cfg.surfRx.size(); r++) if (cfg.surfRx[r].id == sfc.idRs) { lastLocal = (int)r; break; }
            lastXml = sfc.idRs;
          }
          if (lastLocal < 0) continue;   // refused before the run (surface_receiver_undeclared)
          Pending p;
          p.rs = lastLocal;
          p.idx = (int)m.rxFaces[lastLocal].size();
          p.face = k;
          p.other = -1;
          RxFace rf;
          rf.tetraFace = k;
          rf.ia = m.fv[3 * k]; rf.ib = m.fv[3 * k + 1]; rf.ic = m.fv[3 * k + 2];
          rf.p0 = m.nodes[rf.ia]; rf.p1 = m.nodes[rf.ib]; rf.p2 = m.nodes[rf.ic];
          m.rxFaces[lastLocal].push_back(rf);
          int nbt = m.nb[k];
          if (nbt >= 0) {
            // GetTetraFaceWithNormal (coreinitialisation.cpp:38-46): face 0 when none matches
            V3 want = mul(m.fn[k], -1.0f);
            int mirror = 0;
            for (int f = 0; f < 4; f++) if (barelyEqual(m.fn[4 * nbt + f], want)) { mirror = f; break; }
            p.other = 4 * nbt + mirror;
          }
          pend.push_back(p);
        }
      }
    }
    m.rxFaceBase.assign(cfg.surfRx.size(), 0);
    int base = 0;
    for (size_t r = 0; r < cfg.surfRx.size(); r++) { m.rxFaceBase[r] = base; base += (int)m.rxFaces[r].size(); }
    m.nbRxFaces = base;
    for (const Pending& p : pend) {
      int g = m.rxFaceBase[p.rs] + p.idx;
      m.rf[p.face] = g;
      if (p.other >= 0) m.rf[p.other] = g;
    }
  }
  // sppsInitialisation.cpp:43-93 ExpandPunctualReceiverTetrahedronLocalisation
  {
    double volumeRP = (pow(cfg.radius, 3) * (double)(float)3.141592653589793238462643383279 * 4.) / 3.;
    for (size_t r = 0; r < cfg.receivers.size(); r++) {
      PointRx& rx = cfg.receivers[r];
      if (rx.tetra < 0) continue;
      rx.cdt_vol = (float)(((double)(cfg.c * cfg.rho)) / volumeRP);
      std::vector<int> stack;
      for (int k = 0; k < 4; k++) stack.push_back(m.nb[4 * rx.tetra + k]);
      // the recursion visits a tetrahedron once: it links it (and recurses) when one of its faces
      // comes within the radius, and stops where the receiver is already linked; the set reached
      // does not depend on the visiting order
      while (!stack.empty()) {
        int t = stack.back();
        stack.pop_back();
        if (t < 0) continue;
        auto& lst = m.rpByTetra[t];
        if (std::find(lst.begin(), lst.end(), (int)r) != lst.end()) continue;
        for (int f = 0; f < 4; f++) {
          int k = 4 * t + f;
          float minLength = sqrtf(closestDistSq(m.nodes[m.fv[3 * k]], m.nodes[m.fv[3 * k + 1]], m.nodes[m.fv[3 * k + 2]], rx.pos));
          if (minLength <= cfg.radius) {
            lst.push_back((int)r);
            for (int v = 3; v >= 0; v--) stack.push_back(m.nb[4 * t + v]);
            break;
          }
        }
      }
    }
  }
  // sppsInitialisation.cpp:13-34 TranslateSourceAtTetrahedronVertex
  for (auto& s : cfg.sources) {
    if (s.tetra < 0) continue;   // SPPS dereferences NULL here; refused before the run (source_outside_mesh)
    const int* c = &m.corner[4 * s.tetra];
    for (int v = 0; v < 4; v++) {
      double d = distanceD(s.pos, m.nodes[c[v]]);
      if (d < BEPS) {
        V3 center = mul(add(add(add(m.nodes[c[0]], m.nodes[c[1]]), m.nodes[c[2]]), m.nodes[c[3]]), 0.25f);
        std::cout << "Source at tetrahedron vertex, move source position from [" << s.pos.x << ";" << s.pos.y << ";" << s.pos.z << "]";
        s.pos = add(s.pos, mul(sub(center, s.pos), 0.005f));
        std::cout << " to [" << s.pos.x << ";" << s.pos.y << ";" << s.pos.z << "]" << std::endl;
        break;
      }
    }
  }
}

// sppsInitialisation.cpp:95-111 CheckSourcePosition with mathlib.h:725-744 DotIsInVertex (host only)
bool checkSourcePosition(const Config& cfg, const Model& m) {
  auto angleH = [](V3 a, V3 b) -> float {
    float an = acosf(dot(a, b) / (len(a) * len(b)));
    if (an < EPS) return 0;
    return an;
  };
  for (const auto& s : cfg.sources) {
    for (const auto& f : m.faces) {
      V3 a = m.sceneVerts[f.a], b = m.sceneVerts[f.b], c = m.sceneVerts[f.c];
      V3 vda = sub(s.pos, a), vdb = sub(s.pos, b), vdc = sub(s.pos, c);
      float tot = 0;
      tot += angleH(vda, vdb);
      tot += angleH(vda, vdc);
      tot += angleH(vdb, vdc);
      float ecart = (float)fabs(TWOPI_F - tot);
      bool in = (int(tot * 10) == int(TWOPI_F * 10)) || !std::isfinite(tot);
      if (in && ecart < BEPS) return false;
    }
  }
  return true;
}

std::vector<std::string> refusals(const Config& cfg, const Model& m) {
  std::vector<std::string> r;
  if (cfg.alog != 0 || cfg.blin != 0)
    r.push_back("stratified_unsupported: condition_atmospherique alog/blin set a sound-speed gradient; SPPS re-aims every particle each step in a stratified medium (CalculationCore.cpp:80-84, 389-410), which spps-gpu does not port in A2");
  if (cfg.encCalc) {
    for (int t = 0; t < m.T; t++)
      if (m.volEnc[t] >= 0) {
        r.push_back("fittings_unsupported: enc_calc is on and the mesh holds fitting volumes (encombrement); the fitting walk (CalculationCore.cpp:32-39, 132-185) comes in A3");
        break;
      }
  }
  for (size_t i = 0; i < cfg.sources.size(); i++) {
    int ty = cfg.sources[i].type;
    if (ty == 5) r.push_back("directivity_balloon_unsupported: source '" + cfg.sources[i].name + "' uses a measured directivity balloon (directivite 5), which comes in A3");
    else if (ty < 0 || ty > 5) r.push_back("source_type_unknown: source '" + cfg.sources[i].name + "' has directivite " + std::to_string(ty) + ", for which SPPS has no branch");
    if (cfg.sources[i].tetra < 0)
      r.push_back("source_outside_mesh: source '" + cfg.sources[i].name + "' lies in no tetrahedron (SPPS crashes there with 0xC0000005, sppsInitialisation.cpp:13-20)");
  }
  if (cfg.nbSteps > 65535)
    r.push_back("too_many_steps: " + std::to_string(cfg.nbSteps) + " time steps; SPPS counts steps in 16 bits (uentier_court pasCourant) and the .csbin stores them in 16 bits");
  for (const auto& f : m.faces)
    if (f.mat < 0) { r.push_back("material_missing: a scene face has material id 0 and no material 0 is declared (SPPS dereferences NULL there)"); break; }
  {
    bool undeclared = false;
    for (int k = 0; k < 4 * m.T && !undeclared; k++) {
      if (m.sf[k] < 0) continue;
      int id = m.faces[m.sf[k]].idRs;
      if (id == -1) continue;
      bool found = false;
      for (const auto& s : cfg.surfRx) if (s.id == id) found = true;
      if (!found) undeclared = true;
    }
    if (undeclared) r.push_back("surface_receiver_undeclared: a scene face names a surface receiver id that config.xml does not declare (SPPS indexes its list at -1 there)");
  }
  for (const auto& b : cfg.bands) (void)b;
  return r;
}

}  // namespace spg
