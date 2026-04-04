#include "app/theme.h"
#include <imgui.h>

namespace isimpa {

void ApplyDarkNeonTheme() {
    ImGuiStyle& style = ImGui::GetStyle();
    ImVec4* colors = style.Colors;

    // ── Rounding & spacing ──────────────────────────────────────────────────
    style.WindowRounding    = 4.0f;
    style.ChildRounding     = 4.0f;
    style.FrameRounding     = 3.0f;
    style.PopupRounding     = 4.0f;
    style.ScrollbarRounding = 6.0f;
    style.GrabRounding      = 3.0f;
    style.TabRounding       = 4.0f;

    style.WindowPadding     = ImVec2(10, 10);
    style.FramePadding      = ImVec2(8, 5);
    style.ItemSpacing       = ImVec2(8, 6);
    style.ItemInnerSpacing  = ImVec2(6, 4);
    style.IndentSpacing     = 20.0f;
    style.ScrollbarSize     = 12.0f;
    style.GrabMinSize       = 8.0f;

    style.WindowBorderSize  = 1.0f;
    style.ChildBorderSize   = 1.0f;
    style.PopupBorderSize   = 1.0f;
    style.FrameBorderSize   = 0.0f;
    style.TabBorderSize     = 0.0f;

    // ── Colors ──────────────────────────────────────────────────────────────

    // Window backgrounds
    colors[ImGuiCol_WindowBg]          = ImVec4(0.09f, 0.09f, 0.12f, 1.00f);
    colors[ImGuiCol_ChildBg]           = ImVec4(0.09f, 0.09f, 0.12f, 0.00f);
    colors[ImGuiCol_PopupBg]           = ImVec4(0.08f, 0.08f, 0.11f, 0.96f);

    // Borders
    colors[ImGuiCol_Border]            = ImVec4(0.20f, 0.20f, 0.25f, 1.00f);
    colors[ImGuiCol_BorderShadow]      = ImVec4(0.00f, 0.00f, 0.00f, 0.00f);

    // Frames (input fields, checkboxes)
    colors[ImGuiCol_FrameBg]           = ImVec4(0.12f, 0.12f, 0.16f, 1.00f);
    colors[ImGuiCol_FrameBgHovered]    = ImVec4(0.16f, 0.16f, 0.21f, 1.00f);
    colors[ImGuiCol_FrameBgActive]     = ImVec4(0.60f, 0.08f, 0.12f, 0.40f);

    // Title bar
    colors[ImGuiCol_TitleBg]           = ImVec4(0.06f, 0.06f, 0.08f, 1.00f);
    colors[ImGuiCol_TitleBgActive]     = ImVec4(0.06f, 0.06f, 0.08f, 1.00f);
    colors[ImGuiCol_TitleBgCollapsed]  = ImVec4(0.06f, 0.06f, 0.08f, 0.60f);

    // Menu bar
    colors[ImGuiCol_MenuBarBg]         = ImVec4(0.08f, 0.08f, 0.10f, 1.00f);

    // Scrollbar
    colors[ImGuiCol_ScrollbarBg]       = ImVec4(0.06f, 0.06f, 0.08f, 0.60f);
    colors[ImGuiCol_ScrollbarGrab]     = ImVec4(0.20f, 0.20f, 0.25f, 1.00f);
    colors[ImGuiCol_ScrollbarGrabHovered] = ImVec4(0.28f, 0.28f, 0.34f, 1.00f);
    colors[ImGuiCol_ScrollbarGrabActive]  = ImVec4(0.75f, 0.10f, 0.15f, 1.00f);

    // Buttons
    colors[ImGuiCol_Button]            = ImVec4(0.14f, 0.14f, 0.18f, 1.00f);
    colors[ImGuiCol_ButtonHovered]     = ImVec4(0.60f, 0.08f, 0.12f, 0.70f);
    colors[ImGuiCol_ButtonActive]      = ImVec4(0.95f, 0.10f, 0.15f, 0.80f);

    // Headers (collapsing headers, tree nodes, selectables, menu items)
    colors[ImGuiCol_Header]            = ImVec4(0.14f, 0.14f, 0.18f, 1.00f);
    colors[ImGuiCol_HeaderHovered]     = ImVec4(0.60f, 0.08f, 0.12f, 0.50f);
    colors[ImGuiCol_HeaderActive]      = ImVec4(0.75f, 0.10f, 0.15f, 0.60f);

    // Separator
    colors[ImGuiCol_Separator]         = ImVec4(0.20f, 0.20f, 0.25f, 1.00f);
    colors[ImGuiCol_SeparatorHovered]  = ImVec4(0.75f, 0.10f, 0.15f, 0.70f);
    colors[ImGuiCol_SeparatorActive]   = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);

