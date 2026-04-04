#include "app/theme.h"
#include "project/project.h"
#include "project/result_parser.h"
#include "viewport/viewport.h"
#include <imgui.h>
#include <implot.h>
#include <glm/glm.hpp>
#include <cstdio>
#include <cstring>
#include <cmath>
#include <fstream>
#include <filesystem>
#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <commdlg.h>
#endif

namespace fs = std::filesystem;

namespace isimpa {

void ConsoleLog(const std::string& msg, int level);

// ─── State ─────────────────────────────────────────────────────────────────

static SimulationResults s_results;
static SurfaceRecResult s_surfRecResult;
static ParticleData s_particleData;

// Viz mode
enum VizMode { VIZ_SPL_MAP = 0, VIZ_PARTICLES, VIZ_INTENSITY, VIZ_PARAMETERS };
static int s_vizMode = VIZ_PARAMETERS;

// SPL Map state
static int s_mapBandIdx = -1;  // -1 = Global, 0..N = per-band index
static std::vector<std::string> s_mapBandPaths;  // paths to per-band .csbin files
static std::string s_mapGlobalPath;               // path to Global .csbin

// Particle state
static int s_partBandIdx = 0;
static std::vector<std::string> s_partBandPaths;
static bool s_particlePlaying = false;
static float s_particleTimer = 0;
static float s_playbackSpeed = 1.0f;

// Intensity state
static std::vector<std::string> s_rpiPaths;

// Parameters state
static int s_selectedReceiver = 0;
static int s_selectedBand = 0;

// Display settings
static float s_colormapOpacity = 0.75f;

// Auto-load
static bool s_autoLoadPending = false;
static std::string s_lastKnownResultDir;
static bool s_scanned = false;

// ─── Helpers ───────────────────────────────────────────────────────────────

static std::string ResultsSaveFileDialog(const char* filter, const char* title, const char* defaultExt) {
#ifdef _WIN32
    char filename[MAX_PATH] = {};
    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrTitle = title;
    ofn.lpstrDefExt = defaultExt;
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
    if (GetSaveFileNameA(&ofn)) return std::string(filename);
#endif
    return {};
}

// Scan result directory and populate band paths
static void ScanResults(const std::string& dir) {
    s_mapBandPaths.clear();
    s_mapGlobalPath.clear();
    s_partBandPaths.clear();
    s_rpiPaths.clear();
    s_scanned = true;

    if (!fs::exists(dir)) return;

    // Find surface receiver .csbin files (per-band and global)
    std::string srDir = dir + "/Surface_receiver";
    if (fs::exists(srDir)) {
        for (auto& entry : fs::directory_iterator(srDir)) {
            if (!entry.is_directory()) continue;
            std::string bandName = entry.path().filename().string();
            // Look for rs_cut.csbin or Sound_level.csbin
            for (auto& f : fs::directory_iterator(entry.path())) {
                if (f.path().extension() == ".csbin") {
                    if (bandName == "Global")
                        s_mapGlobalPath = f.path().string();
                    else
                        s_mapBandPaths.push_back(f.path().string());
                }
            }
        }
        std::sort(s_mapBandPaths.begin(), s_mapBandPaths.end());
    }

    // Find particle .pbin files
    std::string partDir = dir + "/particles";
    if (fs::exists(partDir)) {
        for (auto& entry : fs::directory_iterator(partDir)) {
            if (!entry.is_directory()) continue;
            for (auto& f : fs::directory_iterator(entry.path())) {
                if (f.path().extension() == ".pbin")
                    s_partBandPaths.push_back(f.path().string());
            }
        }
        std::sort(s_partBandPaths.begin(), s_partBandPaths.end());
    }

    // Find .rpi files
    std::string intDir = dir + "/Intensity animation";
    if (!fs::exists(intDir)) intDir = dir + "/IntensityAnimation";
    if (fs::exists(intDir)) {
        for (auto& entry : fs::directory_iterator(intDir)) {
            if (!entry.is_directory()) continue;
            for (auto& f : fs::directory_iterator(entry.path())) {
                if (f.path().extension() == ".rpi")
                    s_rpiPaths.push_back(f.path().string());
            }
        }
    }

    // Load receiver results
    LoadResults(dir, s_results);

    // Also search sibling dirs
    if (!s_results.loaded) {
        fs::path parentDir = fs::path(dir).parent_path();
        if (fs::exists(parentDir)) {
            for (auto& entry : fs::directory_iterator(parentDir)) {
                if (entry.is_directory() && entry.path().string() != dir) {
                    if (LoadResults(entry.path().string(), s_results)) break;
                }
            }
        }
    }

    ConsoleLog("[Results] Scanned: " + std::to_string(s_mapBandPaths.size()) + " SPL maps, " +
               std::to_string(s_partBandPaths.size()) + " particle sets, " +
               std::to_string(s_rpiPaths.size()) + " intensity files, " +
               std::to_string(s_results.receivers.size()) + " receivers", 0);
}

static void LoadColormapFile(const std::string& path) {
    s_surfRecResult = {};
    if (LoadCSBIN(path, s_surfRecResult) && s_surfRecResult.loaded) {
        ViewportLoadColormap(s_surfRecResult);
        // Convert min/max to dB for legend display
        const float P0 = 2.5e9f;
        float minDb = 200, maxDb = -200;
        for (auto& face : s_surfRecResult.faces) {
            if (face.energySum > 0) {
                float db = 10.0f * log10f(face.energySum * P0);
                if (db < minDb) minDb = db;
                if (db > maxDb) maxDb = db;
            }
        }
        if (maxDb > minDb) {
            s_surfRecResult.minEnergy = minDb;
            s_surfRecResult.maxEnergy = maxDb;
        }
        ConsoleLog("[SPL Map] Loaded: " + path + " (" + std::to_string((int)minDb) + " — " + std::to_string((int)maxDb) + " dB)", 0);
    }
}

// ─── Main Panel ────────────────────────────────────────────────────────────

void DrawResults() {
    if (!ImGui::Begin("Results")) { ImGui::End(); return; }

    Project& proj = GetProject();

    // Auto-detect when result dir changes
    if (!proj.lastResultDir.empty() && proj.lastResultDir != s_lastKnownResultDir) {
        s_lastKnownResultDir = proj.lastResultDir;
        s_scanned = false;
        s_autoLoadPending = true;
    }

    // Auto-scan on first access or when triggered
    if (s_autoLoadPending && !proj.lastResultDir.empty()) {
        ScanResults(proj.lastResultDir);
        s_autoLoadPending = false;

        // Auto-load global colormap if available
        if (!s_mapGlobalPath.empty()) {
            LoadColormapFile(s_mapGlobalPath);
            s_vizMode = VIZ_SPL_MAP;
        } else if (!s_mapBandPaths.empty()) {
            LoadColormapFile(s_mapBandPaths[0]);
            s_vizMode = VIZ_SPL_MAP;
        } else if (s_results.loaded && !s_results.receivers.empty()) {
            s_vizMode = VIZ_PARAMETERS;
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // SECTION 1: "What to show" — mode selector
    // ════════════════════════════════════════════════════════════════════════

    bool hasMapData = !s_mapBandPaths.empty() || !s_mapGlobalPath.empty();
    bool hasParticles = !s_partBandPaths.empty() || ViewportHasParticles();
    bool hasIntensity = !s_rpiPaths.empty();
    bool hasParams = s_results.loaded && !s_results.receivers.empty();

    ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(NeonColors::BgMid[0], NeonColors::BgMid[1], NeonColors::BgMid[2], 1.0f));
    auto ModeButton = [](const char* label, int mode, int& current, bool available) {
        if (!available) {
            ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 0.4f));
            ImGui::Button(label);
            ImGui::PopStyleColor();
        } else {
            bool active = (current == mode);
            if (active) ImGui::PushStyleColor(ImGuiCol_Button,
                ImVec4(NeonColors::Cyan[0]*0.3f, NeonColors::Cyan[1]*0.3f, NeonColors::Cyan[2]*0.3f, 1.0f));
            if (ImGui::Button(label)) current = mode;
            if (active) ImGui::PopStyleColor();
        }
        ImGui::SameLine();
    };
    ModeButton("SPL Map", VIZ_SPL_MAP, s_vizMode, hasMapData);
    ModeButton("Particles", VIZ_PARTICLES, s_vizMode, hasParticles);
    ModeButton("Intensity", VIZ_INTENSITY, s_vizMode, hasIntensity);
    ModeButton("Parameters", VIZ_PARAMETERS, s_vizMode, hasParams);
    ImGui::NewLine();
    ImGui::PopStyleColor();

    ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

