#include "app/theme.h"
#include "app/selection.h"
#include "project/project.h"
#include "viewport/viewport.h"
#include <imgui.h>
#include <cstdio>

namespace isimpa {

void DrawProperties() {
    if (!ImGui::Begin("Properties")) { ImGui::End(); return; }

    Project& proj = GetProject();
    Selection& sel = GetSelection();

    // ── Source properties ────────────────────────────────────────────────────
    if (sel.source >= 0 && sel.source < (int)proj.sources.size()) {
        auto& src = proj.sources[sel.source];

        ImGui::TextColored(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f),
            "SOUND SOURCE: %s", src.name.c_str());
        ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

        if (ImGui::CollapsingHeader("Transform", ImGuiTreeNodeFlags_DefaultOpen)) {
            ImGui::DragFloat3("Position (m)", &src.position.x, 0.1f);
            ImGui::DragFloat3("Direction", &src.direction.x, 0.01f);
        }
        if (ImGui::CollapsingHeader("Source Properties", ImGuiTreeNodeFlags_DefaultOpen)) {
            ImGui::Checkbox("Active", &src.active);
            ImGui::DragFloat("Global Power (dB)", &src.globalPowerDb, 0.5f, 0.0f, 150.0f);
            ImGui::DragFloat("Delay (s)", &src.delaySeconds, 0.001f, 0.0f, 10.0f);
            const char* dirItems[] = {"Omnidirectional", "Unidirectional", "XY Plane", "YZ Plane", "XZ Plane"};
            ImGui::Combo("Directivity", &src.directivity, dirItems, 5);
        }
        if (ImGui::CollapsingHeader("Display")) {
            ImGui::ColorEdit3("Color", &src.displayColor.x);
        }
    }
    // ── Receiver properties ─────────────────────────────────────────────────
    else if (sel.receiver >= 0 && sel.receiver < (int)proj.punctualReceivers.size()) {
        auto& rcv = proj.punctualReceivers[sel.receiver];

        ImGui::TextColored(ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f),
            "RECEIVER: %s", rcv.name.c_str());
        ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