    // Resize grip
    colors[ImGuiCol_ResizeGrip]        = ImVec4(0.20f, 0.20f, 0.25f, 0.50f);
    colors[ImGuiCol_ResizeGripHovered] = ImVec4(0.75f, 0.10f, 0.15f, 0.70f);
    colors[ImGuiCol_ResizeGripActive]  = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);

    // Tabs
    colors[ImGuiCol_Tab]              = ImVec4(0.10f, 0.10f, 0.13f, 1.00f);
    colors[ImGuiCol_TabHovered]       = ImVec4(0.60f, 0.08f, 0.12f, 0.60f);
    colors[ImGuiCol_TabSelected]      = ImVec4(0.44f, 0.06f, 0.10f, 1.00f);
    colors[ImGuiCol_TabDimmed]        = ImVec4(0.08f, 0.08f, 0.10f, 1.00f);
    colors[ImGuiCol_TabDimmedSelected] = ImVec4(0.12f, 0.12f, 0.16f, 1.00f);

    // Docking
    colors[ImGuiCol_DockingPreview]   = ImVec4(0.95f, 0.10f, 0.15f, 0.30f);
    colors[ImGuiCol_DockingEmptyBg]   = ImVec4(0.06f, 0.06f, 0.08f, 1.00f);

    // Checkmarks, sliders
    colors[ImGuiCol_CheckMark]        = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);
    colors[ImGuiCol_SliderGrab]       = ImVec4(0.75f, 0.10f, 0.15f, 1.00f);
    colors[ImGuiCol_SliderGrabActive] = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);

    // Text
    colors[ImGuiCol_Text]            = ImVec4(0.92f, 0.93f, 0.95f, 1.00f);
    colors[ImGuiCol_TextDisabled]    = ImVec4(0.45f, 0.46f, 0.52f, 1.00f);

    // Plot
    colors[ImGuiCol_PlotLines]       = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);
    colors[ImGuiCol_PlotLinesHovered]= ImVec4(0.20f, 0.92f, 0.40f, 1.00f);
    colors[ImGuiCol_PlotHistogram]   = ImVec4(0.75f, 0.10f, 0.15f, 1.00f);
    colors[ImGuiCol_PlotHistogramHovered] = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);

    // Table
    colors[ImGuiCol_TableHeaderBg]    = ImVec4(0.10f, 0.10f, 0.13f, 1.00f);
    colors[ImGuiCol_TableBorderStrong]= ImVec4(0.20f, 0.20f, 0.25f, 1.00f);
    colors[ImGuiCol_TableBorderLight] = ImVec4(0.15f, 0.15f, 0.19f, 1.00f);
    colors[ImGuiCol_TableRowBg]       = ImVec4(0.00f, 0.00f, 0.00f, 0.00f);
    colors[ImGuiCol_TableRowBgAlt]    = ImVec4(0.10f, 0.10f, 0.13f, 0.40f);

    // Nav highlight
    colors[ImGuiCol_NavHighlight]     = ImVec4(0.95f, 0.10f, 0.15f, 1.00f);

    // Modal dim
    colors[ImGuiCol_ModalWindowDimBg] = ImVec4(0.00f, 0.00f, 0.00f, 0.60f);

    // Drag-drop target
    colors[ImGuiCol_DragDropTarget]   = ImVec4(0.95f, 0.10f, 0.15f, 0.90f);

    // Text selection
    colors[ImGuiCol_TextSelectedBg]   = ImVec4(0.60f, 0.08f, 0.12f, 0.40f);
}

} // namespace isimpa
