// spps-gpu: SPPS's output files. Receipts are paths under B:/repos/I-Simpa-upstream/src.
#include "output.h"
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <iostream>

namespace spg {

namespace fs = std::filesystem;
static const char* SEP = "\\";   // st_path_separator (std_tools.cpp:52-56) on Windows

void mkdirs(const std::string& path) {
  std::error_code ec;
  fs::create_directories(fs::u8path(path), ec);
}
static bool exists(const std::string& path) {
  std::error_code ec;
  return fs::exists(fs::u8path(path), ec);
}

// ---------------------------------------------------------------------------------------------
// GABE::Save (gabe.cpp:455-515): fields one by one; seekp over padding, so a seek that ends the
// file writes nothing and one followed by data leaves zeros.
class Writer {
 public:
  std::string buf;
  size_t pending = 0;
  void skip(size_t n) { pending += n; }
  void raw(const void* p, size_t n) {
    if (n == 0) return;
    if (pending) { buf.append(pending, '\0'); pending = 0; }
    buf.append((const char*)p, n);
  }
  template <class T> void put(T v) { raw(&v, sizeof(T)); }
  bool save(const std::string& path) {
    std::ofstream o(fs::u8path(path), std::ios::binary | std::ios::trunc);
    if (!o) return false;
    o.write(buf.data(), (std::streamsize)buf.size());
    return (bool)o;
  }
};

bool saveGabe(const std::string& path, bool readOnly, const std::vector<GCol>& cols) {
  Writer w;
  w.put<int32_t>(2);
  w.put<int32_t>(0);
  w.put<int32_t>(0);
  w.put<int32_t>((int32_t)cols.size());
  w.put<uint8_t>(readOnly ? 1 : 0);
  w.skip(3);
  for (const GCol& c : cols) {
    int32_t rows = c.type == 50 ? (int32_t)c.f.size() : c.type == 51 ? (int32_t)c.i.size() : (int32_t)c.s.size();
    int64_t elem = c.type == 52 ? 50 : 4;
    w.put<uint16_t>((uint16_t)c.type);
    w.skip(2);
    w.put<int32_t>(rows);
    w.put<int64_t>(elem * rows);
    w.put<int64_t>(c.type == 50 ? 4 : 1);
    char label[255];
    memset(label, 0, sizeof(label));
    strncpy(label, c.label.c_str(), sizeof(label));   // GABE_Object::SetLabel
    w.raw(label, sizeof(label));
    w.skip(1);
    if (c.type == 50) {
      w.put<int32_t>(c.digits);
      w.raw(c.f.data(), 4 * c.f.size());
    } else if (c.type == 51) {
      w.skip(1);
      w.raw(c.i.data(), 4 * c.i.size());
    } else {
      w.skip(1);
      for (const std::string& s : c.s) {
        char cell[50];
        memset(cell, 0, sizeof(cell));
        // SetString strcpy's into 50 bytes (gabe.cpp:174-178); a longer name overflows upstream, here it is cut
        memcpy(cell, s.data(), s.size() < 50 ? s.size() : 50);
        w.raw(cell, 50);
      }
    }
  }
  return w.save(path);
}

static GCol fcol(const std::string& label, size_t n) { GCol c; c.type = 50; c.label = label; c.f.assign(n, 0.f); return c; }
static std::string fromInt(int v) { char b[32]; snprintf(b, sizeof(b), "%i", v); return b; }
static std::string hz(int f) { return fromInt(f) + " Hz"; }

// ---------------------------------------------------------------------------------------------
// RSBIN::ExportBIN (rsbin.cpp:150-200): the C structs as written (padding zeroed here; upstream
// leaves heap garbage there, which no reader reads)
struct CsbinFace { int32_t v[3]; std::vector<std::pair<uint16_t, float>> rec; };
struct CsbinRx { int32_t xml; std::string name; std::vector<CsbinFace> faces; };
static bool saveCsbin(const std::string& path, int nbTimeStep, float timeStep, const std::vector<V3>& nodes, const std::vector<CsbinRx>& rxs) {
  std::ofstream o(fs::u8path(path), std::ios::binary | std::ios::trunc);
  if (!o) return false;
  int32_t h[11];
  h[0] = 3; h[1] = 44; h[2] = 12; h[3] = 264; h[4] = 16; h[5] = 8;
  h[6] = (int32_t)nodes.size(); h[7] = (int32_t)rxs.size(); h[8] = nbTimeStep;
  memcpy(&h[9], &timeStep, 4);
  h[10] = 0;
  o.write((const char*)h, 44);
  for (const V3& n : nodes) { float f[3] = {n.x, n.y, n.z}; o.write((const char*)f, 12); }
  std::vector<char> blk;
  for (const CsbinRx& r : rxs) {
    char rec[264];
    memset(rec, 0, sizeof(rec));
    int32_t nf = (int32_t)r.faces.size();
    memcpy(rec, &r.xml, 4);
    memcpy(rec + 4, &nf, 4);
    memcpy(rec + 8, r.name.data(), r.name.size() < 254 ? r.name.size() : 254);
    o.write(rec, 264);
    for (const CsbinFace& f : r.faces) {
      int32_t fh[4] = {f.v[0], f.v[1], f.v[2], (int32_t)f.rec.size()};
      o.write((const char*)fh, 16);
      blk.assign(8 * f.rec.size(), 0);
      for (size_t i = 0; i < f.rec.size(); i++) {
        memcpy(&blk[8 * i], &f.rec[i].first, 2);
        memcpy(&blk[8 * i + 4], &f.rec[i].second, 4);
      }
      if (!blk.empty()) o.write(blk.data(), (std::streamsize)blk.size());
    }
  }
  return (bool)o;
}

// GetAireTriangle (mathlib.h:711-720): .5 * |ab x ac|, with Vector(a, b) = a - b
static float aire(V3 a, V3 b, V3 c) {
  V3 ab = sub(a, b), ac = sub(a, c);
  V3 x = mk(ab.y * ac.z - ab.z * ac.y, ab.z * ac.x - ab.x * ac.z, ab.x * ac.y - ab.y * ac.x);
  return (float)(.5 * len(x));
}

Report::Report(Config& c, Model& mm) : cfg(c), m(mm) {
  surfGlobal.assign((size_t)m.nbRxFaces * cfg.nbBins, 0.f);
  size_t cells = 0;
  for (const auto& cp : cfg.cuts) cells += (size_t)cp.nu * cp.nv;
  cutGlobal.assign(cells * cfg.nbBins, 0.f);
  // sppsNantes.cpp:341-342: the surface folder, when there are surface receivers or planes
  if (!cfg.surfRx.empty() || !cfg.cuts.empty()) mkdirs(cfg.wd + cfg.rssDir);
}

// BaseReportManager::SauveRecepteursSurfaciques / SauveGlobalRecepteursSurfaciques with
// ExportSimpleBande / ExportTouteBande (baseReportManager.cpp:35-138, 331-358, 391-417): the
// tetrahedral mesh's nodes, each receiver's faces, the bins holding energy, divided by the face area.
void Report::saveSurf(const std::string& path, const std::vector<float>& data, bool global) {
  if (cfg.surfRx.empty() || m.rxFaces[0].empty()) return;
  std::vector<CsbinRx> rxs;
  for (size_t r = 0; r < cfg.surfRx.size(); r++) {
    CsbinRx x;
    x.xml = cfg.surfRx[r].id;
    x.name = cfg.surfRx[r].name;
    for (size_t f = 0; f < m.rxFaces[r].size(); f++) {
      const RxFace& rf = m.rxFaces[r][f];
      CsbinFace cf;
      cf.v[0] = rf.ia; cf.v[1] = rf.ib; cf.v[2] = rf.ic;
      float area = aire(rf.p0, rf.p1, rf.p2);
      const float* d = &data[((size_t)m.rxFaceBase[r] + f) * cfg.nbBins];
      for (int t = 0; t < cfg.nbBins; t++)
        if (d[t] != 0) cf.rec.push_back({(uint16_t)t, global ? d[t] : d[t] / area});
      x.faces.push_back(std::move(cf));
    }
    rxs.push_back(std::move(x));
  }
  saveCsbin(path, cfg.nbBins, cfg.binDt, m.nodes, rxs);
}

// BaseReportManager::SauveRecepteursSurfaciquesCoupe (baseReportManager.cpp:211-330)
void Report::saveCut(const std::string& path, const std::vector<float>& data, bool /*global*/) {
  if (cfg.cuts.empty()) return;
  std::vector<V3> nodes;
  std::vector<CsbinRx> rxs;
  size_t cellBase = 0;
  int nodecount = 0;
  for (const CutRx& c : cfg.cuts) {
    V3 B = c.B, BC = sub(c.C, B), BA = sub(c.A, B);
    V3 stepU = mul(divs(BC, len(BC)), c.usize);
    V3 stepV = mul(divs(BA, len(BA)), c.vsize);
    int nbrows = c.nu, nbcols = c.nv, nbvertrow = nbrows + 1, nbvertcol = nbcols + 1;
    size_t first = nodes.size();
    nodes.resize(first + (size_t)nbvertrow * nbvertcol);
    for (int r = 0; r < nbvertrow; r++)
      for (int col = 0; col < nbvertcol; col++)
        nodes[first + col + (size_t)r * nbvertcol] = add(B, add(mul(stepU, (float)r), mul(stepV, (float)col)));
    CsbinRx x;
    x.xml = c.id;
    x.name = c.name;
    float tricell = aire(add(B, stepV), B, add(B, stepU)) * 2;   // divBySurface is true on both calls
    x.faces.resize((size_t)nbrows * nbcols * 2);
    for (int r = 0; r < nbrows; r++) {
      for (int col = 0; col < nbcols; col++) {
        size_t idFace = ((size_t)col + (size_t)r * nbcols) * 2;
        CsbinFace& f1 = x.faces[idFace];
        CsbinFace& f2 = x.faces[idFace + 1];
        f1.v[0] = nodecount + (col + 1 + r * nbvertcol);
        f1.v[1] = nodecount + (col + r * nbvertcol);
        f1.v[2] = nodecount + (col + (r + 1) * nbvertcol);
        f2.v[0] = f1.v[0];
        f2.v[1] = f1.v[2];
        f2.v[2] = nodecount + (col + 1 + (r + 1) * nbvertcol);
        const float* d = &data[(cellBase + (size_t)r * nbcols + col) * cfg.nbBins];
        for (int t = 0; t < cfg.nbBins; t++)
          if (d[t] != 0) f1.rec.push_back({(uint16_t)t, d[t] / tricell});
        f2.rec = f1.rec;
      }
    }
    rxs.push_back(std::move(x));
    nodecount += nbvertrow * nbvertcol;
    cellBase += (size_t)c.nu * c.nv;
  }
  saveCsbin(path, cfg.nbBins, cfg.binDt, nodes, rxs);
}

void Report::band(BandSums& b) {
  const Band& bd = cfg.bands[b.band];
  const int S = cfg.nbSteps, R = (int)cfg.receivers.size(), NS = (int)cfg.sources.size();
  // sppsNantes.cpp:183-193
  std::string surfPath = cfg.wd + cfg.rssDir, cutPath;
  if ((!cfg.surfRx.empty() || !cfg.cuts.empty()) && cfg.byFreq) {
    surfPath += fromInt(bd.freq) + " Hz/";
    cutPath = surfPath;
    mkdirs(surfPath);
    surfPath += cfg.rssFile;
    cutPath += cfg.rssCutFile;
  }
  // reportmanager.cpp:540-578 SetPostProcess(Cut)SurfaceReceiver: surf_receiv_method 1 only
  if (cfg.surfMode == 1) {
    float i0 = (float)(pow(10.f, -12.f) / pow((float)(20 * pow(10.f, (int)-6)), (int)2) * cfg.rho * cfg.c);
    for (float& v : b.surf) v *= i0;
    for (float& v : b.cut) v *= i0;
  }
  if (!cfg.surfRx.empty() && cfg.byFreq) saveSurf(surfPath, b.surf, false);
  if (!cfg.cuts.empty() && cfg.byFreq) saveCut(cutPath, b.cut, false);
  // the Global maps: ExportTouteBande adds value / area band by band; the cut file adds the values
  if (!cfg.surfRx.empty()) {
    for (size_t r = 0; r < cfg.surfRx.size(); r++)
      for (size_t f = 0; f < m.rxFaces[r].size(); f++) {
        const RxFace& rf = m.rxFaces[r][f];
        float area = aire(rf.p0, rf.p1, rf.p2);
        size_t o = ((size_t)m.rxFaceBase[r] + f) * cfg.nbBins;
        for (int t = 0; t < cfg.nbBins; t++)
          if (b.surf[o + t] != 0) surfGlobal[o + t] += b.surf[o + t] / area;
      }
  }
  for (size_t i = 0; i < b.cut.size(); i++) cutGlobal[i] += b.cut[i];

  BandCols bc;
  bc.band = b.band;
  // reportmanager.cpp:345-359 GetColStats
  bc.stats.type = 51;
  bc.stats.label = hz(bd.freq);
  uint64_t tot = 0;
  for (int k = 0; k < 6; k++) tot += b.states[k];
  bc.stats.i = {(int32_t)b.states[ABS_ATMO], (int32_t)b.states[ABS_SURF], (int32_t)b.states[ABS_ENC], (int32_t)b.states[LOOP],
                (int32_t)b.states[LOST], (int32_t)b.states[ALIVE], (int32_t)tot};
  lostOrLoop += b.states[LOST] + b.states[LOOP];
  total += tot;
  // reportmanager.cpp:426-438 GetSumEnergy
  double cdtRho = (double)(cfg.rho * cfg.c);
  bc.sumEnergy = fcol(hz(bd.freq), S);
  for (int s = 0; s < S; s++) bc.sumEnergy.f[s] = (float)(b.total[s] * cdtRho);
  // reportmanager.cpp:360-424 FillWithLefData
  float volRp = (float)((pow(cfg.radius, 3) * (double)(float)3.141592653589793238462643383279 * 4.) / 3.);
  size_t srcCols = cfg.bySource ? (size_t)S * NS : (size_t)NS;
  for (int r = 0; r < R; r++) {
    const PointRx& rx = cfg.receivers[r];
    bc.energy.push_back(std::vector<double>(b.rpE.begin() + (size_t)r * S, b.rpE.begin() + (size_t)(r + 1) * S));
    GCol lf = fcol("", S), lfc = fcol("", S);
    GCol ix = fcol(hz(bd.freq) + "\nx", S + 1), iy = fcol(hz(bd.freq) + "\ny", S + 1), iz = fcol(hz(bd.freq) + "\nz", S + 1);
    double cx = 0, cy = 0, cz = 0;
    double inv = 1.0f / (double)volRp;
    for (int s = 0; s < S; s++) {
      size_t k = (size_t)r * S + s;
      lfc.f[s] = (float)(b.rpLfc[k] * rx.cdt_vol);
      lf.f[s] = (float)(b.rpLf[k] * rx.cdt_vol);
      double vx = b.rpI[3 * k], vy = b.rpI[3 * k + 1], vz = b.rpI[3 * k + 2];
      ix.f[s] = (float)(vx / volRp);
      iy.f[s] = (float)(vy / volRp);
      iz.f[s] = (float)(vz / volRp);
      cx += vx * inv; cy += vy * inv; cz += vz * inv;
    }
    ix.f[S] = (float)cx; iy.f[S] = (float)cy; iz.f[S] = (float)cz;
    GCol bs = fcol(hz(bd.freq), NS);
    const double* sc = &b.rpSrc[(size_t)r * srcCols];
    for (int src = 0; src < NS; src++) {
      double sum = 0;
      if (cfg.bySource) for (int s = 0; s < S; s++) sum += sc[(size_t)s * NS + src];
      else sum = sc[src];
      bs.f[src] = (float)(sum * rx.cdt_vol);
    }
    bc.lf.push_back(std::move(lf));
    bc.lfc.push_back(std::move(lfc));
    bc.ix.push_back(std::move(ix));
    bc.iy.push_back(std::move(iy));
    bc.iz.push_back(std::move(iz));
    bc.bySrc.push_back(std::move(bs));
    if (cfg.bySource) bc.srcContrib.push_back(std::vector<double>(sc, sc + srcCols));
  }
  cols.push_back(std::move(bc));
}

void Report::finish() {
  const int S = cfg.nbSteps, R = (int)cfg.receivers.size(), NS = (int)cfg.sources.size();
  std::cout << "End of calculation." << std::endl;
  // coreinitialisation.cpp:473-488 reportCompilation, baseReportManager.cpp:419-429 InitHeaderArrays
  std::cout << "Output results files." << std::endl;
  std::vector<std::string> lblFreq, lblTime;
  for (const Band& b : cfg.bands) lblFreq.push_back(hz(b.freq));
  for (int i = 0; i < S; i++) {
    char buf[64];
    snprintf(buf, sizeof(buf), "%.1f", (double)(cfg.dt * (float)(i + 1) * 1000.f));
    lblTime.push_back(std::string(buf) + " ms");
  }
  std::string rootRp = cfg.wd + cfg.rpDir + SEP;
  mkdirs(cfg.wd);
  mkdirs(rootRp);
  auto colsFor = [&](int bandIndex) -> const BandCols* {
    for (const BandCols& c : cols) if (c.band == bandIndex) return &c;
    return nullptr;
  };
  // baseReportManager.cpp:150-209 SauveRecepteursPonctuels / SauveRecepteurPonctuel
  auto saveRecp = [&](const std::string& file, const std::vector<const std::vector<double>*>& energyByBand, float cdt_vol) {
    std::vector<GCol> g;
    GCol l;
    l.type = 52; l.label = "SPL"; l.s = lblTime;
    g.push_back(l);
    for (size_t f = 0; f < cfg.bands.size(); f++) {
      if (!energyByBand[f]) continue;
      GCol c = fcol(lblFreq[f], S);
      for (int s = 0; s < S; s++) c.f[s] = (float)((*energyByBand[f])[s] * cdt_vol);
      g.push_back(std::move(c));
    }
    saveGabe(file, true, g);
  };
  for (int r = 0; r < R; r++) {
    PointRx& rx = cfg.receivers[r];
    std::string folder = rootRp + rx.lbl;
    int counter = 0;
    while (exists(folder) && counter < 20) folder = rootRp + rx.lbl + fromInt(counter++);
    rx.pathRp = folder + SEP;
    mkdirs(folder);
    std::vector<const std::vector<double>*> e(cfg.bands.size(), nullptr);
    for (size_t f = 0; f < cfg.bands.size(); f++) if (const BandCols* c = colsFor((int)f)) e[f] = &c->energy[r];
    saveRecp(folder + SEP + cfg.rpFile, e, rx.cdt_vol);
  }
  // reportmanager.cpp:482-539 SaveThreadsStats
  {
    std::vector<GCol> g;
    GCol l;
    l.type = 52;
    l.s = {"Particles absorbed by the atmosphere", "Particles absorbed by the materials", "Particles absorbed by the fittings",
           "Particles lost by infinite loops", "Particles lost by meshing problems", "Particles remaining at the end of the calculation", "Total"};
    g.push_back(l);
    for (size_t f = 0; f < cfg.bands.size(); f++) if (const BandCols* c = colsFor((int)f)) g.push_back(c->stats);
    saveGabe(cfg.wd + cfg.statsFile, true, g);
    std::vector<GCol> h;
    GCol t;
    t.type = 52; t.label = "SPL";
    for (int i = 0; i < S; i++) t.s.push_back(fromInt((int)(cfg.dt * (float)(i + 1) * 1000)) + " ms");
    h.push_back(t);
    for (size_t f = 0; f < cfg.bands.size(); f++) if (const BandCols* c = colsFor((int)f)) h.push_back(c->sumEnergy);
    saveGabe(cfg.wd + cfg.cumulFile, false, h);
  }
  int nbUsed = (int)cols.size();
  // reportmanager.cpp:760-873 SaveRecpAcousticParamsAdvance
  {
    GCol idx; idx.type = 51;
    idx.i = {1, 2, 3, 4, 5, nbUsed, 3, S};
    GCol dec = fcol("", 2);
    dec.f[0] = cfg.dt;
    dec.f[1] = (float)S * cfg.dt;
    GCol srcCol = fcol("", nbUsed);
    GCol freqCol; freqCol.type = 51;
    int line = 0;
    for (size_t f = 0; f < cfg.bands.size(); f++) {
      if (!cfg.bands[f].docalc) continue;
      float cumul = 0;
      for (const Source& s : cfg.sources) cumul += s.w[f];
      srcCol.f[line] = cumul * cfg.rho * cfg.c;
      freqCol.i.push_back(cfg.bands[f].freq);
      line++;
    }
    for (int r = 0; r < R; r++) {
      const PointRx& rx = cfg.receivers[r];
      std::vector<GCol> g = {idx, dec, srcCol, freqCol};
      GCol noise = fcol("", nbUsed);
      int k = 0;
      for (size_t f = 0; f < cfg.bands.size(); f++) if (cfg.bands[f].docalc) noise.f[k++] = rx.noiseDb[f];
      g.push_back(noise);
      for (size_t f = 0; f < cfg.bands.size(); f++) {
        const BandCols* c = colsFor((int)f);
        if (!c) continue;
        GCol e = fcol("", S);
        for (int s = 0; s < S; s++) e.f[s] = (float)(c->energy[r][s] * rx.cdt_vol);
        g.push_back(e);
        g.push_back(c->lf[r]);
        g.push_back(c->lfc[r]);
      }
      saveGabe(rx.pathRp + cfg.rpFileAdv, false, g);
    }
  }
  // reportmanager.cpp:658-759 SaveRecpIntensity
  {
    GCol l; l.type = 52; l.label = "Intensity";
    for (int i = 0; i < S; i++) l.s.push_back(fromInt((int)(cfg.dt * (float)(i + 1) * 1000)) + " ms");
    l.s.push_back("Sum");
    for (int r = 0; r < R; r++) {
      std::vector<GCol> g = {l};
      for (size_t f = 0; f < cfg.bands.size(); f++) {
        const BandCols* c = colsFor((int)f);
        if (!c) continue;
        g.push_back(c->ix[r]); g.push_back(c->iy[r]); g.push_back(c->iz[r]);
      }
      saveGabe(cfg.receivers[r].pathRp + "Punctual receiver intensity.gabe", true, g);
    }
    std::string work = cfg.wd + "Intensity animation" + "/";
    mkdirs(work);
    for (size_t f = 0; f < cfg.bands.size(); f++) {
      const BandCols* c = colsFor((int)f);
      if (!c) continue;
      std::string dir = work + fromInt(cfg.bands[f].freq) + " Hz/";
      mkdirs(dir);
      GCol ip; ip.type = 51; ip.i = {R, S, 3, 2};
      GCol fp = fcol("", 1); fp.f[0] = cfg.dt;
      std::vector<GCol> g = {ip, fp};
      for (int r = 0; r < R; r++) {
        const GCol* dims[3] = {&c->ix[r], &c->iy[r], &c->iz[r]};
        float pos[3] = {cfg.receivers[r].pos.x, cfg.receivers[r].pos.y, cfg.receivers[r].pos.z};
        for (int d = 0; d < 3; d++) {
          GCol sp = fcol("", dims[d]->f.size() + 1);
          sp.f[0] = pos[d];
          for (size_t s = 0; s < dims[d]->f.size(); s++) sp.f[s + 1] = dims[d]->f[s];
          g.push_back(std::move(sp));
        }
      }
      saveGabe(dir + "Intensity.rpi", true, g);
    }
  }
  // reportmanager.cpp:579-657 SaveSoundLevelBySource
  {
    GCol names; names.type = 52; names.label = "SPL";
    for (const Source& s : cfg.sources) names.s.push_back(s.name);
    for (int r = 0; r < R; r++) {
      std::vector<GCol> g = {names};
      for (size_t f = 0; f < cfg.bands.size(); f++) if (const BandCols* c = colsFor((int)f)) g.push_back(c->bySrc[r]);
      saveGabe(cfg.receivers[r].pathRp + "Sound level per source.recps", true, g);
    }
    if (cfg.bySource) {
      for (int src = 0; src < NS; src++) {
        for (int r = 0; r < R; r++) {
          std::vector<std::vector<double>> per(cfg.bands.size());
          std::vector<const std::vector<double>*> e(cfg.bands.size(), nullptr);
          for (size_t f = 0; f < cfg.bands.size(); f++) {
            const BandCols* c = colsFor((int)f);
            if (!c) continue;
            per[f].resize(S);
            for (int s = 0; s < S; s++) per[f][s] = c->srcContrib[r][(size_t)s * NS + src];
            e[f] = &per[f];
          }
          std::string file = rootRp + cfg.receivers[r].lbl + "/" + cfg.sources[src].name + SEP;
          mkdirs(file);
          file += cfg.rpFile;
          saveRecp(file, e, cfg.receivers[r].cdt_vol);
        }
      }
    }
  }
  // sppsNantes.cpp:408-422: the Global folder, always; its maps when there are receivers
  mkdirs(cfg.wd + cfg.rssDir);
  std::string global = cfg.wd + cfg.rssDir + "Global" + SEP;
  mkdirs(global);
  saveSurf(global + cfg.rssFile, surfGlobal, true);
  saveCut(global + cfg.rssCutFile, cutGlobal, true);
  // sppsNantes.cpp:425-439
  if ((double)lostOrLoop / (double)total > 0.05)
    fprintf(stderr, "Warning %li particles has been in error on %li particles. The computation result may be wrong, please check the particles statitics file for more details.",
            (long)lostOrLoop, (long)total);
}

// ---------------------------------------------------------------------------------------------
// B3: the live stream (output.h)
bool LiveStream::open(const std::string& p, const Config& cfg) {
  path = p;
  out.open(fs::u8path(path), std::ios::out | std::ios::binary | std::ios::trunc);
  if (!out) return false;
  std::vector<int32_t> calc;
  for (const Band& b : cfg.bands)
    if (b.docalc) calc.push_back(b.freq);
  uint32_t h[6];
  h[0] = MAGIC; h[1] = VERSION;
  memcpy(&h[2], &cfg.dt, 4);
  h[3] = (uint32_t)cfg.nbSteps;
  h[4] = (uint32_t)(cfg.nbPartRender * cfg.sources.size());
  h[5] = (uint32_t)calc.size();
  out.write((const char*)h, sizeof(h));
  if (!calc.empty()) out.write((const char*)calc.data(), (std::streamsize)(calc.size() * 4));
  out.flush();
  bytes = sizeof(h) + calc.size() * 4;
  return (bool)out;
}
void LiveStream::frame(int bandHz, uint32_t index, uint32_t firstStep, const std::vector<float>& xyze) {
  if (!out.is_open()) return;
  auto t0 = std::chrono::steady_clock::now();
  const uint32_t n = (uint32_t)(xyze.size() / 4);
  const size_t total = 4 + 16 + 16ull * n;
  buf.resize(total);
  char* p = buf.data();
  uint32_t len = (uint32_t)(total - 4);
  int32_t band = bandHz;
  memcpy(p, &len, 4); memcpy(p + 4, &band, 4); memcpy(p + 8, &index, 4); memcpy(p + 12, &firstStep, 4); memcpy(p + 16, &n, 4);
  float* pos = (float*)(p + 20);
  float* en = pos + 3ull * n;
  for (uint32_t k = 0; k < n; k++) {
    pos[3 * k] = xyze[4 * k]; pos[3 * k + 1] = xyze[4 * k + 1]; pos[3 * k + 2] = xyze[4 * k + 2];
    en[k] = xyze[4 * k + 3];
  }
  out.write(p, (std::streamsize)total);
  out.flush();
  frames++;
  bytes += total;
  seconds += std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
}
void LiveStream::close(bool keep) {
  if (!out.is_open()) return;
  out.close();
  if (!keep) {
    std::error_code ec;
    removed = fs::remove(fs::u8path(path), ec) && !ec;
  }
}

// reportmanager.cpp:90-146 writeParticleFile
ParticleFiles::ParticleFiles(const Config& c, int band, LiveStream* l) : cfg(c), live(l), bandHz(c.bands[band].freq) {
  std::string partPath = cfg.wd + cfg.partDir;
  mkdirs(partPath);
  std::string freqFolder = partPath + fromInt(cfg.bands[band].freq) + SEP;
  mkdirs(freqFolder);
  pbin.open(fs::u8path(freqFolder + cfg.partFile), std::ios::out | std::ios::binary | std::ios::trunc);
  if (cfg.saveSurf) {
    csvS.open(fs::u8path(freqFolder + "particle_surface_collision_statistics.csv"), std::ios::out | std::ios::trunc);
    csvS << "id,collision coordinate,face normal,reflection order,incident vector,energy" << std::endl;
    haveS = true;
  }
  if (cfg.saveRp) {
    csvR.open(fs::u8path(freqFolder + "particle_receivers_collision_statistics.csv"), std::ios::out | std::ios::trunc);
    csvR << "time(s),receiver name,incident vector x,incident vector y,incident vector z,energy * dist" << std::endl;
    haveR = true;
  }
  uint32_t h[7];
  h[0] = (uint32_t)(cfg.nbPartRender * cfg.sources.size());
  h[1] = 1; h[2] = 28; h[3] = 16; h[4] = 8;
  h[5] = (uint32_t)cfg.nbSteps;
  memcpy(&h[6], &cfg.dt, 4);
  pbin.write((const char*)h, 28);
  open = true;
}
ParticleFiles::~ParticleFiles() { close(); }
void ParticleFiles::newParticle() { pos.clear(); firstStep = -1; }
void ParticleFiles::step(const V3& p, double E, int s) {
  if (firstStep == -1) firstStep = s;
  pos.push_back(p.x); pos.push_back(p.y); pos.push_back(p.z); pos.push_back((float)E);
}
void ParticleFiles::surfHit(const V3& n, int order, const V3& at, const V3& dir, double E) { sh.push_back({n, order, at, dir, E}); }
void ParticleFiles::rpHit(float time, int rp, const V3& dir, double E) { rh.push_back({time, rp, dir, E}); }
// reportmanager.cpp:292-335 CloseLastParticleHeader
void ParticleFiles::save() {
  if (!pos.empty()) {
    real++;
    uint32_t n = (uint32_t)(pos.size() / 4);
    char hdr[8];
    memset(hdr, 0, 8);
    uint16_t fs16 = (uint16_t)firstStep;
    memcpy(hdr, &n, 4);
    memcpy(hdr + 4, &fs16, 2);
    pbin.write(hdr, 8);
    pbin.write((const char*)pos.data(), (std::streamsize)(pos.size() * 4));
    // B3: the same particle, the same records, to the live stream as it is saved
    if (live) live->frame(bandHz, real - 1, (uint32_t)fs16, pos);
    pos.clear();
  }
  if (haveS)
    for (const SHit& e : sh)
      csvS << real << "," << e.at.x << " " << e.at.y << " " << e.at.z << "," << e.n.x << " " << e.n.y << " " << e.n.z << "," << e.order << ","
           << e.dir.x << " " << e.dir.y << " " << e.dir.z << "," << e.E << std::endl;
  sh.clear();
  if (haveR)
    for (const RHit& e : rh)
      csvR << e.time << "," << cfg.receivers[e.rp].lbl << "," << e.dir.x << "," << e.dir.y << "," << e.dir.z << "," << e.E << std::endl;
  rh.clear();
}
// reportmanager.cpp:337-343, 439-468
void ParticleFiles::close() {
  if (!open) return;
  pbin.seekp(0);
  pbin.write((const char*)&real, 4);
  pbin.close();
  if (haveS) csvS.close();
  if (haveR) csvR.close();
  open = false;
}

}  // namespace spg
