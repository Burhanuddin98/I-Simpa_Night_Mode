#include "app/theme.h"
#include "app/selection.h"
#include "project/project.h"
#include "project/solver.h"
#include "viewport/viewport.h"
#include <imgui.h>
#include <cstdio>
#include <cctype>
#include <cstring>
#include <cmath>

#ifdef _WIN32
#include <windows.h>
#include <commdlg.h>
#endif

namespace isimpa {

static int s_assigningMaterial = -1; // material index being assigned, -1 = not assigning
static int s_editingMaterial = -1;   // material index being edited for per-band data

void DrawMaterials() {
    if (!ImGui::Begin("Materials")) { ImGui::End(); return; }

    Project& proj = GetProject();

    // Search + Add button
    static char filterBuf[64] = {};
    ImGui::PushItemWidth(ImGui::GetContentRegionAvail().x - 80);
    ImGui::InputTextWithHint("##MatFilter", "Search...", filterBuf, sizeof(filterBuf));
    ImGui::PopItemWidth();
    ImGui::SameLine();

    ImGui::PushStyleColor(ImGuiCol_Button,
        ImVec4(NeonColors::Cyan[0]*0.3f, NeonColors::Cyan[1]*0.3f, NeonColors::Cyan[2]*0.3f, 1.0f));
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered,
        ImVec4(NeonColors::Cyan[0]*0.5f, NeonColors::Cyan[1]*0.5f, NeonColors::Cyan[2]*0.5f, 1.0f));
    if (ImGui::Button("+ New", ImVec2(50, 0))) {
        AcousticMaterial mat;
        mat.id = (int)proj.materials.size();
        mat.name = "New Material";
        mat.color = glm::vec3(0.5f, 0.5f, 0.5f);
        proj.materials.push_back(mat);
    }
    ImGui::PopStyleColor(2);
    ImGui::SameLine();
    if (ImGui::SmallButton("Export")) {
#ifdef _WIN32
        char filename[MAX_PATH] = {};
        OPENFILENAMEA ofn = {};
        ofn.lStructSize = sizeof(ofn);
        ofn.lpstrFilter = "Material Database (*.xml)\0*.xml\0";
        ofn.lpstrFile = filename;
        ofn.nMaxFile = MAX_PATH;
        ofn.lpstrTitle = "Export Materials";
        ofn.lpstrDefExt = "xml";
        ofn.Flags = OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
        if (GetSaveFileNameA(&ofn)) {
            ExportMaterials(proj.materials, filename);
        }
#endif
    }
    ImGui::SameLine();
    if (ImGui::SmallButton("Import")) {
#ifdef _WIN32
        char filename[MAX_PATH] = {};
        OPENFILENAMEA ofn = {};
        ofn.lStructSize = sizeof(ofn);
        ofn.lpstrFilter = "Material Database (*.xml)\0*.xml\0";
        ofn.lpstrFile = filename;
        ofn.nMaxFile = MAX_PATH;
        ofn.lpstrTitle = "Import Materials";
        ofn.Flags = OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR;
        if (GetOpenFileNameA(&ofn)) {
            ImportMaterials(proj.materials, filename);
        }
#endif
    }

