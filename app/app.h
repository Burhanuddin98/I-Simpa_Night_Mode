#pragma once

#include "app/workflow_rail.h"
#include "commands/command_palette.h"
#include <glm/glm.hpp>
#include <string>
#include <vector>

struct GLFWwindow;

namespace isimpa {

// ─── Automation command (queued from command-line args) ─────────────────────
struct AutoCmd {
    enum Type { LoadScene, RunSPPS, RunTCR, LoadResults, BuildHeatmap, FocusCamera,
                AddSource, AddReceiver, AddSurfaceReceiver, Wait, SkipSplash, Quit,
                PlayParticles } type;
    std::string arg;
    float delay = 0; // seconds to wait before executing
};

class App {
public:
    App();
    ~App();

    bool Init();
    void Run();
    void Shutdown();

    // Queue automation commands from command-line args
    void QueueAutomation(const std::vector<std::string>& args);

private:
    void BeginFrame();
    void EndFrame();
    void DrawMainDockspace();
    void DrawMenuBar();
    void DrawStatusBar();
    void DrawSimulatePanel();
    void UpdateWorkflowStatus();

    // Window
    GLFWwindow* m_window = nullptr;
    int m_windowWidth  = 1600;
    int m_windowHeight = 1000;

    // Subsystems
    WorkflowRail   m_workflowRail;
    CommandPalette  m_commandPalette;

    // State
    bool m_running    = true;
    bool m_showDemo   = false;
    bool m_showNewRoom = false;
    bool m_showAbout  = false;
    bool m_dirty      = false; // unsaved changes
    bool m_splashDone = false;
    float m_splashTimer = 0;
    float m_newRoomDims[3] = {6.0f, 10.0f, 3.0f}; // width, length, height

    // Measurement tool
    bool  m_measActive = false;
    glm::vec3 m_measPointA{0}, m_measPointB{0};
    int   m_measState = 0; // 0=idle, 1=waiting for A, 2=waiting for B, 3=showing

    // Clipping plane
    bool  m_clipEnabled = false;
    float m_clipHeight = 1.2f; // Y height for horizontal clip
    int   m_clipAxis = 1; // 0=X, 1=Y, 2=Z

    // Automation
    std::vector<AutoCmd> m_autoCmds;
    int m_autoCmdIdx = 0;
    float m_autoTimer = 0;
    bool m_autoMode = false;
    void ProcessAutomation();

    // Recent files
    std::vector<std::string> m_recentFiles;
    void LoadRecentFiles();
    void SaveRecentFiles();
    void AddRecentFile(const std::string& path);

    void UpdateWindowTitle();

    // Panel visibility
    bool m_showOutliner   = true;
    bool m_showProperties = true;
    bool m_showMaterials  = true;
    bool m_showConsole    = true;
    bool m_showResults    = true;
    bool m_showSimulate   = true;
    bool m_showViewport   = true;

    // Viewport clear color
    float m_clearColor[4] = {0.06f, 0.06f, 0.08f, 1.00f};
};

} // namespace isimpa
