#include "app/workflow_rail.h"
#include "app/theme.h"
#include <imgui.h>

namespace isimpa {

WorkflowRail::WorkflowRail() {
    m_phases[(size_t)WorkflowPhase::Room]      = {"ROOM",       "1", "Import or create room geometry"};
    m_phases[(size_t)WorkflowPhase::Materials]  = {"MATERIALS",  "2", "Assign acoustic materials to surfaces"};
    m_phases[(size_t)WorkflowPhase::Sources]    = {"SOURCES",    "3", "Place sound sources"};
    m_phases[(size_t)WorkflowPhase::Receivers]  = {"RECEIVERS",  "4", "Place punctual and surface receivers"};
    m_phases[(size_t)WorkflowPhase::Mesh]       = {"MESH",       "5", "Generate tetrahedral mesh"};
    m_phases[(size_t)WorkflowPhase::Simulate]   = {"SIMULATE",   "6", "Configure and run solver"};
    m_phases[(size_t)WorkflowPhase::Results]    = {"RESULTS",    "7", "View and analyze simulation results"};
}

static ImVec4 StatusColor(PhaseStatus status) {
    switch (status) {
        case PhaseStatus::NotStarted: return ImVec4(0.35f, 0.36f, 0.40f, 1.0f);
        case PhaseStatus::Incomplete: return ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1], NeonColors::Yellow[2], 1.0f);
        case PhaseStatus::Ready:      return ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f);
        case PhaseStatus::Error:      return ImVec4(NeonColors::Red[0], NeonColors::Red[1], NeonColors::Red[2], 1.0f);
    }
    return ImVec4(0.35f, 0.36f, 0.40f, 1.0f);
}

bool WorkflowRail::Draw() {
    bool changed = false;

    ImGui::PushStyleColor(ImGuiCol_ChildBg, ImVec4(0.06f, 0.06f, 0.08f, 1.0f));
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(6, 12));

    ImGui::BeginChild("##WorkflowRail", ImVec2(90, 0), ImGuiChildFlags_Borders);

    float railWidth = ImGui::GetContentRegionAvail().x;

    for (int i = 0; i < (int)WorkflowPhase::COUNT; i++) {
        auto& phase = m_phases[i];
        bool isActive = (WorkflowPhase)i == m_activePhase;
        ImVec4 dotColor = StatusColor(phase.status);

        ImGui::PushID(i);

        // Active phase highlight bar
        if (isActive) {
            ImVec2 cursorPos = ImGui::GetCursorScreenPos();
            ImDrawList* dl = ImGui::GetWindowDrawList();
            dl->AddRectFilled(
                cursorPos,
                ImVec2(cursorPos.x + 3, cursorPos.y + 52),
                ImGui::GetColorU32(ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f)),
                2.0f
            );
        }

        ImGui::SetCursorPosX(isActive ? 10.0f : 6.0f);

        // Status dot
        {
            ImVec2 pos = ImGui::GetCursorScreenPos();
            ImDrawList* dl = ImGui::GetWindowDrawList();
            float dotRadius = 4.0f;
            ImVec2 center(pos.x + railWidth * 0.5f, pos.y + dotRadius);
            dl->AddCircleFilled(center, dotRadius, ImGui::GetColorU32(dotColor));

            // Glow effect for ready/active
            if (phase.status == PhaseStatus::Ready || isActive) {
                dl->AddCircle(center, dotRadius + 2.0f,
                    ImGui::GetColorU32(ImVec4(dotColor.x, dotColor.y, dotColor.z, 0.3f)), 0, 1.5f);
            }
            ImGui::Dummy(ImVec2(0, dotRadius * 2 + 4));
        }

        // Phase number + label button
        ImVec4 textCol = isActive
            ? ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f)
            : ImVec4(NeonColors::TextNormal[0], NeonColors::TextNormal[1], NeonColors::TextNormal[2], 1.0f);

        ImGui::PushStyleColor(ImGuiCol_Text, textCol);
        ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0, 0, 0, 0));
        ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4(0.15f, 0.15f, 0.20f, 1.0f));
        ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImVec4(0.00f, 0.40f, 0.44f, 1.0f));

        // Center the label
        float labelWidth = ImGui::CalcTextSize(phase.label).x;
        float btnWidth = railWidth - (isActive ? 10.0f : 6.0f);
        ImGui::SetCursorPosX(isActive ? 10.0f : 6.0f);

        if (ImGui::Button(phase.label, ImVec2(btnWidth, 0))) {
            m_activePhase = (WorkflowPhase)i;
            changed = true;
        }

        ImGui::PopStyleColor(4);

        // Count badge
        if (phase.count > 0) {
            char countStr[16];
            snprintf(countStr, sizeof(countStr), "%d", phase.count);
            ImVec4 badgeCol = ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f);
            ImGui::PushStyleColor(ImGuiCol_Text, badgeCol);
            float countWidth = ImGui::CalcTextSize(countStr).x;
            ImGui::SetCursorPosX((railWidth - countWidth) * 0.5f);
            ImGui::TextUnformatted(countStr);
            ImGui::PopStyleColor();
        }

        // Tooltip
        if (ImGui::IsItemHovered()) {
            ImGui::SetTooltip("%s", phase.tooltip);
        }

        ImGui::Spacing();
        if (i < (int)WorkflowPhase::COUNT - 1) {
            // Connecting line between phases
            ImVec2 pos = ImGui::GetCursorScreenPos();
            ImDrawList* dl = ImGui::GetWindowDrawList();
            float cx = pos.x + railWidth * 0.5f;
            dl->AddLine(ImVec2(cx, pos.y), ImVec2(cx, pos.y + 8),
                ImGui::GetColorU32(ImVec4(0.20f, 0.20f, 0.25f, 1.0f)), 1.0f);
            ImGui::Dummy(ImVec2(0, 10));
        }

        ImGui::PopID();
    }

    ImGui::EndChild();
    ImGui::PopStyleVar();
    ImGui::PopStyleColor();

    return changed;
}

void WorkflowRail::SetPhaseStatus(WorkflowPhase phase, PhaseStatus status) {
    m_phases[(size_t)phase].status = status;
}

void WorkflowRail::SetPhaseCount(WorkflowPhase phase, int count) {
    m_phases[(size_t)phase].count = count;
}

PhaseInfo& WorkflowRail::GetPhase(WorkflowPhase phase) {
    return m_phases[(size_t)phase];
}

} // namespace isimpa
