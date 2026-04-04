#include "commands/command_palette.h"
#include "app/theme.h"
#include <imgui.h>
#include <algorithm>
#include <cctype>

namespace isimpa {

CommandPalette::CommandPalette() {
    // Register default commands
    RegisterCommand("New Project",           "Ctrl+N",  []{});
    RegisterCommand("Open Project",          "Ctrl+O",  []{});
    RegisterCommand("Save Project",          "Ctrl+S",  []{});
    RegisterCommand("Import Geometry",       "",         []{});
    RegisterCommand("Export Scene",          "",         []{});
    RegisterCommand("Add Sound Source",      "",         []{});
    RegisterCommand("Add Punctual Receiver", "",         []{});
    RegisterCommand("Add Surface Receiver",  "",         []{});
    RegisterCommand("Create Receiver Grid",  "",         []{});
    RegisterCommand("Assign Material",       "",         []{});
    RegisterCommand("Generate Mesh",         "",         []{});
    RegisterCommand("Run SPPS Simulation",   "",         []{});
    RegisterCommand("Run TCR Simulation",    "",         []{});
    RegisterCommand("Toggle Wireframe",      "",         []{});
    RegisterCommand("Toggle Mesh Visibility", "",        []{});
    RegisterCommand("Camera: Top View",      "",         []{});
    RegisterCommand("Camera: Front View",    "",         []{});
    RegisterCommand("Camera: Right View",    "",         []{});
    RegisterCommand("Camera: Reset",         "",         []{});
    RegisterCommand("Undo",                  "Ctrl+Z",  []{});
    RegisterCommand("Redo",                  "Ctrl+Y",  []{});
    RegisterCommand("Preferences",           "",         []{});
    RegisterCommand("Show Demo Window",      "F1",       []{});

    FilterCommands();
}

void CommandPalette::RegisterCommand(const std::string& name, const std::string& shortcut,
                                      std::function<void()> action) {
    m_commands.push_back({name, shortcut, action});
}

void CommandPalette::Open() {
    m_isOpen = true;
    m_justOpened = true;
    m_searchBuf[0] = '\0';
    m_selectedIndex = 0;
    FilterCommands();
}

void CommandPalette::Close() {
    m_isOpen = false;
}

static bool FuzzyMatch(const std::string& text, const std::string& query) {
    if (query.empty()) return true;
    size_t qi = 0;
    for (size_t ti = 0; ti < text.size() && qi < query.size(); ti++) {
        if (std::tolower(text[ti]) == std::tolower(query[qi])) {
            qi++;
        }
    }
    return qi == query.size();
}

void CommandPalette::FilterCommands() {
    m_filtered.clear();
    std::string query(m_searchBuf);
    for (int i = 0; i < (int)m_commands.size(); i++) {
        if (FuzzyMatch(m_commands[i].name, query)) {
            m_filtered.push_back(i);
        }
    }
    if (m_selectedIndex >= (int)m_filtered.size()) {
        m_selectedIndex = 0;
    }
}

void CommandPalette::Draw() {
    if (!m_isOpen) return;

    // Center the palette at top of screen
    ImGuiViewport* vp = ImGui::GetMainViewport();
    float paletteWidth = 500.0f;
    ImGui::SetNextWindowPos(
        ImVec2(vp->WorkPos.x + (vp->WorkSize.x - paletteWidth) * 0.5f,
               vp->WorkPos.y + 60),
        ImGuiCond_Always);
    ImGui::SetNextWindowSize(ImVec2(paletteWidth, 0), ImGuiCond_Always);

    ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 8.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(12, 12));
    ImGui::PushStyleColor(ImGuiCol_WindowBg, ImVec4(0.08f, 0.08f, 0.11f, 0.98f));
    ImGui::PushStyleColor(ImGuiCol_Border, ImVec4(
        NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 0.5f));

    ImGuiWindowFlags flags = ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize |
                             ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoScrollbar |
                             ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoDocking;

    if (ImGui::Begin("##CommandPalette", &m_isOpen, flags)) {
        // Search input
        ImGui::PushStyleColor(ImGuiCol_FrameBg, ImVec4(0.12f, 0.12f, 0.16f, 1.0f));
        ImGui::PushItemWidth(-1);

        if (m_justOpened) {
            ImGui::SetKeyboardFocusHere();
            m_justOpened = false;
        }

        bool changed = ImGui::InputTextWithHint("##CmdSearch", "Type a command...",
            m_searchBuf, sizeof(m_searchBuf));
        if (changed) FilterCommands();

        ImGui::PopItemWidth();
        ImGui::PopStyleColor();

        // Handle keyboard navigation
        if (ImGui::IsKeyPressed(ImGuiKey_Escape)) {
            Close();
        }
        if (ImGui::IsKeyPressed(ImGuiKey_UpArrow) && m_selectedIndex > 0) {
            m_selectedIndex--;
        }
        if (ImGui::IsKeyPressed(ImGuiKey_DownArrow) && m_selectedIndex < (int)m_filtered.size() - 1) {
            m_selectedIndex++;
        }
        if (ImGui::IsKeyPressed(ImGuiKey_Enter) && !m_filtered.empty()) {
            auto& cmd = m_commands[m_filtered[m_selectedIndex]];
            if (cmd.action) cmd.action();
            Close();
        }

        ImGui::Spacing();
        ImGui::Separator();
        ImGui::Spacing();

        // Command list
        ImGui::BeginChild("##CmdList", ImVec2(0, 300));
        for (int i = 0; i < (int)m_filtered.size(); i++) {
            auto& cmd = m_commands[m_filtered[i]];
            bool isSelected = (i == m_selectedIndex);

            if (isSelected) {
                ImGui::PushStyleColor(ImGuiCol_Header,
                    ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 0.15f));
            }

            if (ImGui::Selectable(cmd.name.c_str(), isSelected)) {
                if (cmd.action) cmd.action();
                Close();
            }

            if (isSelected) {
                ImGui::PopStyleColor();
                if (ImGui::IsItemVisible()) {
                    ImGui::SetScrollHereY();
                }
            }

            // Shortcut text on the right
            if (!cmd.shortcut.empty()) {
                ImGui::SameLine(ImGui::GetContentRegionAvail().x - ImGui::CalcTextSize(cmd.shortcut.c_str()).x);
                ImGui::PushStyleColor(ImGuiCol_Text,
                    ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f));
                ImGui::TextUnformatted(cmd.shortcut.c_str());
                ImGui::PopStyleColor();
            }
        }
        ImGui::EndChild();
    }
    ImGui::End();

    ImGui::PopStyleColor(2);
    ImGui::PopStyleVar(2);

    // Close if clicked outside
    if (m_isOpen && ImGui::IsMouseClicked(0) && !ImGui::IsWindowHovered(ImGuiHoveredFlags_AnyWindow)) {
        Close();
    }
}

// Free function wrapper for App to call
void DrawCommandPalettePopup(CommandPalette& palette) {
    palette.Draw();
}

} // namespace isimpa