    // Assignment mode indicator
    if (s_assigningMaterial >= 0 && s_assigningMaterial < (int)proj.materials.size()) {
        ImGui::Spacing();
        ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1], NeonColors::Yellow[2], 1.0f));
        ImGui::TextWrapped("ASSIGN MODE: Click a surface group in the Outliner to assign \"%s\"",
            proj.materials[s_assigningMaterial].name.c_str());
        ImGui::PopStyleColor();
        if (ImGui::SmallButton("Cancel")) s_assigningMaterial = -1;

        // Check if a group was selected while in assign mode
        Selection& sel = GetSelection();
        if (sel.group >= 0) {
            proj.AssignMaterial(sel.group, s_assigningMaterial);
            s_assigningMaterial = -1;
        }
    }

    ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

    // Material cards from real project data
    float cardWidth = ImGui::GetContentRegionAvail().x;

    for (int i = 0; i < (int)proj.materials.size(); i++) {
        auto& mat = proj.materials[i];

        // Filter
        if (filterBuf[0] != '\0') {
            bool found = false;
            const char* h = mat.name.c_str();
            const char* n = filterBuf;
            size_t hLen = strlen(h), nLen = strlen(n);
            for (size_t j = 0; j + nLen <= hLen; j++) {
                bool match = true;
                for (size_t k = 0; k < nLen; k++) {
                    if (tolower(h[j+k]) != tolower(n[k])) { match = false; break; }
                }
                if (match) { found = true; break; }
            }
            if (!found) continue;
        }

        ImGui::PushID(i);

        bool isEditing = (s_editingMaterial == i);
        float cardHeight = isEditing ? 0 : 72.0f; // Auto-height when editing

        if (!isEditing) {
            // ── Compact card view ───────────────────────────────────────────
            ImVec2 pos = ImGui::GetCursorScreenPos();
            ImDrawList* dl = ImGui::GetWindowDrawList();

            bool isAssigning = (s_assigningMaterial == i);
            ImVec4 bgCol = isAssigning ? ImVec4(0.0f, 0.15f, 0.16f, 1.0f) : ImVec4(0.10f, 0.10f, 0.14f, 1.0f);
            ImVec2 cardMin = pos;
            ImVec2 cardMax = ImVec2(pos.x + cardWidth, pos.y + 72.0f);
            dl->AddRectFilled(cardMin, cardMax, ImGui::GetColorU32(bgCol), 4.0f);

            // Color swatch
            float swatchSize = 40.0f;
            ImVec2 swMin(pos.x + 8, pos.y + (72.0f - swatchSize) * 0.5f);
            ImVec2 swMax(swMin.x + swatchSize, swMin.y + swatchSize);
            dl->AddRectFilled(swMin, swMax,
                ImGui::GetColorU32(ImVec4(mat.color.r, mat.color.g, mat.color.b, 1.0f)), 4.0f);

            // Name
            ImVec2 textPos(pos.x + 58, pos.y + 8);
            dl->AddText(textPos, ImGui::GetColorU32(ImVec4(
                NeonColors::TextBright[0], NeonColors::TextBright[1], NeonColors::TextBright[2], 1.0f)),
                mat.name.c_str());

            // Average absorption
            char absBuf[32];
            snprintf(absBuf, sizeof(absBuf), "avg alpha: %.3f", mat.AverageAbsorption());
            dl->AddText(ImVec2(textPos.x, textPos.y + 18), ImGui::GetColorU32(ImVec4(
                NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f)), absBuf);

            // Mini sparkline (absorption curve)
            float sparkX = pos.x + 58;
            float sparkY = pos.y + 46;
            float sparkW = cardWidth - 70;
            float sparkH = 18.0f;

            int octaveIdx[7] = {4, 7, 10, 13, 16, 19, 22};
            for (int j = 0; j < 6; j++) {
                float x0 = sparkX + (sparkW / 6.0f) * j;
                float x1 = sparkX + (sparkW / 6.0f) * (j + 1);
                float y0 = sparkY + sparkH * (1.0f - mat.bands[octaveIdx[j]].absorption);
                float y1 = sparkY + sparkH * (1.0f - mat.bands[octaveIdx[j+1]].absorption);
                dl->AddLine(ImVec2(x0, y0), ImVec2(x1, y1),
                    ImGui::GetColorU32(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 0.7f)), 1.5f);
            }

            // Click to assign (left), double-click to edit
            if (ImGui::InvisibleButton("##card", ImVec2(cardWidth, 72.0f))) {
                s_assigningMaterial = i;
            }
            if (ImGui::IsItemHovered() && ImGui::IsMouseDoubleClicked(0)) {
                s_editingMaterial = i;
                s_assigningMaterial = -1;
            }

            if (ImGui::IsItemHovered()) {
                dl->AddRect(cardMin, cardMax,
                    ImGui::GetColorU32(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 0.6f)),
                    4.0f, 0, 1.5f);
                ImGui::SetTooltip("Click = assign mode | Double-click = edit bands");
            }

            if (isAssigning) {
                dl->AddRect(cardMin, cardMax,
                    ImGui::GetColorU32(ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1], NeonColors::Yellow[2], 0.8f)),
                    4.0f, 0, 2.0f);
            }
        } else {
            // ── Expanded editor view ────────────────────────────────────────
            ImGui::PushStyleColor(ImGuiCol_ChildBg, ImVec4(0.10f, 0.10f, 0.14f, 1.0f));
            ImGui::BeginChild("##MatEdit", ImVec2(cardWidth, 0), ImGuiChildFlags_AutoResizeY | ImGuiChildFlags_Borders);

            // Header with name + color + close
            static char nameBuf[128];
            if (ImGui::IsWindowAppearing()) {
                strncpy(nameBuf, mat.name.c_str(), sizeof(nameBuf) - 1);
            }
            ImGui::PushItemWidth(200);
            if (ImGui::InputText("Name", nameBuf, sizeof(nameBuf))) {
                mat.name = nameBuf;
            }
            ImGui::PopItemWidth();
            ImGui::SameLine();
            ImGui::ColorEdit3("##MatColor", &mat.color.x, ImGuiColorEditFlags_NoInputs);
            ImGui::SameLine(cardWidth - 60);
            if (ImGui::SmallButton("Close")) {
                s_editingMaterial = -1;
            }

            ImGui::Spacing();
            ImGui::Text("Avg alpha: %.3f  |  Resistivity: %.0f", mat.AverageAbsorption(), mat.resistivity);
            ImGui::DragFloat("Resistivity", &mat.resistivity, 10, 0, 100000);

            ImGui::Spacing();
            ImGui::Separator();
            ImGui::Spacing();

            // ── Per-band absorption/diffusion editor ────────────────────────
            ImGui::TextColored(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f),
                "Frequency Band Absorption & Diffusion");
            ImGui::Spacing();

            // Octave band quick-edit (7 main bands)
            ImGui::Text("Octave bands:");
            const char* octLabels[] = {"125", "250", "500", "1k", "2k", "4k", "8k"};
            int octIdx[] = {4, 7, 10, 13, 16, 19, 22};

            // Absorption row
            ImGui::Text("Abs:");
            ImGui::SameLine(50);
            for (int j = 0; j < 7; j++) {
                ImGui::PushID(100 + j);
                ImGui::PushItemWidth(52);
                char label[16];
                snprintf(label, sizeof(label), "%s Hz", octLabels[j]);
                ImGui::VSliderFloat("##abs", ImVec2(36, 80), &mat.bands[octIdx[j]].absorption, 0.0f, 1.0f, "");
                if (ImGui::IsItemHovered()) {
                    ImGui::SetTooltip("%s Hz: %.3f", octLabels[j], mat.bands[octIdx[j]].absorption);
                }
                ImGui::PopItemWidth();
                ImGui::PopID();
                if (j < 6) ImGui::SameLine();
            }

            // Labels under sliders
            ImGui::Text("     ");
            ImGui::SameLine(50);
            for (int j = 0; j < 7; j++) {
                ImGui::Text("%-5s", octLabels[j]);
                if (j < 6) ImGui::SameLine();
            }

            ImGui::Spacing();

            // Interpolate between octave bands button
            if (ImGui::Button("Interpolate Third-Octave")) {
                // Fill in the third-octave bands by interpolation from octave bands
                for (int b = 0; b < 27; b++) {
                    bool isOctave = false;
                    for (int j = 0; j < 7; j++) { if (b == octIdx[j]) { isOctave = true; break; } }
                    if (isOctave) continue;

                    int lo = -1, hi = -1;
                    for (int j = b - 1; j >= 0; j--) {
                        for (int k = 0; k < 7; k++) { if (j == octIdx[k]) { lo = j; break; } }
                        if (lo >= 0) break;
                    }
                    for (int j = b + 1; j < 27; j++) {
                        for (int k = 0; k < 7; k++) { if (j == octIdx[k]) { hi = j; break; } }
                        if (hi >= 0) break;
                    }

                    if (lo >= 0 && hi >= 0) {
                        float t = (float)(b - lo) / (float)(hi - lo);
                        mat.bands[b].absorption = mat.bands[lo].absorption * (1 - t) + mat.bands[hi].absorption * t;
                        mat.bands[b].diffusion = mat.bands[lo].diffusion * (1 - t) + mat.bands[hi].diffusion * t;
                    } else if (lo >= 0) {
                        mat.bands[b].absorption = mat.bands[lo].absorption;
                        mat.bands[b].diffusion = mat.bands[lo].diffusion;
                    } else if (hi >= 0) {
                        mat.bands[b].absorption = mat.bands[hi].absorption;
                        mat.bands[b].diffusion = mat.bands[hi].diffusion;
                    }
                }
            }

            ImGui::Spacing();

            // Full 27-band table (collapsible)
            if (ImGui::TreeNode("All 27 Third-Octave Bands")) {
                ImGui::BeginChild("##BandTable", ImVec2(0, 300));
                if (ImGui::BeginTable("bands", 4, ImGuiTableFlags_Borders | ImGuiTableFlags_RowBg | ImGuiTableFlags_ScrollY)) {
                    ImGui::TableSetupColumn("Freq (Hz)", ImGuiTableColumnFlags_WidthFixed, 70);
                    ImGui::TableSetupColumn("Absorption", ImGuiTableColumnFlags_WidthStretch);
                    ImGui::TableSetupColumn("Diffusion", ImGuiTableColumnFlags_WidthStretch);
                    ImGui::TableSetupColumn("Law", ImGuiTableColumnFlags_WidthFixed, 60);
                    ImGui::TableHeadersRow();

                    for (int b = 0; b < 27; b++) {
                        ImGui::TableNextRow();
                        ImGui::PushID(200 + b);

                        ImGui::TableSetColumnIndex(0);
                        ImGui::Text("%d", (int)AcousticMaterial::BAND_FREQS[b]);

                        ImGui::TableSetColumnIndex(1);
                        ImGui::PushItemWidth(-1);
                        ImGui::SliderFloat("##abs", &mat.bands[b].absorption, 0.0f, 1.0f, "%.3f");
                        ImGui::PopItemWidth();

                        ImGui::TableSetColumnIndex(2);
                        ImGui::PushItemWidth(-1);
                        ImGui::SliderFloat("##diff", &mat.bands[b].diffusion, 0.0f, 1.0f, "%.3f");
                        ImGui::PopItemWidth();

                        ImGui::TableSetColumnIndex(3);
                        ImGui::PushItemWidth(-1);
                        const char* laws[] = {"Lamb", "Spec", "Uni", "W2", "W3", "W4"};
                        ImGui::Combo("##law", &mat.bands[b].diffusionLaw, laws, 6);
                        ImGui::PopItemWidth();

                        ImGui::PopID();
                    }
                    ImGui::EndTable();
                }
                ImGui::EndChild();
                ImGui::TreePop();
            }

            ImGui::Spacing();

            // Assign button
            ImGui::PushStyleColor(ImGuiCol_Button,
                ImVec4(NeonColors::Cyan[0]*0.3f, NeonColors::Cyan[1]*0.3f, NeonColors::Cyan[2]*0.3f, 1.0f));
            if (ImGui::Button("Assign to Selected Group")) {
                Selection& sel = GetSelection();
                if (sel.group >= 0) {
                    proj.AssignMaterial(sel.group, i);
                }
            }
            ImGui::PopStyleColor();

            ImGui::SameLine();
            if (ImGui::Button("Delete Material")) {
                if (i > 0) { // Don't delete first material
                    proj.materials.erase(proj.materials.begin() + i);
                    s_editingMaterial = -1;
                    // Fix IDs
                    for (int m = 0; m < (int)proj.materials.size(); m++) proj.materials[m].id = m;
                }
            }

            ImGui::EndChild();
            ImGui::PopStyleColor();
        }

        ImGui::Dummy(ImVec2(0, 4));
        ImGui::PopID();
    }

    ImGui::End();
}

} // namespace isimpa