    // ════════════════════════════════════════════════════════════════════════
    // SECTION 2: Context-sensitive controls
    // ════════════════════════════════════════════════════════════════════════

    if (s_vizMode == VIZ_SPL_MAP) {
        // ── SPL Map controls ────────────────────────────────────────────
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::Text("SPL Surface Map");
        ImGui::PopStyleColor();

        // Band selector
        ImGui::Text("Frequency:");
        ImGui::SameLine();
        if (ImGui::SmallButton(s_mapGlobalPath.empty() ? "##noGlobal" : "Global")) {
            if (!s_mapGlobalPath.empty()) {
                LoadColormapFile(s_mapGlobalPath);
                s_mapBandIdx = -1;
            }
        }
        for (int i = 0; i < (int)s_mapBandPaths.size(); i++) {
            ImGui::SameLine();
            std::string bandLabel = fs::path(s_mapBandPaths[i]).parent_path().filename().string();
            bool active = (s_mapBandIdx == i);
            if (active) ImGui::PushStyleColor(ImGuiCol_Button,
                ImVec4(NeonColors::Cyan[0]*0.3f, NeonColors::Cyan[1]*0.3f, NeonColors::Cyan[2]*0.3f, 1.0f));
            if (ImGui::SmallButton(bandLabel.c_str())) {
                LoadColormapFile(s_mapBandPaths[i]);
                s_mapBandIdx = i;
            }
            if (active) ImGui::PopStyleColor();
        }

        // Display info
        if (s_surfRecResult.loaded) {
            ImGui::Text("%s: %zu faces, range %.2e — %.2e",
                s_surfRecResult.name.c_str(), s_surfRecResult.faces.size(),
                s_surfRecResult.minEnergy, s_surfRecResult.maxEnergy);
        }

        // Clear button
        if (ViewportHasColormap()) {
            if (ImGui::Button("Clear Map")) ViewportClearColormap();
        }
    }