        if (ImGui::CollapsingHeader("Transform", ImGuiTreeNodeFlags_DefaultOpen)) {
            ImGui::DragFloat3("Position (m)", &rcv.position.x, 0.1f);
            ImGui::DragFloat3("Direction", &rcv.direction.x, 0.01f);
        }
        if (ImGui::CollapsingHeader("Display")) {
            ImGui::ColorEdit3("Color", &rcv.displayColor.x);
        }
    }
    // ── Surface group properties ────────────────────────────────────────────
    else if (sel.group >= 0 && sel.group < (int)proj.model.groups.size()) {
        auto& grp = proj.model.groups[sel.group];

        ImGui::TextColored(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f),
            "SURFACE: %s", grp.name.c_str());
        ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

        if (ImGui::CollapsingHeader("Info", ImGuiTreeNodeFlags_DefaultOpen)) {
            ImGui::Text("Faces: %d", (int)grp.faces.size());
            ImGui::Text("Area: %.2f m2", grp.surfaceArea);
        }
        if (ImGui::CollapsingHeader("Material", ImGuiTreeNodeFlags_DefaultOpen)) {
            int currentMat = -1;
            auto it = proj.groupMaterialMap.find(sel.group);
            if (it != proj.groupMaterialMap.end()) currentMat = it->second;

            if (ImGui::BeginCombo("Assign Material",
                    currentMat >= 0 ? proj.materials[currentMat].name.c_str() : "(none)")) {
                for (int i = 0; i < (int)proj.materials.size(); i++) {
                    if (ImGui::Selectable(proj.materials[i].name.c_str(), currentMat == i))
                        proj.AssignMaterial(sel.group, i);
                }
                ImGui::EndCombo();
            }
            if (currentMat >= 0) {
                ImGui::Text("Avg absorption: %.3f", proj.materials[currentMat].AverageAbsorption());
            }
        }
        if (ImGui::CollapsingHeader("Display")) {
            if (ImGui::ColorEdit3("Group Color", &grp.color.x)) {
                // Re-upload on color change
                ViewportRefreshGPU();
            }
        }
    }
    // ── Nothing selected — show global settings ─────────────────────────────
    else {
        ImGui::TextColored(
            ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
            "Select an object to edit properties.\nGlobal settings shown below.");

        ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

        if (ImGui::CollapsingHeader("Environment", ImGuiTreeNodeFlags_DefaultOpen)) {
            auto& env = proj.environment;
            ImGui::DragFloat("Temperature (C)", &env.temperature, 0.5f, -20, 50);
            ImGui::DragFloat("Pressure (Pa)", &env.pressure, 100, 90000, 110000);
            ImGui::DragFloat("Humidity (%)", &env.humidity, 1, 0, 100);
            ImGui::DragFloat("Roughness z0 (m)", &env.z0, 0.001f, 0.001f, 10);
            const char* meteo[] = {"Very Favorable", "Favorable", "Homogeneous", "Unfavorable", "Very Unfavorable"};
            ImGui::Combo("Meteo", &env.meteoProfile, meteo, 5);

            // Ground type presets
            ImGui::Text("Ground Preset:");
            ImGui::SameLine();
            struct GroundPreset { const char* name; float z0; };
            static const GroundPreset presets[] = {
                {"Water", 0.0001f}, {"Bare soil", 0.005f}, {"Short grass", 0.02f},
                {"Dense grass", 0.1f}, {"Wheat field", 0.25f}, {"Sparse suburban", 0.5f},
                {"Dense suburban", 1.0f}, {"Urban", 2.0f},
            };
            for (int i = 0; i < 8; i++) {
                if (i > 0) ImGui::SameLine();
                if (ImGui::SmallButton(presets[i].name)) env.z0 = presets[i].z0;
            }
        }
        if (ImGui::CollapsingHeader("SPPS Configuration")) {
            auto& cfg = proj.sppsConfig;
            const char* methods[] = {"Random", "Energetic"};
            ImGui::Combo("Method", &cfg.calcMethod, methods, 2);
            ImGui::DragInt("Particles/Source", &cfg.particlesPerSource, 1000, 1000, 10000000);
            ImGui::DragFloat("Time Step (s)", &cfg.timeStep, 0.001f, 0.0001f, 0.1f, "%.4f");
            ImGui::DragFloat("Sim Length (s)", &cfg.simLength, 0.1f, 0.1f, 30.0f);
            ImGui::DragFloat("Receiver Radius (m)", &cfg.receiverRadius, 0.05f, 0.05f, 5.0f);
            ImGui::Checkbox("Atmo Absorption", &cfg.atmoAbsorption);
            ImGui::Checkbox("Transmission", &cfg.transmission);
            ImGui::Checkbox("Direct Field Only", &cfg.directFieldOnly);

            ImGui::Spacing();
            ImGui::Text("Frequency Presets:");
            ImGui::SameLine();
            if (ImGui::SmallButton("Octave")) {
                for (int i = 0; i < 27; i++) cfg.freqBands[i] = false;
                cfg.freqBands[4] = cfg.freqBands[7] = cfg.freqBands[10] = true; // 125,250,500
                cfg.freqBands[13] = cfg.freqBands[16] = cfg.freqBands[19] = true; // 1k,2k,4k
            }
            ImGui::SameLine();
            if (ImGui::SmallButton("1/3 Oct")) {
                for (int i = 0; i < 27; i++) cfg.freqBands[i] = true;
            }
            ImGui::SameLine();
            if (ImGui::SmallButton("Clear")) {
                for (int i = 0; i < 27; i++) cfg.freqBands[i] = false;
            }
        }
        if (ImGui::CollapsingHeader("Mesh Generation")) {
            auto& mesh = proj.meshConfig;
            ImGui::DragFloat("Quality Ratio", &mesh.qualityRatio, 0.1f, 1.0f, 10.0f, "%.1f");
            if (ImGui::IsItemHovered()) ImGui::SetTooltip("TetGen -q flag: lower = finer mesh, higher = coarser. Default 1.5");
            ImGui::Checkbox("Volume Constraint", &mesh.useVolumeConstraint);
            if (mesh.useVolumeConstraint)
                ImGui::DragFloat("Max Tet Volume (m3)", &mesh.maxVolume, 0.1f, 0.01f, 100.0f, "%.2f");
            ImGui::Checkbox("Preprocess (mesh repair)", &mesh.preprocess);
        }
        if (ImGui::CollapsingHeader("TCR Configuration")) {
            auto& tcr = proj.tcrConfig;
            ImGui::Checkbox("Atmo Absorption##tcr", &tcr.atmoAbsorption);

            ImGui::Spacing();
            ImGui::Text("Frequency Presets:");
            ImGui::SameLine();
            if (ImGui::SmallButton("Octave##tcr")) {
                for (int i = 0; i < 27; i++) tcr.freqBands[i] = false;
                tcr.freqBands[4] = tcr.freqBands[7] = tcr.freqBands[10] = true; // 125,250,500
                tcr.freqBands[13] = tcr.freqBands[16] = tcr.freqBands[19] = true; // 1k,2k,4k
            }
            ImGui::SameLine();
            if (ImGui::SmallButton("1/3 Oct##tcr")) {
                for (int i = 0; i < 27; i++) tcr.freqBands[i] = true;
            }
            ImGui::SameLine();
            if (ImGui::SmallButton("Clear##tcr")) {
                for (int i = 0; i < 27; i++) tcr.freqBands[i] = false;
            }
        }
        if (ImGui::CollapsingHeader("Fitting Zones")) {
            int deleteIdx = -1;
            for (int i = 0; i < (int)proj.encumbrances.size(); i++) {
                auto& enc = proj.encumbrances[i];
                ImGui::PushID(i);

                char label[128];
                snprintf(label, sizeof(label), "[%d] %s###enc_%d", i, enc.name.c_str(), i);
                if (ImGui::TreeNode(label)) {
                    ImGui::Checkbox("Active", &enc.active);

                    char nameBuf[256];
                    snprintf(nameBuf, sizeof(nameBuf), "%s", enc.name.c_str());
                    if (ImGui::InputText("Name", nameBuf, sizeof(nameBuf)))
                        enc.name = nameBuf;

                    ImGui::DragFloat3("Min Corner", &enc.boxMin.x, 0.1f);
                    ImGui::DragFloat3("Max Corner", &enc.boxMax.x, 0.1f);
                    ImGui::DragFloat("Absorption (500 Hz)", &enc.bands[10].absorption, 0.01f, 0.0f, 1.0f);
                    ImGui::DragFloat("Mean Free Path (m)", &enc.bands[10].meanFreePath, 0.1f, 0.01f, 50.0f);

                    const char* diffLaws[] = {"Uniform", "Specular", "Lambert"};
                    ImGui::Combo("Diffusion Law", &enc.bands[10].diffusionLaw, diffLaws, 3);

                    ImGui::ColorEdit3("Color", &enc.color.x);

                    if (ImGui::Button("Delete")) deleteIdx = i;
                    ImGui::TreePop();
                }

                ImGui::PopID();
                if (i < (int)proj.encumbrances.size() - 1) ImGui::Separator();
            }
            if (deleteIdx >= 0)
                proj.encumbrances.erase(proj.encumbrances.begin() + deleteIdx);

            ImGui::Spacing();
            if (ImGui::Button("+ Add Fitting Zone"))
                proj.AddEncumbrance();
        }
        if (ImGui::CollapsingHeader("Background Noise")) {
            ImGui::Checkbox("Enabled##bgnoise", &proj.backgroundNoise.enabled);
            if (proj.backgroundNoise.enabled) {
                const int bandIdx[] = {4, 7, 10, 13, 16, 19, 22};
                const char* bandNames[] = {"125 Hz", "250 Hz", "500 Hz", "1000 Hz", "2000 Hz", "4000 Hz", "8000 Hz"};
                for (int b = 0; b < 7; b++) {
                    ImGui::DragFloat(bandNames[b], &proj.backgroundNoise.spectrum[bandIdx[b]], 0.5f, 0.0f, 120.0f, "%.1f dB");
                }
            }
        }
        if (!proj.volumes.empty()) {
            if (ImGui::CollapsingHeader("Volume Definitions")) {
                int deleteVolIdx = -1;
                for (int i = 0; i < (int)proj.volumes.size(); i++) {
                    auto& vol = proj.volumes[i];
                    ImGui::PushID(1000 + i);

                    char vlabel[128];
                    snprintf(vlabel, sizeof(vlabel), "[%d] %s###vol_%d", i, vol.name.c_str(), i);
                    if (ImGui::TreeNode(vlabel)) {
                        char vnameBuf[256];
                        snprintf(vnameBuf, sizeof(vnameBuf), "%s", vol.name.c_str());
                        if (ImGui::InputText("Name", vnameBuf, sizeof(vnameBuf)))
                            vol.name = vnameBuf;

                        ImGui::DragFloat3("Min Corner", &vol.boxMin.x, 0.1f);
                        ImGui::DragFloat3("Max Corner", &vol.boxMax.x, 0.1f);
                        ImGui::DragFloat("Temperature (C)", &vol.temperature, 0.5f, -20.0f, 50.0f);
                        ImGui::DragFloat("Humidity (%)", &vol.humidity, 1.0f, 0.0f, 100.0f);

                        if (ImGui::Button("Delete")) deleteVolIdx = i;
                        ImGui::TreePop();
                    }

                    ImGui::PopID();
                    if (i < (int)proj.volumes.size() - 1) ImGui::Separator();
                }
                if (deleteVolIdx >= 0)
                    proj.volumes.erase(proj.volumes.begin() + deleteVolIdx);

                ImGui::Spacing();
                if (ImGui::Button("+ Add Volume"))
                    proj.AddVolume();
            }
        }
    }

    ImGui::End();
}

} // namespace isimpa
