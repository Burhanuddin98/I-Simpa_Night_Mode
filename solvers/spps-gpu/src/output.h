// spps-gpu: the files SPPS writes, in its formats (docs/formats/gabe.md, csbin.md, pbin.md), names and
// folders (spps/sppsNantes.cpp, spps/input_output/reportmanager.cpp, lib_interface/coreinitialisation.cpp,
// lib_interface/input_output/baseReportManager.cpp).
#pragma once
#include <cstdint>
#include <fstream>
#include <string>
#include <vector>
#include "model.h"

namespace spg {

// One computed band's sums, on the host, as the walk left them.
struct BandSums {
  int band = -1;
  std::vector<double> total;               // nbSteps: tabEnergyByTimeStep
  std::vector<double> rpE, rpLf, rpLfc;    // R x nbSteps
  std::vector<double> rpI;                 // R x nbSteps x 3
  std::vector<double> rpSrc;               // R x (bySource ? nbSteps x nbSrc : nbSrc)
  std::vector<float> surf;                 // nbRxFaces x nbBins
  std::vector<float> cut;                  // sum over planes of nu x nv x nbBins
  uint64_t states[6] = {0, 0, 0, 0, 0, 0}; // by State
  uint64_t childOverflow = 0;
  double overflowEnergy = 0;
};

// A GABE column (gabe.h): float 50, int 51, short string 52.
struct GCol {
  int type = 50;
  std::string label;
  std::vector<float> f;
  std::vector<int32_t> i;
  std::vector<std::string> s;
  int32_t digits = 12;
};
bool saveGabe(const std::string& path, bool readOnly, const std::vector<GCol>& cols);

void mkdirs(const std::string& path);   // st_mkdir: boost::filesystem::create_directories, errors ignored

// The report: per band as each band ends, and the compilation after the last band.
class Report {
 public:
  Report(Config& cfg, Model& m);
  // runFrequenceCalculation's tail (sppsNantes.cpp:179-231, patched): folders, the band's surface
  // and cut files, its columns. Takes the band's sums (scaled in place for surf_receiv_method 1).
  void band(BandSums& b);
  // MainProcess after the threads (sppsNantes.cpp:389-439): "End of calculation.", the receivers'
  // files, the stats, the Global maps, the lost-particle warning.
  void finish();
  uint64_t lostOrLoop = 0, total = 0;

 private:
  Config& cfg;
  Model& m;
  struct BandCols {
    int band;
    GCol stats, sumEnergy;
    std::vector<std::vector<double>> energy;          // per receiver, nbSteps (energy_sum)
    std::vector<GCol> lf, lfc, ix, iy, iz, bySrc;     // per receiver
    std::vector<std::vector<double>> srcContrib;      // per receiver when output_recp_bysource
  };
  std::vector<BandCols> cols;
  std::vector<float> surfGlobal;   // sum over bands of value / face area (ExportTouteBande)
  std::vector<float> cutGlobal;    // sum over bands of the cell values (SauveRecepteursSurfaciquesCoupe, allfreq)
  bool anySurfBand = false, anyCutBand = false;
  void saveSurf(const std::string& path, const std::vector<float>& data, bool global);
  void saveCut(const std::string& path, const std::vector<float>& data, bool global);
};

// B3 (decision 71): the live stream of the saved particles, `spps-gpu.pstream` in the working
// directory, one per run. Not an SPPS output: a channel to the app, which tails it while the run is
// live and draws each saved particle's trajectory as it is written. The particles and their records
// are those of the `.pbin` files, written as each one is re-traced; the `.pbin` files are unchanged.
// Little-endian, every field 4 bytes:
//   header  u32 magic 0x4D545350 ("PSTM"), u32 version 1, f32 time step (s), u32 time-step count,
//           u32 particles saved per band (the .pbin header's count), u32 B bands computed,
//           i32 x B the computed bands, Hz, in computation order
//   frame   u32 length of the rest of the frame (16 + 16 n), i32 band (Hz), u32 the particle's index
//           in its band's .pbin, u32 its first time step (the .pbin's u16), u32 n records,
//           f32 x 3n positions, f32 x n energies (the .pbin's values, bit for bit)
// Each frame is written in one call and flushed, so a reader sees whole frames and at most one torn
// tail, which it leaves until its length is there.
class LiveStream {
 public:
  static constexpr uint32_t MAGIC = 0x4D545350u, VERSION = 1u;
  bool open(const std::string& path, const Config& cfg);
  void frame(int bandHz, uint32_t index, uint32_t firstStep, const std::vector<float>& xyze);
  // Closes the file; removes it unless `keep` (the stream is a channel, not a result).
  void close(bool keep);
  bool isOpen() const { return out.is_open(); }
  uint64_t frames = 0, bytes = 0;
  double seconds = 0;   // spent writing and flushing frames
  bool removed = false;

 private:
  std::ofstream out;
  std::string path;
  std::vector<char> buf;
};

// The particle file and its two CSVs for one band (reportmanager.cpp:90-146, 288-343, 439-479),
// fed by the host re-trace of the particles SPPS would mark.
class ParticleFiles {
 public:
  ParticleFiles(const Config& cfg, int band, LiveStream* live = nullptr);
  ~ParticleFiles();
  void newParticle();
  void step(const V3& pos, double E, int stepIndex);
  void surfHit(const V3& normal, int order, const V3& at, const V3& dir, double E);
  void rpHit(float time, int rp, const V3& dir, double energyTimesCdt);
  void save();
  void close();

 private:
  const Config& cfg;
  LiveStream* live;
  int bandHz;
  std::ofstream pbin, csvS, csvR;
  bool open = false, haveS = false, haveR = false;
  uint32_t real = 0;
  int firstStep = -1;
  std::vector<float> pos;   // x, y, z, E per recorded step
  struct SHit { V3 n; int order; V3 at, dir; double E; };
  struct RHit { float time; int rp; V3 dir; double E; };
  std::vector<SHit> sh;
  std::vector<RHit> rh;
};

}  // namespace spg