    else if (s_vizMode == VIZ_PARTICLES) {
        // ── Particle controls ───────────────────────────────────────────
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::Text("Particle Animation");
        ImGui::PopStyleColor();

        // Band selector for particles
        if (!s_partBandPaths.empty()) {
            ImGui::Text("Frequency:");
            for (int i = 0; i < (int)s_partBandPaths.size(); i++) {
                if (i > 0) ImGui::SameLine();
                std::string bandLabel = fs::path(s_partBandPaths[i]).parent_path().filename().string();
                bool active = (s_partBandIdx == i && ViewportHasParticles());
                if (active) ImGui::PushStyleColor(ImGuiCol_Button,
                    ImVec4(NeonColors::Cyan[0]*0.3f, NeonColors::Cyan[1]*0.3f, NeonColors::Cyan[2]*0.3f, 1.0f));
                if (ImGui::SmallButton(bandLabel.c_str())) {
                    s_partBandIdx = i;
                    LoadPBIN(s_partBandPaths[i], s_particleData);
                    if (s_particleData.loaded) {
                        ViewportLoadParticles(s_particleData);
                        ConsoleLog("[Particles] Loaded: " + s_partBandPaths[i], 0);
                    }
                }
                if (active) ImGui::PopStyleColor();
            }
        }

        // Playback controls
        if (ViewportHasParticles()) {
            int maxSteps = ViewportGetParticleMaxSteps();
            static int currentStep = 0;

            // Play/Pause/Stop/Prev/Next
            if (s_particlePlaying) {
                if (ImGui::Button("Pause")) s_particlePlaying = false;
            } else {
                if (ImGui::Button("Play")) s_particlePlaying = true;
            }
            ImGui::SameLine();
            if (ImGui::Button("Stop")) { s_particlePlaying = false; currentStep = 0; ViewportSetParticleTimeStep(0); }
            ImGui::SameLine();
            if (ImGui::SmallButton("|<")) { if (currentStep > 0) { currentStep--; ViewportSetParticleTimeStep(currentStep); } }
            ImGui::SameLine();
            if (ImGui::SmallButton(">|")) { if (currentStep < maxSteps - 1) { currentStep++; ViewportSetParticleTimeStep(currentStep); } }
            ImGui::SameLine();
            if (ImGui::Button("Clear")) { ViewportClearParticles(); s_particlePlaying = false; currentStep = 0; }

            // Timeline
            ImGui::PushItemWidth(-1);
            if (ImGui::SliderInt("##TimeStep", &currentStep, 0, maxSteps - 1))
                ViewportSetParticleTimeStep(currentStep);
            ImGui::PopItemWidth();

            ImGui::Text("Step %d / %d  (t = %.4f s)", currentStep, maxSteps, currentStep * s_particleData.timeStep);

            // Speed
            ImGui::SliderFloat("Speed", &s_playbackSpeed, 0.1f, 4.0f, "%.1fx");

            // Auto-advance
            if (s_particlePlaying) {
                s_particleTimer += ImGui::GetIO().DeltaTime;
                float stepInterval = s_particleData.timeStep * (1.0f / s_playbackSpeed);
                if (stepInterval < 0.05f) stepInterval = 0.05f;
                while (s_particleTimer >= stepInterval) {
                    s_particleTimer -= stepInterval;
                    currentStep++;
                    if (currentStep >= maxSteps) currentStep = 0;
                    ViewportSetParticleTimeStep(currentStep);
                }
            }
        } else {
            ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
                "Click a frequency band above to load particles.");
        }
    }

    else if (s_vizMode == VIZ_INTENSITY) {
        // ── Intensity controls ──────────────────────────────────────────
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::Text("Intensity Vectors");
        ImGui::PopStyleColor();

        ImGui::Text("%zu frequency bands available", s_rpiPaths.size());
        for (auto& p : s_rpiPaths) {
            std::string bandName = fs::path(p).parent_path().filename().string();
            ImGui::BulletText("%s", bandName.c_str());
        }
        ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
            "Intensity arrows shown at receiver positions in the viewport.");
    }

    else if (s_vizMode == VIZ_PARAMETERS) {
        // ── Acoustic Parameters ─────────────────────────────────────────
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::Text("Acoustic Parameters");
        ImGui::PopStyleColor();

        if (!s_results.loaded || s_results.receivers.empty()) {
            ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
                "Run a simulation to see results here.");
        } else {
            // Receiver selector
            ImGui::Text("Receiver:");
            for (int i = 0; i < (int)s_results.receivers.size(); i++) {
                ImGui::SameLine();
                bool sel = (s_selectedReceiver == i);
                if (sel) ImGui::PushStyleColor(ImGuiCol_Button,
                    ImVec4(NeonColors::Cyan[0]*0.3f, NeonColors::Cyan[1]*0.3f, NeonColors::Cyan[2]*0.3f, 1.0f));
                if (ImGui::SmallButton(s_results.receivers[i].name.c_str()))
                    s_selectedReceiver = i;
                if (sel) ImGui::PopStyleColor();
            }

            ReceiverResult* rcv = nullptr;
            if (s_selectedReceiver < (int)s_results.receivers.size())
                rcv = &s_results.receivers[s_selectedReceiver];

            ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

            // Parameters table
            if (rcv) {
                ImGui::Columns(3, "##AcParams", true);
                ImGui::SetColumnWidth(0, 100); ImGui::SetColumnWidth(1, 80);
                auto Param = [](const char* name, float val, const char* unit, bool ok) {
                    ImGui::TextColored(ImVec4(NeonColors::TextNormal[0], NeonColors::TextNormal[1], NeonColors::TextNormal[2], 1.0f), "%s", name);
                    ImGui::NextColumn();
                    if (ok && val != 0)
                        ImGui::TextColored(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f), "%.2f", val);
                    else
                        ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f), "--");
                    ImGui::NextColumn();
                    ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f), "%s", unit);
                    ImGui::NextColumn();
                };
                Param("RT60",  rcv->rt60, "s",  true);
                Param("EDT",   rcv->edt,  "s",  true);
                Param("C80",   rcv->c80,  "dB", true);
                Param("D50",   rcv->d50,  "%%", true);
                Param("Ts",    rcv->ts,   "ms", true);
                Param("G",     rcv->g,    "dB", true);
                Param("SPL",   rcv->totalSplDb, "dB", true);
                ImGui::Columns(1);

                // Global TCR results
                if (s_results.sabineRT > 0) {
                    ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();
                    ImGui::Text("TCR Global: Sabine RT=%.2fs, Eyring RT=%.2fs",
                        s_results.sabineRT, s_results.eyringRT);
                }
            }

            ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

            // Frequency Response plot
            if (ImGui::CollapsingHeader("Frequency Response", ImGuiTreeNodeFlags_DefaultOpen)) {
                if (ImPlot::BeginPlot("##FreqResponse", ImVec2(-1, 200))) {
                    ImPlot::SetupAxes("Frequency (Hz)", "SPL (dB)");
                    ImPlot::SetupAxisScale(ImAxis_X1, ImPlotScale_Log10);

                    if (rcv && !rcv->frequencies.empty() && !rcv->splDb.empty()) {
                        int n = (std::min)((int)rcv->frequencies.size(), (int)rcv->splDb.size());
                        std::vector<double> freqs(n), spls(n);
                        for (int i = 0; i < n; i++) { freqs[i] = rcv->frequencies[i]; spls[i] = rcv->splDb[i]; }
                        ImPlot::PlotLine("SPL (dB)", freqs.data(), spls.data(), n);
                    }
                    ImPlot::EndPlot();
                }
            }

            // Echogram
            if (ImGui::CollapsingHeader("Echogram")) {
                if (ImPlot::BeginPlot("##Echogram", ImVec2(-1, 150))) {
                    ImPlot::SetupAxes("Time (s)", "Energy (dB)");
                    if (rcv && !rcv->echoTime.empty()) {
                        int n = (std::min)((int)rcv->echoTime.size(), (int)rcv->echoEnergy.size());
                        std::vector<double> t(n), e(n);
                        for (int i = 0; i < n; i++) { t[i] = rcv->echoTime[i]; e[i] = rcv->echoEnergy[i]; }
                        ImPlot::PlotLine("Energy", t.data(), e.data(), n);
                    }
                    ImPlot::EndPlot();
                }
            }

            // Schroeder curve
            if (ImGui::CollapsingHeader("Schroeder Curve")) {
                if (ImPlot::BeginPlot("##Schroeder", ImVec2(-1, 150))) {
                    ImPlot::SetupAxes("Time (s)", "Level (dB)");
                    if (rcv && !rcv->schroederTime.empty()) {
                        int n = (std::min)((int)rcv->schroederTime.size(), (int)rcv->schroederDb.size());
                        std::vector<double> t(n), db(n);
                        for (int i = 0; i < n; i++) { t[i] = rcv->schroederTime[i]; db[i] = rcv->schroederDb[i]; }
                        ImPlot::PlotLine("Schroeder", t.data(), db.data(), n);
                    }
                    ImPlot::EndPlot();
                }
            }

            // CSV Export
            ImGui::Spacing();
            if (ImGui::Button("Export to CSV")) {
                std::string path = ResultsSaveFileDialog(
                    "CSV Files (*.csv)\0*.csv\0", "Export Results", "csv");
                if (!path.empty()) {
                    std::ofstream csv(path);
                    csv << "Receiver,Frequency (Hz),SPL (dB),RT60 (s),EDT (s),C80 (dB),D50 (%%),Ts (ms)\n";
                    for (auto& r : s_results.receivers) {
                        for (int i = 0; i < (int)r.frequencies.size() && i < (int)r.splDb.size(); i++) {
                            csv << r.name << "," << r.frequencies[i] << "," << r.splDb[i]
                                << "," << r.rt60 << "," << r.edt << "," << r.c80
                                << "," << r.d50 << "," << r.ts << "\n";
                        }
                    }
                    csv.close();
                    ConsoleLog("[Export] CSV saved: " + path, 0);
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // SECTION 3: Display settings (always visible when colormap active)
    // ════════════════════════════════════════════════════════════════════════

    if (ViewportHasColormap()) {
        ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();
        ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
            "Display Settings");
        ImGui::SliderFloat("Opacity", &s_colormapOpacity, 0.1f, 1.0f, "%.0f%%");
        // TODO: pass opacity to viewport shader
    }

    // ════════════════════════════════════════════════════════════════════════
    // Status line
    // ════════════════════════════════════════════════════════════════════════

    ImGui::Spacing(); ImGui::Separator();
    if (!proj.lastResultDir.empty()) {
        ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 0.6f),
            "%s", proj.lastResultDir.c_str());
    }

    ImGui::End();
}

// ─── Colormap data accessors (used by viewport legend) ──────────────────────

float GetColormapMin() { return s_surfRecResult.minEnergy; }
float GetColormapMax() { return s_surfRecResult.maxEnergy; }
const char* GetColormapName() { return s_surfRecResult.name.c_str(); }

// ─── Simulation results accessor (used by viewport intensity arrows) ────────

const SimulationResults& GetSimResults() { return s_results; }

} // namespace isimpa
