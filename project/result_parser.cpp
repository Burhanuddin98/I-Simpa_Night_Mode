#include "project/result_parser.h"
#include <fstream>
#include <filesystem>
#include <cstdio>
#include <cstring>
#include <cmath>
#include <map>

namespace fs = std::filesystem;

namespace isimpa {

// ─── GABE Binary Format Reader ──────────────────────────────────────────────

bool GabeFile::Load(const std::string& path) {
    std::ifstream f(path, std::ios::binary);
    if (!f.is_open()) {
        fprintf(stderr, "[GABE] Cannot open: %s\n", path.c_str());
        return false;
    }

    // File header
    int32_t formatVersion, headerLen, colHeaderLen, numCols;
    f.read((char*)&formatVersion, 4);
    f.read((char*)&headerLen, 4);
    f.read((char*)&colHeaderLen, 4);
    f.read((char*)&numCols, 4);

    uint8_t ro;
    f.read((char*)&ro, 1);
    readOnly = (ro != 0);

    // Skip padding to align to header length
    int bytesRead = 17;
    if (headerLen > bytesRead) {
        f.seekg(headerLen - bytesRead, std::ios::cur);
    }
    // Also skip 3 bytes padding after header (observed in I-Simpa code)
    f.seekg(3, std::ios::cur);

    columns.resize(numCols);

    for (int c = 0; c < numCols; c++) {
        auto& col = columns[c];

        uint16_t colType;
        f.read((char*)&colType, 2);
        f.seekg(2, std::ios::cur); // padding
        col.type = (GabeColType)colType;

        f.read((char*)&col.rows, 4);

        int64_t sizeofCol, sizeofHeader;
        f.read((char*)&sizeofCol, 8);
        f.read((char*)&sizeofHeader, 8);

        char label[256] = {};
        f.read(label, 255);
        f.seekg(1, std::ios::cur); // padding
        col.label = label;

        // Read column data (matching original gabe.cpp ReadFile methods)
        switch (col.type) {
            case GABE_FLOAT: {
                // GABE_Data_Float::ReadFile: read numOfDigits(4), then rows*float(4)
                int32_t numDigits;
                f.read((char*)&numDigits, 4);
                col.floatData.resize(col.rows);
                f.read((char*)col.floatData.data(), col.rows * 4);
                break;
            }
            case GABE_INT: {
                // GABE_Data_Integer::ReadFile: skip 1 byte, then rows*int(4)
                f.seekg(1, std::ios::cur);
                col.intData.resize(col.rows);
                f.read((char*)col.intData.data(), col.rows * 4);
                break;
            }
            case GABE_STRING: {
                // GABE_Data_ShortString::ReadFile: skip 1 byte, then rows*50 bytes
                f.seekg(1, std::ios::cur);
                col.strData.resize(col.rows);
                for (int r = 0; r < col.rows; r++) {
                    char str[51] = {};
                    f.read(str, 50);
                    str[50] = '\0';
                    col.strData[r] = str;
                }
                break;
            }
            default: {
                // Unknown column type — skip using sizeofCol already read above
                printf("[GABE] Warning: unknown column type %d, skipping %lld bytes\n",
                       (int)col.type, (long long)sizeofCol);
                f.seekg(sizeofCol, std::ios::cur);
                break;
            }
        }
    }

    printf("[GABE] Loaded: %s (%d cols, %d rows)\n", path.c_str(), numCols,
           columns.empty() ? 0 : columns[0].rows);
    return true;
}

const GabeColumn* GabeFile::FindColumn(const std::string& label) const {
    for (auto& col : columns) {
        if (col.label == label) return &col;
    }
    return nullptr;
}

// ─── Result Directory Scanner ───────────────────────────────────────────────

std::vector<std::string> FindResultFiles(const std::string& dir, const std::string& extension) {
    std::vector<std::string> results;
    if (!fs::exists(dir)) return results;

    for (auto& entry : fs::recursive_directory_iterator(dir)) {
        if (entry.is_regular_file() && entry.path().extension() == extension) {
            results.push_back(entry.path().string());
        }
    }
    return results;
}

// ─── Load Simulation Results ────────────────────────────────────────────────

bool LoadResults(const std::string& resultDir, SimulationResults& results) {
    results = {};
    results.loaded = false;

    if (!fs::exists(resultDir)) {
        fprintf(stderr, "[Results] Directory not found: %s\n", resultDir.c_str());
        return false;
    }

    // Find .gabe files (main results)
    auto gabeFiles = FindResultFiles(resultDir, ".gabe");
    auto recpFiles = FindResultFiles(resultDir, ".recp");

    printf("[Results] Found %zu .gabe files, %zu .recp files in %s\n",
           gabeFiles.size(), recpFiles.size(), resultDir.c_str());

    // Helper to parse a receiver GABE file (works for both .recp and .gabe)
    auto parseReceiverGabe = [](const std::string& path, ReceiverResult& rcv) {
        GabeFile gabe;
        if (!gabe.Load(path)) return false;

        // Derive receiver name from parent directory (SPPS: "R1 (near)/Sound_level.recp")
        // or from filename stem (TCR: "Punctual receivers/R01.gabe")
        std::string parentDir = fs::path(path).parent_path().filename().string();
        if (parentDir.find("Punctual") != std::string::npos ||
            parentDir.find("receivers") != std::string::npos) {
            // TCR style — receiver name is the file stem
            rcv.name = fs::path(path).stem().string();
        } else {
            // SPPS style — receiver name is the parent directory
            rcv.name = parentDir;
        }

        if (gabe.columns.empty()) return false;

        // Detect SPPS Sound_level.recp format:
        //   Col 0: STRING labeled "SPL" with time step labels ("5.0 ms", "10.0 ms", ...)
        //   Cols 1-N: FLOAT labeled "125 Hz", "250 Hz", ... with energy density per time step
        bool isSppsTimeSeries = (gabe.columns[0].type == GABE_STRING &&
                                 gabe.columns[0].label == "SPL" &&
                                 gabe.columns.size() >= 2 &&
                                 gabe.columns[1].type == GABE_FLOAT);

        if (isSppsTimeSeries) {
            // SPPS .recp: each float column is a frequency band with energy per time step
            // Reference: p_0 = 1/(20e-6)^2 = 2.5e9 (matches original I-Simpa)
            const float p_0 = 1.0f / powf(20.0f * 1e-6f, 2.0f); // 2.5e9

            int numTimeSteps = gabe.columns[0].rows;
            int numBands = (int)gabe.columns.size() - 1;

            rcv.frequencies.clear();
            rcv.splDb.clear();

            // Build broadband echogram (sum across bands per time step)
            std::vector<float> broadbandEnergy(numTimeSteps, 0);

            for (int c = 1; c < (int)gabe.columns.size(); c++) {
                auto& col = gabe.columns[c];
                if (col.type != GABE_FLOAT || col.floatData.empty()) continue;

                // Extract frequency from column label (e.g., "125 Hz" -> 125.0)
                float freq = 0;
                sscanf(col.label.c_str(), "%f", &freq);
                if (freq > 0) rcv.frequencies.push_back(freq);

                // Sum energy across all time steps for this band (total per-band energy)
                float energySum = 0;
                int n = std::min((int)col.floatData.size(), numTimeSteps);
                for (int t = 0; t < n; t++) {
                    energySum += col.floatData[t];
                    broadbandEnergy[t] += col.floatData[t];
                }

                // Convert to dB SPL using I-Simpa reference: 10*log10(energy * p_0)
                float splDb = (energySum > 0) ? 10.0f * log10f(energySum * p_0) : -120.0f;
                rcv.splDb.push_back(splDb);
            }

            // Build echogram (broadband energy per time step, in dB)
            // Extract time step from first string label (e.g., "5.0 ms")
            float dt = 0.005f;
            if (!gabe.columns[0].strData.empty()) {
                float firstTime = 0;
                sscanf(gabe.columns[0].strData[0].c_str(), "%f", &firstTime);
                if (firstTime > 0) dt = firstTime / 1000.0f; // ms -> s
            }

            rcv.echoTime.resize(numTimeSteps);
            rcv.echoEnergy.resize(numTimeSteps);
            for (int t = 0; t < numTimeSteps; t++) {
                rcv.echoTime[t] = (t + 1) * dt;
                rcv.echoEnergy[t] = (broadbandEnergy[t] > 0)
                    ? 10.0f * log10f(broadbandEnergy[t] * p_0) : -120.0f;
            }

            // Build Schroeder curve (backward integration of broadband energy)
            rcv.schroederTime.resize(numTimeSteps);
            rcv.schroederDb.resize(numTimeSteps);
            float cumul = 0;
            for (int t = numTimeSteps - 1; t >= 0; t--) {
                cumul += broadbandEnergy[t];
                rcv.schroederTime[t] = (t + 1) * dt;
                rcv.schroederDb[t] = (cumul > 0) ? 10.0f * log10f(cumul * p_0) : -120.0f;
            }

            // Compute acoustic parameters from Schroeder curve
            // Normalize Schroeder to 0 dB at start
            float schroederMax = rcv.schroederDb[0];
            std::vector<float> schNorm(numTimeSteps);
            for (int t = 0; t < numTimeSteps; t++)
                schNorm[t] = rcv.schroederDb[t] - schroederMax;

            // EDT: linear regression on 0 to -10 dB range
            {
                std::vector<float> xv, yv;
                for (int t = 0; t < numTimeSteps; t++) {
                    if (schNorm[t] >= -10.0f && schNorm[t] <= 0.0f) {
                        xv.push_back(rcv.schroederTime[t] * 1000.0f); // ms
                        yv.push_back(schNorm[t]);
                    }
                    if (schNorm[t] < -10.0f) break;
                }
                if (xv.size() >= 2) {
                    float n = (float)xv.size(), sx = 0, sy = 0, sxy = 0, sxx = 0;
                    for (int i = 0; i < (int)xv.size(); i++) {
                        sx += xv[i]; sy += yv[i]; sxy += xv[i]*yv[i]; sxx += xv[i]*xv[i];
                    }
                    float slope = (n*sxy - sx*sy) / (n*sxx - sx*sx);
                    if (slope < -0.001f) rcv.edt = (-60.0f / slope) / 1000.0f;
                }
            }
            // RT30: linear regression on -5 to -35 dB range
            {
                std::vector<float> xv, yv;
                for (int t = 0; t < numTimeSteps; t++) {
                    if (schNorm[t] <= -5.0f && schNorm[t] >= -35.0f) {
                        xv.push_back(rcv.schroederTime[t] * 1000.0f);
                        yv.push_back(schNorm[t]);
                    }
                    if (schNorm[t] < -35.0f) break;
                }
                if (xv.size() >= 2) {
                    float n = (float)xv.size(), sx = 0, sy = 0, sxy = 0, sxx = 0;
                    for (int i = 0; i < (int)xv.size(); i++) {
                        sx += xv[i]; sy += yv[i]; sxy += xv[i]*yv[i]; sxx += xv[i]*xv[i];
                    }
                    float slope = (n*sxy - sx*sy) / (n*sxx - sx*sx);
                    if (slope < -0.001f) rcv.rt60 = (-60.0f / slope) / 1000.0f;
                }
            }
            // C80: early (0-80ms) vs late (80ms+) energy ratio
            {
                int t80 = (int)(0.08f / dt);
                if (t80 > 0 && t80 < numTimeSteps) {
                    float early = 0, late = 0;
                    for (int t = 0; t < t80; t++) early += broadbandEnergy[t];
                    for (int t = t80; t < numTimeSteps; t++) late += broadbandEnergy[t];
                    if (late > 0) rcv.c80 = 10.0f * log10f(early / late);
                }
            }
            // D50: early (0-50ms) / total energy as percentage
            {
                int t50 = (int)(0.05f / dt);
                float totalE = 0;
                for (float e : broadbandEnergy) totalE += e;
                if (t50 > 0 && t50 < numTimeSteps && totalE > 0) {
                    float early = 0;
                    for (int t = 0; t < t50; t++) early += broadbandEnergy[t];
                    rcv.d50 = 100.0f * early / totalE;
                }
            }
            // Ts: centre time
            {
                float totalE = 0, weightedSum = 0;
                for (int t = 0; t < numTimeSteps; t++) {
                    totalE += broadbandEnergy[t];
                    weightedSum += ((t + 1) * dt) * broadbandEnergy[t];
                }
                if (totalE > 0) rcv.ts = 1000.0f * weightedSum / totalE; // ms
            }

            printf("[RECP] SPPS: %s, %zu bands, splDb[0]=%.1f dB, RT60=%.2fs\n",
                   rcv.name.c_str(), rcv.splDb.size(),
                   rcv.splDb.empty() ? 0.0f : rcv.splDb[0], rcv.rt60);
        } else {
            // TCR .gabe format: columns labeled "Direct\rdB SPL", "Total (Sabine)\rdB SPL", etc.
            // Or frequency-indexed data
            for (auto& col : gabe.columns) {
                if (col.type == GABE_STRING && !col.strData.empty()) {
                    // Row labels — extract frequency values if they look like "125 Hz"
                    rcv.frequencies.clear();
                    for (auto& s : col.strData) {
                        float freq = 0;
                        if (sscanf(s.c_str(), "%f", &freq) == 1 && freq > 0) {
                            rcv.frequencies.push_back(freq);
                        }
                    }
                }
                if (col.type == GABE_FLOAT && !col.floatData.empty()) {
                    if (col.label.find("dB(A)") != std::string::npos ||
                        col.label.find("dbA") != std::string::npos) {
                        rcv.splDbA = col.floatData;
                    } else if (col.label.find("Sabine") != std::string::npos ||
                               col.label.find("Total") != std::string::npos) {
                        rcv.splDb = col.floatData;
                    } else if (col.label.find("Direct") != std::string::npos) {
                        if (rcv.splDb.empty()) rcv.splDb = col.floatData;
                    } else if (col.label.find("Eyring") != std::string::npos) {
                        rcv.splDbA = col.floatData;
                    } else if (col.label.find("dB") != std::string::npos ||
                               col.label.find("SPL") != std::string::npos) {
                        if (rcv.splDb.empty()) rcv.splDb = col.floatData;
                    }
                }
            }
        }

        // Compute total SPL if we have per-band data
        if (!rcv.splDb.empty()) {
            float total = 0;
            for (float v : rcv.splDb) {
                if (v > 0) total += powf(10.0f, v / 10.0f);
            }
            rcv.totalSplDb = (total > 0) ? 10.0f * log10f(total) : 0;
        }

        return !rcv.splDb.empty() || !rcv.frequencies.empty();
    };

    // Load .recp files (SPPS receiver format)
    // SPPS creates per-band subdirectories: "R1 (near)/", "R1 (near)0/", "R1 (near)1/"
    // Only load the base directory (no trailing digit) to avoid duplicates
    std::map<std::string, std::string> recpByReceiver; // receiver name -> first .recp path
    for (auto& recpPath : recpFiles) {
        std::string parentDir = fs::path(recpPath).parent_path().filename().string();
        // Skip per-band duplicates: "R1 (near)0", "R1 (near)1" etc.
        // These have a digit suffix appended to the base receiver name
        bool isDuplicate = false;
        if (!parentDir.empty() && parentDir.back() >= '0' && parentDir.back() <= '9') {
            // Check if removing trailing digits gives a name we already have
            std::string baseName = parentDir;
            while (!baseName.empty() && baseName.back() >= '0' && baseName.back() <= '9')
                baseName.pop_back();
            if (!baseName.empty() && recpByReceiver.count(baseName))
                isDuplicate = true;
        }
        if (!isDuplicate) {
            recpByReceiver[parentDir] = recpPath;
        }
    }
    for (auto& [name, recpPath] : recpByReceiver) {
        ReceiverResult rcv;
        if (parseReceiverGabe(recpPath, rcv))
            results.receivers.push_back(rcv);
    }

    // Load per-receiver .gabe files (TCR format: Punctual receivers/R01.gabe)
    for (auto& gabePath : gabeFiles) {
        std::string fname = fs::path(gabePath).stem().string();
        std::string parent = fs::path(gabePath).parent_path().filename().string();

        // Skip "Main results.gabe" — handled below
        if (fname.find("Main") != std::string::npos) continue;
        // Skip intensity/collision files
        if (fname.find("intensity") != std::string::npos ||
            fname.find("Intensity") != std::string::npos) continue;

        // If it's in a "Punctual receivers" directory, it's a receiver result
        if (parent.find("receivers") != std::string::npos ||
            parent.find("Receivers") != std::string::npos ||
            parent.find("Punctual") != std::string::npos) {
            ReceiverResult rcv;
            if (parseReceiverGabe(gabePath, rcv))
                results.receivers.push_back(rcv);
        }
    }

    // Load main/global .gabe files
    for (auto& gabePath : gabeFiles) {
        GabeFile gabe;
        if (!gabe.Load(gabePath)) continue;

        std::string fname = fs::path(gabePath).stem().string();

        // Main results (TCR global: absorption, RT, SPL per frequency)
        if (fname.find("Main") != std::string::npos || fname.find("main") != std::string::npos) {
            for (auto& col : gabe.columns) {
                if (col.type != GABE_FLOAT || col.floatData.empty()) continue;
                // Average across all frequency bands for summary values
                float avg = 0;
                for (float v : col.floatData) avg += v;
                avg /= (float)col.floatData.size();

                if (col.label.find("TR_Sabine") != std::string::npos) {
                    results.sabineRT = avg;
                }
                if (col.label.find("TR_Eyring") != std::string::npos) {
                    results.eyringRT = avg;
                }
            }
            results.solverName = "TCR";
        }
    }

    // Load .gap files (SPPS advanced receiver data — energy time-series for acoustic params)
    auto gapFiles = FindResultFiles(resultDir, ".gap");
    for (auto& gapPath : gapFiles) {
        std::string fname = fs::path(gapPath).stem().string();
        if (fname.find("Advanced") == std::string::npos && fname.find("advanced") == std::string::npos)
            continue;

        std::string rcvDir = fs::path(gapPath).parent_path().filename().string();

        // Find the matching receiver by name
        ReceiverResult* rcv = nullptr;
        for (auto& r : results.receivers) {
            if (r.name == rcvDir) { rcv = &r; break; }
        }
        if (!rcv) continue;

        GabeFile gabe;
        if (!gabe.Load(gapPath)) continue;
        if (gabe.columns.size() < 5) continue;

        // Parse metadata from col 0 (INT)
        int numFreqBands = 0, colsPerBand = 3, numTimeSteps = 0;
        if (gabe.columns[0].type == GABE_INT && gabe.columns[0].intData.size() >= 8) {
            numFreqBands = gabe.columns[0].intData[5];
            colsPerBand  = gabe.columns[0].intData[6];
            numTimeSteps = gabe.columns[0].intData[7];
        }
        // Parse time step from col 1 (FLOAT)
        float dt = 0.005f;
        if (gabe.columns[1].type == GABE_FLOAT && !gabe.columns[1].floatData.empty()) {
            dt = gabe.columns[1].floatData[0];
        }
        if (numFreqBands <= 0 || numTimeSteps <= 0 || dt <= 0) continue;

        // Build broadband echogram by summing energy across all frequency bands
        std::vector<float> echogram(numTimeSteps, 0);
        for (int b = 0; b < numFreqBands; b++) {
            int colIdx = 5 + b * colsPerBand;
            if (colIdx >= (int)gabe.columns.size()) break;
            auto& col = gabe.columns[colIdx];
            if (col.type != GABE_FLOAT) continue;
            int n = std::min((int)col.floatData.size(), numTimeSteps);
            for (int t = 0; t < n; t++) {
                echogram[t] += col.floatData[t];
            }
        }

        // Store echogram
        rcv->echoTime.resize(numTimeSteps);
        rcv->echoEnergy.resize(numTimeSteps);
        for (int t = 0; t < numTimeSteps; t++) {
            rcv->echoTime[t] = t * dt;
            rcv->echoEnergy[t] = (echogram[t] > 0) ? 10.0f * log10f(echogram[t] / 1e-12f) : -120.0f;
        }

        // Build Schroeder curve (backward integration)
        float totalEnergy = 0;
        for (int t = 0; t < numTimeSteps; t++) totalEnergy += echogram[t];
        if (totalEnergy <= 0) continue;

        rcv->schroederTime.resize(numTimeSteps);
        rcv->schroederDb.resize(numTimeSteps);
        float cumul = 0;
        for (int t = 0; t < numTimeSteps; t++) {
            rcv->schroederTime[t] = t * dt;
            float remaining = totalEnergy - cumul;
            rcv->schroederDb[t] = (remaining > 0) ? 10.0f * log10f(remaining / totalEnergy) : -60.0f;
            cumul += echogram[t];
        }

        // Compute acoustic parameters from Schroeder curve
        // RT60: time for Schroeder to drop from -5 dB to -35 dB (T30 * 2)
        float t5 = -1, t35 = -1;
        for (int t = 0; t < numTimeSteps; t++) {
            if (t5 < 0 && rcv->schroederDb[t] <= -5.0f)  t5 = t * dt;
            if (t35 < 0 && rcv->schroederDb[t] <= -35.0f) t35 = t * dt;
        }
        if (t5 >= 0 && t35 > t5) rcv->rt60 = 2.0f * (t35 - t5);

        // EDT: time for Schroeder to drop from 0 dB to -10 dB, extrapolated to -60
        float t0edt = -1, t10 = -1;
        for (int t = 0; t < numTimeSteps; t++) {
            if (t0edt < 0 && rcv->schroederDb[t] <= 0.0f) t0edt = t * dt;
            if (t10 < 0 && rcv->schroederDb[t] <= -10.0f) t10 = t * dt;
        }
        if (t0edt >= 0 && t10 > t0edt) rcv->edt = 6.0f * (t10 - t0edt);

        // C80: 10*log10(E_0_80ms / E_80ms_inf)
        int t80 = (int)(0.08f / dt);
        if (t80 > 0 && t80 < numTimeSteps) {
            float early = 0, late = 0;
            for (int t = 0; t < t80; t++) early += echogram[t];
            for (int t = t80; t < numTimeSteps; t++) late += echogram[t];
            if (late > 0) rcv->c80 = 10.0f * log10f(early / late);
        }

        // D50: E_0_50ms / E_total
        int t50 = (int)(0.05f / dt);
        if (t50 > 0 && t50 < numTimeSteps) {
            float early = 0;
            for (int t = 0; t < t50; t++) early += echogram[t];
            rcv->d50 = 100.0f * early / totalEnergy;
        }

        // Ts: centre time = sum(t * e(t)) / sum(e(t))
        float weightedSum = 0;
        for (int t = 0; t < numTimeSteps; t++) {
            weightedSum += (t * dt) * echogram[t];
        }
        rcv->ts = 1000.0f * weightedSum / totalEnergy; // in ms

        // LF: sum of lateral fraction from LF columns (col offset +1 within each band)
        float lfTotal = 0, energyTotal = 0;
        for (int b = 0; b < numFreqBands; b++) {
            int eColIdx = 5 + b * colsPerBand;
            int lfColIdx = eColIdx + 1;
            if (lfColIdx >= (int)gabe.columns.size()) break;
            auto& eCol = gabe.columns[eColIdx];
            auto& lfCol = gabe.columns[lfColIdx];
            if (eCol.type != GABE_FLOAT || lfCol.type != GABE_FLOAT) continue;
            int n = std::min({(int)eCol.floatData.size(), (int)lfCol.floatData.size(), numTimeSteps});
            for (int t = 0; t < n; t++) {
                energyTotal += eCol.floatData[t];
                lfTotal += lfCol.floatData[t];
            }
        }
        if (energyTotal > 0) rcv->lf = 100.0f * lfTotal / energyTotal;

        printf("[GAP] Loaded acoustic params for %s: RT60=%.2fs EDT=%.2fs C80=%.1fdB D50=%.0f%% Ts=%.0fms\n",
               rcv->name.c_str(), rcv->rt60, rcv->edt, rcv->c80, rcv->d50, rcv->ts);
    }

    results.loaded = !results.receivers.empty() || !gabeFiles.empty();
    results.timestamp = resultDir;

    printf("[Results] Loaded %zu receiver results\n", results.receivers.size());
    return results.loaded;
}

// ─── Surface Receiver .csbin Reader ─────────────────────────────────────────

bool LoadCSBIN(const std::string& path, SurfaceRecResult& result) {
    std::ifstream f(path, std::ios::binary);
    if (!f.is_open()) return false;

    result = {};

    // File header
    int32_t formatVersion;
    uint32_t headerLen, nodeLen, rsLen, faceLen, faceValueLen;
    int32_t quantNodes, quantRS, nbTimeStep;
    float timeStep;
    int32_t recordType;

    f.read((char*)&formatVersion, 4);
    f.read((char*)&headerLen, 4);
    f.read((char*)&nodeLen, 4);
    f.read((char*)&rsLen, 4);
    f.read((char*)&faceLen, 4);
    f.read((char*)&faceValueLen, 4);
    f.read((char*)&quantNodes, 4);
    f.read((char*)&quantRS, 4);
    f.read((char*)&nbTimeStep, 4);
    f.read((char*)&timeStep, 4);
    f.read((char*)&recordType, 4);

    // Skip rest of header padding
    int headerRead = 44;
    if ((int)headerLen > headerRead)
        f.seekg(headerLen - headerRead, std::ios::cur);

    result.recordType = recordType;

    // Read nodes (use nodeLen from header for alignment)
    result.nodes.resize(quantNodes);
    for (int i = 0; i < quantNodes; i++) {
        std::streampos nodeStart = f.tellg();
        float x, y, z;
        f.read((char*)&x, 4);
        f.read((char*)&y, 4);
        f.read((char*)&z, 4);
        // I-Simpa coords -> GL: X->X, Z->Y, -Y->Z
        result.nodes[i] = glm::vec3(x, z, -y);
        f.seekg(nodeStart + (std::streampos)nodeLen);
    }

    // Read receptors — match original rsbin.cpp reader exactly
    // sizeof(t_RecepteurS) = 4 + 4 + 255 = 263
    // sizeof(t_FaceRS) = 4*3 + 4 = 16
    // sizeof(t_faceValue) = 2 + 4 = 6
    // Only apply padding skip if our struct sizes differ from header values
    // The original reader only applies padding skips when formatVersion != compiled VERSION (3).
    // Our struct sizes: RS=263, Face=16, FaceVal=6, Node=12
    // The header stores the struct sizes from when the file was written.
    // Only skip extra bytes if the file was written by a DIFFERENT version.
    const int CURRENT_VERSION = 3;
    bool versionConflict = (formatVersion != CURRENT_VERSION);

    // Struct sizes we expect (version 3)
    const int MY_RS_SIZE = 263;    // 4 + 4 + 255
    const int MY_FACE_SIZE = 16;   // 4 * 4
    const int MY_FVAL_SIZE = 6;    // 2 + 4
    const int MY_NODE_SIZE = 12;   // 4 * 3

    if (versionConflict) {
        printf("[CSBIN] Version conflict: file=%d, ours=%d\n", formatVersion, CURRENT_VERSION);
    }

    for (int r = 0; r < quantRS; r++) {
        // Read RS header using exact size from file header (handles alignment padding)
        std::streampos rsStart = f.tellg();
        int32_t xmlIndex, quantFaces;
        char rsName[255] = {};

        f.read((char*)&xmlIndex, 4);
        f.read((char*)&quantFaces, 4);
        f.read(rsName, 255);
        // Always skip to rsLen boundary (handles struct alignment padding)
        f.seekg(rsStart + (std::streampos)rsLen);

        if (result.name.empty()) result.name = rsName;

        for (int fi = 0; fi < quantFaces; fi++) {
            std::streampos faceStart = f.tellg();
            int32_t v0, v1, v2, nbRecords;
            f.read((char*)&v0, 4);
            f.read((char*)&v1, 4);
            f.read((char*)&v2, 4);
            f.read((char*)&nbRecords, 4);
            // Skip to exact face header size from file header
            f.seekg(faceStart + (std::streampos)faceLen);

            // Sanity check
            if (v0 < 0 || v0 >= quantNodes || v1 < 0 || v1 >= quantNodes ||
                v2 < 0 || v2 >= quantNodes || nbRecords < 0 || nbRecords > 1000000) {
                printf("[CSBIN] Bad face data at RS %d, face %d: v=(%d,%d,%d) rec=%d\n",
                       r, fi, v0, v1, v2, nbRecords);
                result.loaded = false;
                f.close();
                return false;
            }

            float energySum = 0;
            for (int rec = 0; rec < nbRecords; rec++) {
                std::streampos valStart = f.tellg();
                // t_faceValue may be padded: uint16(2) + pad(2) + float(4) = 8 bytes
                // Read the full struct and extract fields at correct offsets
                char valBuf[16] = {};
                f.read(valBuf, (std::min)((uint32_t)sizeof(valBuf), faceValueLen));
                // timeStep at offset 0 (uint16), energy at offset (faceValueLen - 4)
                float energy;
                memcpy(&energy, valBuf + faceValueLen - 4, 4);
                f.seekg(valStart + (std::streampos)faceValueLen);
                energySum += energy;
            }

            float avg = (nbRecords > 0) ? energySum / (float)nbRecords : 0;

            SurfRecFace face;
            face.v[0] = (uint32_t)v0;
            face.v[1] = (uint32_t)v1;
            face.v[2] = (uint32_t)v2;
            face.energySum = avg;
            result.faces.push_back(face);
        }
    }

    // Compute min/max for colormap
    if (!result.faces.empty()) {
        result.minEnergy = result.faces[0].energySum;
        result.maxEnergy = result.faces[0].energySum;
        for (auto& face : result.faces) {
            if (face.energySum < result.minEnergy) result.minEnergy = face.energySum;
            if (face.energySum > result.maxEnergy) result.maxEnergy = face.energySum;
        }
    }

    result.loaded = true;
    f.close();

    // Debug: show first 3 face energies
    for (int i = 0; i < (std::min)(3, (int)result.faces.size()); i++) {
        printf("[CSBIN]   face %d: v=(%u,%u,%u) energy=%.4e\n",
               i, result.faces[i].v[0], result.faces[i].v[1], result.faces[i].v[2], result.faces[i].energySum);
    }
    printf("[CSBIN] Loaded: %s (%d nodes, %zu faces, range %.4e — %.4e, fvlen=%u)\n",
           path.c_str(), quantNodes, result.faces.size(), result.minEnergy, result.maxEnergy, faceValueLen);
    return true;
}

// ─── Particle Trajectory .pbin Reader ───────────────────────────────────────

bool LoadPBIN(const std::string& path, ParticleData& data) {
    std::ifstream f(path, std::ios::binary);
    if (!f.is_open()) return false;

    data = {};

    // File header (28 bytes)
    uint32_t nbParticles, formatVersion, fileInfoLen, particleInfoLen, particleHeaderLen, nbTimeStepMax;
    float timeStep;

    f.read((char*)&nbParticles, 4);
    f.read((char*)&formatVersion, 4);
    f.read((char*)&fileInfoLen, 4);
    f.read((char*)&particleInfoLen, 4);
    f.read((char*)&particleHeaderLen, 4);
    f.read((char*)&nbTimeStepMax, 4);
    f.read((char*)&timeStep, 4);

    data.timeStep = timeStep;
    data.maxTimeSteps = (int)nbTimeStepMax;

    // Skip any extra header bytes
    if (fileInfoLen > 28)
        f.seekg(fileInfoLen - 28, std::ios::cur);

    data.particles.resize(nbParticles);

    for (uint32_t i = 0; i < nbParticles; i++) {
        auto& p = data.particles[i];

        // Particle header (6 bytes)
        uint32_t nbTimeStep;
        uint16_t firstTimeStep;
        f.read((char*)&nbTimeStep, 4);
        f.read((char*)&firstTimeStep, 2);

        // Skip any extra particle header bytes
        if (particleHeaderLen > 6)
            f.seekg(particleHeaderLen - 6, std::ios::cur);

        p.firstTimeStep = firstTimeStep;
        p.steps.resize(nbTimeStep);

        for (uint32_t t = 0; t < nbTimeStep; t++) {
            float x, y, z, energy;
            f.read((char*)&x, 4);
            f.read((char*)&y, 4);
            f.read((char*)&z, 4);
            f.read((char*)&energy, 4);

            // I-Simpa coords -> GL
            p.steps[t].position = glm::vec3(x, z, -y);
            p.steps[t].energy = energy;

            // Skip extra per-step bytes
            if (particleInfoLen > 16)
                f.seekg(particleInfoLen - 16, std::ios::cur);
        }
    }

    data.loaded = true;
    f.close();

    printf("[PBIN] Loaded: %s (%u particles, %u max timesteps, dt=%.4fs)\n",
           path.c_str(), nbParticles, nbTimeStepMax, timeStep);
    return true;
}

} // namespace isimpa
