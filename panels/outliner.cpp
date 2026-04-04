#include "app/theme.h"
#include "app/selection.h"
#include "viewport/viewport.h"
#include "project/project.h"
#include <imgui.h>
#include <cstring>
#include <cctype>
#include <cstdio>

namespace isimpa {

static char s_filterBuf[128] = {};

static bool MatchesFilter(const char* name) {
    if (s_filterBuf[0] == '\0') return true;
    size_t hLen = strlen(name), nLen = strlen(s_filterBuf);
    if (nLen > hLen) return false;
    for (size_t i = 0; i <= hLen - nLen; i++) {
        bool match = true;
        for (size_t j = 0; j < nLen; j++) {
            if (tolower(name[i+j]) != tolower(s_filterBuf[j])) { match = false; break; }
        }
        if (match) return true;
    }
    return false;
}

void DrawOutliner() {
    if (!ImGui::Begin("Outliner")) { ImGui::End(); return; }

    Project& proj = GetProject();
    SceneModel& model = proj.model;

    // Search bar
    ImGui::PushItemWidth(-1);
    ImGui::InputTextWithHint("##OutlinerFilter", "Filter...", s_filterBuf, sizeof(s_filterBuf));
    ImGui::PopItemWidth();
    ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

    // ── SCENE ───────────────────────────────────────────────────────────────
    ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
    ImGui::Text("SCENE");
    ImGui::PopStyleColor();

    // Room Geometry
    if (MatchesFilter("Room Geometry") && ImGui::TreeNodeEx("Room Geometry", ImGuiTreeNodeFlags_DefaultOpen)) {
        if (model.IsEmpty()) {
            ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
                "  (no geometry)");
        } else {
            for (int i = 0; i < (int)model.groups.size(); i++) {
                auto& grp = model.groups[i];
                if (!MatchesFilter(grp.name.c_str())) continue;
                ImGui::PushID(i);
                ImVec2 pos = ImGui::GetCursorScreenPos();
                ImGui::GetWindowDrawList()->AddCircleFilled(
                    ImVec2(pos.x + 6, pos.y + 8), 4,
                    ImGui::GetColorU32(ImVec4(grp.color.r, grp.color.g, grp.color.b, 1.0f)));
                ImGui::SetCursorPosX(ImGui::GetCursorPosX() + 16);

                // Material name if assigned
                std::string matLabel;
                auto it = proj.groupMaterialMap.find(i);
                if (it != proj.groupMaterialMap.end() && it->second < (int)proj.materials.size())
                    matLabel = " [" + proj.materials[it->second].name + "]";

                char label[256];
                snprintf(label, sizeof(label), "%s (%d faces, %.1f m2)%s",
                    grp.name.c_str(), (int)grp.faces.size(), grp.surfaceArea, matLabel.c_str());
                if (ImGui::Selectable(label, GetSelection().IsGroupSelected(i))) {
                    ImGuiIO& io = ImGui::GetIO();
                    if (io.KeyShift || io.KeyCtrl) {
                        GetSelection().ToggleGroup(i);
                    } else {
                        GetSelection().SelectGroup(i);
                    }
                }
                // Right-click context menu for groups
                if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
                    ImGui::OpenPopup("##GrpCtx");
                    GetSelection().SelectGroup(i);
                }
                if (ImGui::BeginPopup("##GrpCtx")) {
                    static char renameBuf[128];
                    if (ImGui::IsWindowAppearing()) {
                        strncpy(renameBuf, grp.name.c_str(), sizeof(renameBuf) - 1);
                    }
                    ImGui::Text("Rename:");
                    ImGui::PushItemWidth(150);
                    if (ImGui::InputText("##RenameGrp", renameBuf, sizeof(renameBuf),
                            ImGuiInputTextFlags_EnterReturnsTrue)) {
                        grp.name = renameBuf;
                        ImGui::CloseCurrentPopup();
                    }
                    ImGui::PopItemWidth();
                    ImGui::EndPopup();
                }
                ImGui::PopID();
            }
            int tf = 0; for (auto& g : model.groups) tf += (int)g.faces.size();
            ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
                "  %zu verts, %d faces", model.vertices.size(), tf);
        }
        ImGui::TreePop();
    }

    // Sound Sources
    if (MatchesFilter("Sound Sources") && ImGui::TreeNodeEx("Sound Sources", ImGuiTreeNodeFlags_DefaultOpen)) {
        for (int i = 0; i < (int)proj.sources.size(); i++) {
            auto& src = proj.sources[i];
            ImGui::PushID(1000 + i);
            ImVec2 pos = ImGui::GetCursorScreenPos();
            ImGui::GetWindowDrawList()->AddCircleFilled(
                ImVec2(pos.x + 6, pos.y + 8), 4,
                ImGui::GetColorU32(ImVec4(src.displayColor.r, src.displayColor.g, src.displayColor.b, 1.0f)));
            ImGui::SetCursorPosX(ImGui::GetCursorPosX() + 16);
            char label[128];
            snprintf(label, sizeof(label), "%s (%.0f dB)%s", src.name.c_str(), src.globalPowerDb,
                     src.active ? "" : " [OFF]");
            if (ImGui::Selectable(label, GetSelection().source == i)) {
                GetSelection().SelectSource(i);
            }
            if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
                ImGui::OpenPopup("##SrcCtx");
            }
            if (ImGui::BeginPopup("##SrcCtx")) {
                if (ImGui::MenuItem("Delete")) {
                    proj.sources.erase(proj.sources.begin() + i);
                    GetSelection().Clear();
                    ImGui::EndPopup();
                    ImGui::PopID();
                    break;
                }
                ImGui::EndPopup();
            }
            ImGui::PopID();
        }
        if (ImGui::SmallButton("+ Add Source")) {
            proj.AddSource();
        }
        ImGui::TreePop();
    }

    // Punctual Receivers
    if (MatchesFilter("Receivers") && ImGui::TreeNodeEx("Punctual Receivers", ImGuiTreeNodeFlags_DefaultOpen)) {
        for (int i = 0; i < (int)proj.punctualReceivers.size(); i++) {
            auto& rcv = proj.punctualReceivers[i];
            ImGui::PushID(2000 + i);
            ImVec2 pos = ImGui::GetCursorScreenPos();
            ImGui::GetWindowDrawList()->AddCircleFilled(
                ImVec2(pos.x + 6, pos.y + 8), 4,
                ImGui::GetColorU32(ImVec4(rcv.displayColor.r, rcv.displayColor.g, rcv.displayColor.b, 1.0f)));
            ImGui::SetCursorPosX(ImGui::GetCursorPosX() + 16);
            char label[128];
            snprintf(label, sizeof(label), "%s (%.1f, %.1f, %.1f)", rcv.name.c_str(),
                     rcv.position.x, rcv.position.y, rcv.position.z);
            if (ImGui::Selectable(label, GetSelection().receiver == i)) {
                GetSelection().SelectReceiver(i);
            }
            if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
                ImGui::OpenPopup("##RcvCtx");
            }
            if (ImGui::BeginPopup("##RcvCtx")) {
                if (ImGui::MenuItem("Delete")) {
                    proj.punctualReceivers.erase(proj.punctualReceivers.begin() + i);
                    GetSelection().Clear();
                    ImGui::EndPopup();
                    ImGui::PopID();
                    break;
                }
                ImGui::EndPopup();
            }
            ImGui::PopID();
        }
        if (ImGui::SmallButton("+ Add Receiver")) {
            proj.AddReceiver();
        }
        ImGui::TreePop();
    }

    // Surface Receivers
    if (MatchesFilter("Surface") && ImGui::TreeNode("Surface Receivers")) {
        for (int i = 0; i < (int)proj.surfaceReceivers.size(); i++) {
            auto& sr = proj.surfaceReceivers[i];
            ImGui::PushID(3000 + i);
            ImGui::Text("  %s (%.1f m res)", sr.name.c_str(), sr.gridResolution);
            if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
                ImGui::OpenPopup("##SrCtx");
            }
            if (ImGui::BeginPopup("##SrCtx")) {
                if (ImGui::MenuItem("Delete")) {
                    proj.surfaceReceivers.erase(proj.surfaceReceivers.begin() + i);
                    ImGui::EndPopup();
                    ImGui::PopID();
                    break;
                }
                ImGui::EndPopup();
            }
            ImGui::PopID();
        }
        if (ImGui::SmallButton("+ Add Surface Receiver"))
            proj.AddSurfaceReceiver();
        ImGui::TreePop();
    }

    // Fitting Zones (Encumbrances)
    if (MatchesFilter("Fitting Zones") && ImGui::TreeNodeEx("Fitting Zones", ImGuiTreeNodeFlags_DefaultOpen)) {
        for (int i = 0; i < (int)proj.encumbrances.size(); i++) {
            auto& enc = proj.encumbrances[i];
            if (!MatchesFilter(enc.name.c_str())) continue;
            ImGui::PushID(4000 + i);
            ImVec2 pos = ImGui::GetCursorScreenPos();
            ImGui::GetWindowDrawList()->AddCircleFilled(
                ImVec2(pos.x + 6, pos.y + 8), 4,
                ImGui::GetColorU32(ImVec4(enc.color.x, enc.color.y, enc.color.z, 1.0f)));
            ImGui::SetCursorPosX(ImGui::GetCursorPosX() + 16);
            glm::vec3 dims = enc.boxMax - enc.boxMin;
            char label[256];
            snprintf(label, sizeof(label), "%s (%.1f x %.1f x %.1f m)%s",
                enc.name.c_str(), dims.x, dims.y, dims.z,
                enc.active ? "" : " [OFF]");
            ImGui::Selectable(label);
            if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
                ImGui::OpenPopup("##EncCtx");
            }
            if (ImGui::BeginPopup("##EncCtx")) {
                if (ImGui::MenuItem("Delete")) {
                    proj.encumbrances.erase(proj.encumbrances.begin() + i);
                    ImGui::EndPopup();
                    ImGui::PopID();
                    break;
                }
                ImGui::EndPopup();
            }
            ImGui::PopID();
        }
        if (ImGui::SmallButton("+ Add Fitting Zone")) {
            proj.AddEncumbrance();
        }
        ImGui::TreePop();
    }

    // Volumes
    if (MatchesFilter("Volumes") && ImGui::TreeNodeEx("Volumes", ImGuiTreeNodeFlags_DefaultOpen)) {
        for (int i = 0; i < (int)proj.volumes.size(); i++) {
            auto& vol = proj.volumes[i];
            if (!MatchesFilter(vol.name.c_str())) continue;
            ImGui::PushID(5000 + i);
            glm::vec3 dims = vol.boxMax - vol.boxMin;
            char label[256];
            snprintf(label, sizeof(label), "%s (%.1f x %.1f x %.1f m, %.0fC, %.0f%% RH)",
                vol.name.c_str(), dims.x, dims.y, dims.z,
                vol.temperature, vol.humidity);
            ImGui::Text("  %s", label);
            if (ImGui::IsItemHovered() && ImGui::IsMouseClicked(ImGuiMouseButton_Right)) {
                ImGui::OpenPopup("##VolCtx");
            }
            if (ImGui::BeginPopup("##VolCtx")) {
                if (ImGui::MenuItem("Delete")) {
                    proj.volumes.erase(proj.volumes.begin() + i);
                    ImGui::EndPopup();
                    ImGui::PopID();
                    break;
                }
                ImGui::EndPopup();
            }
            ImGui::PopID();
        }
        if (ImGui::SmallButton("+ Add Volume")) {
            proj.AddVolume();
        }
        ImGui::TreePop();
    }

    ImGui::Spacing(); ImGui::Separator(); ImGui::Spacing();

    // ── DATABASE ────────────────────────────────────────────────────────────
    ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
    ImGui::Text("DATABASE");
    ImGui::PopStyleColor();

    if (MatchesFilter("Materials") && ImGui::TreeNode("Materials")) {
        for (auto& mat : proj.materials)
            ImGui::Text("  %s (avg a=%.2f)", mat.name.c_str(), mat.AverageAbsorption());
        ImGui::TreePop();
    }

    ImGui::End();
}

} // namespace isimpa
