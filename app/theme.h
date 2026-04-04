#pragma once

namespace isimpa {

// Neon accent color palette — NEON RED theme
struct NeonColors {
    // Primary accents (was cyan, now neon red)
    static constexpr float Cyan[4]      = {0.95f, 0.10f, 0.15f, 1.00f};  // neon red
    static constexpr float CyanDim[4]   = {0.60f, 0.08f, 0.12f, 1.00f};
    static constexpr float CyanGlow[4]  = {0.95f, 0.10f, 0.15f, 0.25f};

    // Status colors
    static constexpr float Green[4]     = {0.20f, 0.92f, 0.40f, 1.00f};
    static constexpr float Yellow[4]    = {1.00f, 0.85f, 0.20f, 1.00f};
    static constexpr float Red[4]       = {0.95f, 0.22f, 0.30f, 1.00f};
    static constexpr float Orange[4]    = {1.00f, 0.55f, 0.10f, 1.00f};

    // Surfaces
    static constexpr float BgDarkest[4] = {0.06f, 0.06f, 0.08f, 1.00f};
    static constexpr float BgDark[4]    = {0.09f, 0.09f, 0.12f, 1.00f};
    static constexpr float BgMid[4]     = {0.12f, 0.12f, 0.16f, 1.00f};
    static constexpr float BgLight[4]   = {0.16f, 0.16f, 0.21f, 1.00f};
    static constexpr float BgHover[4]   = {0.20f, 0.20f, 0.26f, 1.00f};

    // Text
    static constexpr float TextBright[4] = {0.92f, 0.93f, 0.95f, 1.00f};
    static constexpr float TextNormal[4] = {0.72f, 0.73f, 0.78f, 1.00f};
    static constexpr float TextDim[4]    = {0.45f, 0.46f, 0.52f, 1.00f};

    // Borders
    static constexpr float Border[4]     = {0.20f, 0.20f, 0.25f, 1.00f};
    static constexpr float BorderGlow[4] = {0.60f, 0.08f, 0.12f, 0.50f};
};

// Apply the dark neon theme to ImGui
void ApplyDarkNeonTheme();

} // namespace isimpa
