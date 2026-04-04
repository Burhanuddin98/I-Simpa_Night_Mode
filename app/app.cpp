#include "app/app.h"
#include "app/theme.h"
#include "app/selection.h"
#include "project/project.h"
#include "project/solver.h"
#include "project/result_parser.h"
#include "viewport/viewport.h"

#include <glad/glad.h>
#include <GLFW/glfw3.h>
#include <imgui.h>
#include <imgui_impl_glfw.h>
#include <imgui_impl_opengl3.h>
#include <implot.h>

#include <cstdio>
#include <cstdarg>
#include <cstring>
#include <thread>
#include <atomic>
#include <mutex>
#include <filesystem>
#include <fstream>
#include <algorithm>

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <commdlg.h>
#include <shlobj.h>
#include <dwmapi.h>
#include <gdiplus.h>
#pragma comment(lib, "dwmapi.lib")
#pragma comment(lib, "gdiplus.lib")
#define GLFW_EXPOSE_NATIVE_WIN32
#include <GLFW/glfw3native.h>
#endif

namespace fs = std::filesystem;

// Panel forward declarations - each panel draws itself
namespace isimpa {
    void DrawOutliner();
    void DrawProperties();
    void DrawConsole();
    void DrawResults();
    void DrawMaterials();
    void DrawViewport(float width, float height);
    void DrawCommandPalettePopup(CommandPalette& palette);

    // Console logging (defined in console.cpp, exposed here)
    void ConsoleLog(const std::string& msg, int level = 0);
}

namespace isimpa {

#ifdef _WIN32
static HWND s_mainHwnd = nullptr; // for modal file dialogs
#endif

// ─── File Dialogs ───────────────────────────────────────────────────────────

static std::string OpenFileDialog(const char* filter, const char* title) {
#ifdef _WIN32
    char filename[MAX_PATH] = {};
    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.hwndOwner = s_mainHwnd;
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrTitle = title;
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR;
    if (GetOpenFileNameA(&ofn)) return std::string(filename);
#endif
    return "";
}

static std::string SaveFileDialog(const char* filter, const char* title, const char* defaultExt) {
#ifdef _WIN32
    char filename[MAX_PATH] = {};
    OPENFILENAMEA ofn = {};
    ofn.lStructSize = sizeof(ofn);
    ofn.hwndOwner = s_mainHwnd;
    ofn.lpstrFilter = filter;
    ofn.lpstrFile = filename;
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrTitle = title;
    ofn.lpstrDefExt = defaultExt;
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_NOCHANGEDIR;
    if (GetSaveFileNameA(&ofn)) return std::string(filename);
#endif
    return "";
}

static std::string BrowseFolderDialog(const char* title) {
#ifdef _WIN32
    char path[MAX_PATH] = {};
    BROWSEINFOA bi = {};
    bi.hwndOwner = s_mainHwnd;
    bi.lpszTitle = title;
    bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
    LPITEMIDLIST pidl = SHBrowseForFolderA(&bi);
    if (pidl) {
        SHGetPathFromIDListA(pidl, path);
        CoTaskMemFree(pidl);
        return std::string(path);
    }
#endif
    return "";
}

static void GlfwErrorCallback(int error, const char* description) {
    fprintf(stderr, "[GLFW Error %d] %s\n", error, description);
}

// Smart project loader: detects .proj (original ZIP) vs .isimpa (new XML)
static bool SmartLoadProject(Project& proj, const std::string& path) {
    std::string ext = fs::path(path).extension().string();
    std::transform(ext.begin(), ext.end(), ext.begin(), ::tolower);
    if (ext == ".proj") {
        return LoadOriginalProject(proj, path);
    }
    return LoadProject(proj, path);
}

// ─── Simulation state (for async solver) ────────────────────────────────────

// ─── Exe directory (resolved once at startup) ───────────────────────────────
static std::string s_exeDir;
static void InitExeDir() {
#ifdef _WIN32
    char buf[MAX_PATH] = {};
    GetModuleFileNameA(nullptr, buf, MAX_PATH);
    s_exeDir = std::string(buf);
    auto p = s_exeDir.find_last_of("\\/");
    if (p != std::string::npos) s_exeDir = s_exeDir.substr(0, p + 1);
#endif
}

// ─── Error popup state ──────────────────────────────────────────────────────
static std::string s_errorMsg;
static bool s_showError = false;

void ShowErrorPopup(const std::string& msg) {
    s_errorMsg = msg;
    s_showError = true;
}

static std::atomic<bool> s_simRunning{false};
static std::thread s_simThread;
static std::mutex s_simMsgMutex;
static std::string s_simStatusMsg;
static std::atomic<float> s_simPercent{0};
static std::mutex s_simResultMutex;
static std::string s_simResultDir; // thread-safe result dir from solver thread
static std::atomic<bool> s_simAutoLoad{false}; // trigger auto-load of results

// Thread-safe helpers for status message
static void SetSimStatus(const std::string& msg) {
    std::lock_guard<std::mutex> lock(s_simMsgMutex);
    s_simStatusMsg = msg;
}
static std::string GetSimStatus() {
    std::lock_guard<std::mutex> lock(s_simMsgMutex);
    return s_simStatusMsg;
}

static void JoinSimThread() {
    if (s_simThread.joinable()) {
        s_simThread.join();
    }
}

static void RunSimulationAsync(const std::string& solver) {
    if (s_simRunning.load()) return;

    // Join any previous thread before starting a new one
    JoinSimThread();

    s_simRunning.store(true);
    s_simPercent.store(0);
    SetSimStatus("Starting...");

    // Create working directory (absolute path next to exe)
    std::string workDir = s_exeDir + "sim_output/" + solver;
    fs::create_directories(workDir);

    // Snapshot project data for the solver thread to avoid race conditions
    Project projCopy = GetProject();

    s_simThread = std::thread([solver, workDir, projCopy]() mutable {
        bool ok = RunFullSimulation(solver, workDir, projCopy, [](float pct, const std::string& msg) {
            s_simPercent.store(pct);
            SetSimStatus(msg);
        });
        s_simPercent.store(100);
        if (ok) {
            SetSimStatus("DONE - Simulation complete! Click Load Results.");
            ConsoleLog("[Solver] Simulation completed successfully. Results in: " + workDir, 0);
            ConsoleLog("[Solver] Click 'Load Results' in the Results panel to view.", 0);
            // Store result dir thread-safely; main thread picks it up each frame
            {
                std::lock_guard<std::mutex> lock(s_simResultMutex);
                s_simResultDir = workDir;
            }
            s_simAutoLoad.store(true);
        } else {
            SetSimStatus("FAILED - Check Console for errors.");
            ConsoleLog("[Solver] Simulation FAILED. Check errors below:", 2);
            for (auto& err : projCopy.simProgress.errors)
                ConsoleLog("  Error: " + err, 2);
            for (auto& warn : projCopy.simProgress.warnings)
                ConsoleLog("  Warning: " + warn, 1);
        }
        // Log all solver output to console
        for (auto& line : projCopy.simProgress.log)
            ConsoleLog("[Solver] " + line, 0);
        s_simRunning.store(false);
    });
}

// ─── App Implementation ─────────────────────────────────────────────────────

App::App() = default;
App::~App() { Shutdown(); }

bool App::Init() {
    InitExeDir();
    glfwSetErrorCallback(GlfwErrorCallback);
    if (!glfwInit()) return false;

    // Request OpenGL 4.6 core
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 6);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    glfwWindowHint(GLFW_SAMPLES, 4); // MSAA

    m_window = glfwCreateWindow(m_windowWidth, m_windowHeight, "I-Simpa // Dark Neon", nullptr, nullptr);
    if (!m_window) {
        // Fallback to 4.3 if 4.6 unavailable
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
        m_window = glfwCreateWindow(m_windowWidth, m_windowHeight, "I-Simpa // Dark Neon", nullptr, nullptr);
        if (!m_window) {
            glfwTerminate();
            return false;
        }
    }

    glfwMakeContextCurrent(m_window);
    glfwSwapInterval(1); // VSync

    // ── Load restyled neon I-Simpa logo ────────────────────────────────────
    {
#ifdef _WIN32
        // Load the pre-rendered neon logo PNG via GDI+
        ULONG_PTR gdiplusToken;
        Gdiplus::GdiplusStartupInput gdiplusStartupInput;
        Gdiplus::GdiplusStartup(&gdiplusToken, &gdiplusStartupInput, nullptr);

        // Try logo next to exe, then in parent directory
        std::wstring exeDirW(s_exeDir.begin(), s_exeDir.end());
        const std::wstring logoCandidates[] = {
            exeDirW + L"isimpa_neon_logo.png",
            L"isimpa_neon_logo.png",
            exeDirW + L"../isimpa_neon_logo.png",
        };
        const wchar_t* logoPaths[3];
        for (int i = 0; i < 3; i++) logoPaths[i] = logoCandidates[i].c_str();

        for (auto& logoPath : logoPaths) {
            Gdiplus::Bitmap* bmp = new Gdiplus::Bitmap(logoPath);
            if (bmp && bmp->GetLastStatus() == Gdiplus::Ok) {
                int w = (int)bmp->GetWidth();
                int h = (int)bmp->GetHeight();
                std::vector<unsigned char> pixels(w * h * 4);

                for (int y = 0; y < h; y++) {
                    for (int x = 0; x < w; x++) {
                        Gdiplus::Color col;
                        bmp->GetPixel(x, y, &col);
                        int idx = (y * w + x) * 4;
                        pixels[idx+0] = col.GetR();
                        pixels[idx+1] = col.GetG();
                        pixels[idx+2] = col.GetB();
                        pixels[idx+3] = col.GetA();
                    }
                }

                GLFWimage icon;
                icon.width = w;
                icon.height = h;
                icon.pixels = pixels.data();
                glfwSetWindowIcon(m_window, 1, &icon);
                printf("[Icon] Loaded neon logo: %dx%d\n", w, h);
                delete bmp;
                break;
            }
            delete bmp;
        }
        Gdiplus::GdiplusShutdown(gdiplusToken);
#endif
    }

    // ── Dark title bar (Windows 11) ─────────────────────────────────────────
#ifdef _WIN32
    {
        HWND hwnd = glfwGetWin32Window(m_window);
        s_mainHwnd = hwnd;
        // DWMWA_USE_IMMERSIVE_DARK_MODE = 20
        BOOL useDarkMode = TRUE;
        DwmSetWindowAttribute(hwnd, 20, &useDarkMode, sizeof(useDarkMode));

        // Also try DWMWA_CAPTION_COLOR = 35 (Windows 11 22H2+)
        COLORREF captionColor = RGB(15, 15, 20); // near-black
        DwmSetWindowAttribute(hwnd, 35, &captionColor, sizeof(captionColor));

        // Border color
        COLORREF borderColor = RGB(200, 30, 40); // neon red border
        DwmSetWindowAttribute(hwnd, 34, &borderColor, sizeof(borderColor));
    }
#endif

    if (!gladLoadGLLoader((GLADloadproc)glfwGetProcAddress)) {
        fprintf(stderr, "Failed to initialize glad\n");
        return false;
    }

    printf("OpenGL %s | %s | %s\n", glGetString(GL_VERSION),
           glGetString(GL_RENDERER), glGetString(GL_VENDOR));

    glEnable(GL_MULTISAMPLE);

    // ── ImGui setup ─────────────────────────────────────────────────────────
    IMGUI_CHECKVERSION();
    ImGui::CreateContext();
    ImPlot::CreateContext();

    ImGuiIO& io = ImGui::GetIO();
    io.ConfigFlags |= ImGuiConfigFlags_DockingEnable;
    // ViewportsEnable disabled — causes layout issues on some systems
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
    io.IniFilename = "imgui_isimpa.ini";

    // Load system fonts (Segoe UI for UI, Consolas for console)
    ImFontConfig fontCfg;
    fontCfg.OversampleH = 2;
    fontCfg.OversampleV = 2;

    // Search for UI font in multiple locations
    const char* uiFontCandidates[] = {
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    };
    bool uiLoaded = false;
    for (auto& path : uiFontCandidates) {
        if (std::filesystem::exists(path)) {
            io.Fonts->AddFontFromFileTTF(path, 15.0f, &fontCfg);
            uiLoaded = true;
            break;
        }
    }
    if (!uiLoaded) io.Fonts->AddFontDefault();

    // Search for mono font
    const char* monoFontCandidates[] = {
        "C:/Windows/Fonts/consola.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
    };
    for (auto& path : monoFontCandidates) {
        if (std::filesystem::exists(path)) {
            io.Fonts->AddFontFromFileTTF(path, 14.0f, &fontCfg);
            break;
        }
    }

    ApplyDarkNeonTheme();

    ImGui_ImplGlfw_InitForOpenGL(m_window, true);
    ImGui_ImplOpenGL3_Init("#version 430");

    LoadRecentFiles();

    // ── Wire command palette to real actions ─────────────────────────────────
    m_commandPalette = CommandPalette(); // reset defaults
    m_commandPalette.RegisterCommand("New Room", "Ctrl+N", [this]{ m_showNewRoom = true; });
    m_commandPalette.RegisterCommand("Open Project", "Ctrl+O", [this]{
        std::string path = OpenFileDialog(
            "I-Simpa Project (*.isimpa;*.proj)\0*.isimpa;*.proj\0Original I-Simpa (*.proj)\0*.proj\0All Files\0*.*\0",
            "Open Project");
        if (!path.empty()) {
            Project& proj = GetProject();
            if (SmartLoadProject(proj, path)) {
                proj.projectPath = path;
                ViewportLoadModel(proj.model);
                ConsoleLog("[Project] Loaded: " + path);
            } else {
                ConsoleLog("[Project] Failed to load: " + path, 2);
            }
        }
    });
    m_commandPalette.RegisterCommand("Save Project", "Ctrl+S", []{
        Project& proj = GetProject();
        if (proj.projectPath.empty()) {
            std::string path = SaveFileDialog(
                "I-Simpa Project (*.isimpa)\0*.isimpa\0", "Save Project", "isimpa");
            if (path.empty()) return;
            proj.projectPath = path;
        }
        if (SaveProject(proj, proj.projectPath)) {
            ConsoleLog("[Project] Saved: " + proj.projectPath);
        } else {
            ConsoleLog("[Project] Failed to save: " + proj.projectPath, 2);
        }
    });
    m_commandPalette.RegisterCommand("Import Scene (.ply)", "", [this]{
        std::string path = OpenFileDialog(
            "PLY Files (*.ply)\0*.ply\0All Files\0*.*\0", "Import 3D Scene");
        if (!path.empty()) {
            GetProject().LoadScenePLY(path);
            ConsoleLog("[Scene] Imported: " + path);
        }
    });
    m_commandPalette.RegisterCommand("Add Sound Source", "", []{
        GetUndoManager().SaveState(GetProject(), "Add Source");
        auto& src = GetProject().AddSource();
        ConsoleLog("[Source] Added: " + src.name);
    });
    m_commandPalette.RegisterCommand("Add Punctual Receiver", "", []{
        GetUndoManager().SaveState(GetProject(), "Add Receiver");
        auto& rcv = GetProject().AddReceiver();
        ConsoleLog("[Receiver] Added: " + rcv.name);
    });
    m_commandPalette.RegisterCommand("Add Surface Receiver", "", []{
        GetUndoManager().SaveState(GetProject(), "Add Surface Receiver");
        auto& sr = GetProject().AddSurfaceReceiver();
        ConsoleLog("[Surface Receiver] Added: " + sr.name);
    });
    m_commandPalette.RegisterCommand("Run SPPS Simulation", "", []{
        RunSimulationAsync("spps");
    });
    m_commandPalette.RegisterCommand("Run TCR Simulation", "", []{
        RunSimulationAsync("tcr");
    });
    m_commandPalette.RegisterCommand("Generate Mesh", "", []{
        if (!GetProject().HasGeometry()) {
            ConsoleLog("[Mesh] No geometry loaded", 2);
            return;
        }
        ConsoleLog("[Mesh] Starting mesh generation...");
        std::string workDir = s_exeDir + "sim_output/mesh";
        fs::create_directories(workDir);
        bool ok = RunMeshGeneration(workDir, GetProject(), [](float pct, const std::string& msg) {
            s_simPercent.store(pct);
            SetSimStatus(msg);
        });
        ConsoleLog(ok ? "[Mesh] Generation complete" : "[Mesh] Generation failed", ok ? 0 : 2);
    });
    m_commandPalette.RegisterCommand("Import Scene (.obj)", "", [this]{
        std::string path = OpenFileDialog(
            "Wavefront OBJ (*.obj)\0*.obj\0All Files\0*.*\0", "Import OBJ");
        if (!path.empty()) {
            SceneModel model;
            if (LoadOBJ(path, model)) {
                GetProject().model = model;
                ViewportLoadModel(model);
                m_dirty = true;
                ConsoleLog("[Scene] Imported OBJ: " + path);
            }
        }
    });
    m_commandPalette.RegisterCommand("Import Scene (.3ds)", "", [this]{
        std::string path = OpenFileDialog(
            "3D Studio (*.3ds)\0*.3ds\0All Files\0*.*\0", "Import 3DS");
        if (!path.empty()) {
            SceneModel model;
            if (Load3DS(path, model)) {
                GetProject().model = model;
                ViewportLoadModel(model);
                ConsoleLog("[Scene] Imported 3DS: " + path);
            }
        }
    });
    m_commandPalette.RegisterCommand("Import Scene (.stl)", "", []{
        std::string path = OpenFileDialog(
            "STL Files (*.stl)\0*.stl\0All Files\0*.*\0", "Import STL");
        if (!path.empty()) {
            SceneModel model;
            if (LoadSTL(path, model)) {
                GetProject().model = model;
                ViewportLoadModel(model);
                ConsoleLog("[Scene] Imported STL: " + path);
            }
        }
    });
    m_commandPalette.RegisterCommand("Add Fitting Zone", "", []{
        GetUndoManager().SaveState(GetProject(), "Add Fitting Zone");
        auto& enc = GetProject().AddEncumbrance();
        ConsoleLog("[Fitting] Added: " + enc.name);
    });
    m_commandPalette.RegisterCommand("Add Volume Definition", "", []{
        auto& vol = GetProject().AddVolume();
        ConsoleLog("[Volume] Added: " + vol.name);
    });
    m_commandPalette.RegisterCommand("Merge Selected Groups", "", []{
        Selection& sel = GetSelection();
        Project& proj = GetProject();
        if (sel.selectedGroups.size() < 2) {
            ConsoleLog("[Scene] Select 2+ groups to merge (Shift+Click)", 1);
            return;
        }
        GetUndoManager().SaveState(proj, "Merge Groups");
        // Collect all faces from selected groups into the first group
        int firstIdx = *sel.selectedGroups.begin();
        auto& target = proj.model.groups[firstIdx];
        std::string mergedNames = target.name;
        std::vector<int> toRemove;
        for (int gi : sel.selectedGroups) {
            if (gi == firstIdx) continue;
            auto& src = proj.model.groups[gi];
            mergedNames += " + " + src.name;
            target.faces.insert(target.faces.end(), src.faces.begin(), src.faces.end());
            toRemove.push_back(gi);
        }
        // Remove merged groups (in reverse order to preserve indices)
        std::sort(toRemove.rbegin(), toRemove.rend());
        for (int gi : toRemove) {
            proj.model.groups.erase(proj.model.groups.begin() + gi);
            // Fix group-material map
            std::map<int, int> newMap;
            for (auto& [k, v] : proj.groupMaterialMap) {
                int nk = (k > gi) ? k - 1 : k;
                if (k != gi) newMap[nk] = v;
            }
            proj.groupMaterialMap = newMap;
        }
        target.name = mergedNames;
        proj.model.ComputeGroupArea(firstIdx);
        ViewportLoadModel(proj.model);
        sel.Clear();
        ConsoleLog("[Scene] Merged " + std::to_string(toRemove.size() + 1) + " groups into: " + mergedNames);
    });
    m_commandPalette.RegisterCommand("Delete Selected Group", "", []{
        Selection& sel = GetSelection();
        Project& proj = GetProject();
        if (sel.group < 0 || sel.group >= (int)proj.model.groups.size()) {
            ConsoleLog("[Scene] No group selected", 1);
            return;
        }
        GetUndoManager().SaveState(proj, "Delete Group");
        std::string name = proj.model.groups[sel.group].name;
        proj.model.groups.erase(proj.model.groups.begin() + sel.group);
        // Fix group-material map
        std::map<int, int> newMap;
        for (auto& [k, v] : proj.groupMaterialMap) {
            if (k == sel.group) continue;
            int nk = (k > sel.group) ? k - 1 : k;
            newMap[nk] = v;
        }
        proj.groupMaterialMap = newMap;
        ViewportLoadModel(proj.model);
        sel.Clear();
        ConsoleLog("[Scene] Deleted group: " + name);
    });
    m_commandPalette.RegisterCommand("Flip Group Normals", "", []{
        Selection& sel = GetSelection();
        Project& proj = GetProject();
        if (sel.group < 0) { ConsoleLog("[Scene] No group selected", 1); return; }
        GetUndoManager().SaveState(proj, "Flip Normals");
        auto& grp = proj.model.groups[sel.group];
        for (auto& face : grp.faces) {
            std::swap(face.v[1], face.v[2]);
            face.normal = -face.normal;
        }
        ViewportRefreshGPU();
        ConsoleLog("[Scene] Flipped normals on: " + grp.name);
    });
    m_commandPalette.RegisterCommand("Translate Selected Group", "", []{
        ConsoleLog("[Scene] Use Properties panel DragFloat3 to translate groups", 0);
    });
    m_commandPalette.RegisterCommand("Import Material Library (.xml)", "", []{
        std::string path = OpenFileDialog(
            "Material Database (*.xml)\0*.xml\0I-Simpa AppConst (*.xml)\0*.xml\0All Files\0*.*\0",
            "Import Material Library");
        if (!path.empty()) {
            auto& mats = GetProject().materials;
            size_t before = mats.size();
            // Try our format first, then appconst format
            if (!ImportMaterials(mats, path)) {
                ImportMatlibFromAppConst(mats, path);
            }
            ConsoleLog("[Materials] Imported " + std::to_string(mats.size() - before) + " materials from: " + path);
        }
    });
    m_commandPalette.RegisterCommand("Focus Camera on Model", "", []{
        ViewportFocusModel();
    });
    m_commandPalette.RegisterCommand("Toggle Wireframe", "", []{
        ViewportToggleWireframe();
    });
    m_commandPalette.RegisterCommand("Toggle Mesh Visibility", "", []{
        ViewportToggleFaces();
    });
    m_commandPalette.RegisterCommand("Camera: Top View", "", []{ ViewportCameraTop(); });
    m_commandPalette.RegisterCommand("Camera: Front View", "", []{ ViewportCameraFront(); });
    m_commandPalette.RegisterCommand("Camera: Right View", "", []{ ViewportCameraRight(); });
    m_commandPalette.RegisterCommand("Camera: Reset", "", []{ ViewportCameraReset(); });
    m_commandPalette.RegisterCommand("Save Screenshot", "", []{
        std::string path = SaveFileDialog(
            "TGA Image (*.tga)\0*.tga\0", "Save Screenshot", "tga");
        if (!path.empty()) {
            ViewportSaveScreenshot(path);
            ConsoleLog("[Screenshot] Saved: " + path);
        }
    });
    m_commandPalette.RegisterCommand("Measure Distance", "", []{
        g_viewportMeasureMode = true;
        g_viewportMeasureState = 1;
        ConsoleLog("[Measure] Click first point in viewport...", 0);
    });
    m_commandPalette.RegisterCommand("Exit", "", [this]{ m_running = false; });

    return true;
}

// ─── Automation log file (WIN32 apps have no console) ───────────────────────
static FILE* s_autoLog = nullptr;
static void AutoLog(const char* fmt, ...) {
    if (!s_autoLog) s_autoLog = fopen("isimpa_auto.log", "w");
    if (!s_autoLog) return;
    va_list ap;
    va_start(ap, fmt);
    vfprintf(s_autoLog, fmt, ap);
    va_end(ap);
    fprintf(s_autoLog, "\n");
    fflush(s_autoLog);
}

void App::QueueAutomation(const std::vector<std::string>& args) {
    for (size_t i = 0; i < args.size(); i++) {
        const auto& a = args[i];
        if (a == "--auto" || a == "--skip-splash") {
            m_autoCmds.push_back({AutoCmd::SkipSplash, "", 0});
            m_autoMode = true;
        }
        else if (a == "--load" && i + 1 < args.size()) {
            m_autoCmds.push_back({AutoCmd::LoadScene, args[++i], 0.5f});
        }
        else if (a == "--run-spps") {
            m_autoCmds.push_back({AutoCmd::RunSPPS, "", 1.0f});
        }
        else if (a == "--run-tcr") {
            m_autoCmds.push_back({AutoCmd::RunTCR, "", 1.0f});
        }
        else if (a == "--load-results") {
            m_autoCmds.push_back({AutoCmd::LoadResults, "", 1.0f});
        }
        // --heatmap removed (feature removed from GUI)
        else if (a == "--focus") {
            m_autoCmds.push_back({AutoCmd::FocusCamera, "", 0.3f});
        }
        else if (a == "--add-source" && i + 1 < args.size()) {
            m_autoCmds.push_back({AutoCmd::AddSource, args[++i], 0.3f});
        }
        else if (a == "--add-receiver" && i + 1 < args.size()) {
            m_autoCmds.push_back({AutoCmd::AddReceiver, args[++i], 0.3f});
        }
        else if (a == "--add-surface-receiver") {
            m_autoCmds.push_back({AutoCmd::AddSurfaceReceiver, "", 0.3f});
        }
        else if (a == "--wait" && i + 1 < args.size()) {
            m_autoCmds.push_back({AutoCmd::Wait, "", std::stof(args[++i])});
        }
        else if (a == "--quit") {
            m_autoCmds.push_back({AutoCmd::Quit, "", 0.5f});
        }
    }
    if (!m_autoCmds.empty()) {
        m_autoMode = true;
        m_autoCmdIdx = 0;
        m_autoTimer = 0;
        AutoLog("[Auto] Queued %zu commands", m_autoCmds.size());
        for (size_t j = 0; j < m_autoCmds.size(); j++) {
            AutoLog("  [%zu] type=%d arg='%s' delay=%.1f", j, (int)m_autoCmds[j].type, m_autoCmds[j].arg.c_str(), m_autoCmds[j].delay);
        }
    }
}

void App::ProcessAutomation() {
    if (!m_autoMode || m_autoCmdIdx >= (int)m_autoCmds.size()) return;

    m_autoTimer -= ImGui::GetIO().DeltaTime;
    if (m_autoTimer > 0) return; // Still waiting

    // Don't process next command while simulation is running (wait for it)
    if (s_simRunning.load()) return;

    auto& cmd = m_autoCmds[m_autoCmdIdx];

    switch (cmd.type) {
        case AutoCmd::SkipSplash:
            m_splashDone = true;
            ConsoleLog("[Auto] Splash skipped", 0);
            break;

        case AutoCmd::LoadScene: {
            AutoLog("[Auto] LoadScene: '%s'", cmd.arg.c_str());
            ConsoleLog("[Auto] Loading scene: " + cmd.arg, 0);
            Project& proj = GetProject();
            SceneModel model;
            bool ok = false;
            std::string ext = cmd.arg.substr(cmd.arg.find_last_of('.'));
            AutoLog("[Auto] Extension: '%s', calling loader...", ext.c_str());
            if (ext == ".ply") ok = LoadPLY(cmd.arg, model);
            else if (ext == ".obj") ok = LoadOBJ(cmd.arg, model);
            else if (ext == ".stl") ok = LoadSTL(cmd.arg, model);
            else if (ext == ".3ds") ok = Load3DS(cmd.arg, model);
            else if (ext == ".isimpa" || ext == ".proj") {
                AutoLog("[Auto] Calling SmartLoadProject for .proj...");
                ok = SmartLoadProject(proj, cmd.arg);
                AutoLog("[Auto] SmartLoadProject returned ok=%d, verts=%zu", ok, proj.model.vertices.size());
                if (ok) {
                    ViewportLoadModel(proj.model);
                    ConsoleLog("[Auto] Project loaded: " + std::to_string(proj.model.vertices.size()) + " verts", 0);
                }
            }
            AutoLog("[Auto] Load result: ok=%d, model verts=%zu, proj verts=%zu", ok, model.vertices.size(), proj.model.vertices.size());
            if (ok && model.vertices.size() > 0) {
                proj.model = model;
                ViewportLoadModel(model);
                // Auto-assign default materials to all groups
                for (int gi = 0; gi < (int)proj.model.groups.size(); gi++) {
                    int matIdx = gi % (int)proj.materials.size();
                    proj.AssignMaterial(gi, matIdx);
                }
                ConsoleLog("[Auto] Loaded: " + std::to_string(model.vertices.size()) + " verts, " +
                           std::to_string(model.groups.size()) + " groups", 0);
                // Replace surface receivers sized to new geometry
                {
                    proj.surfaceReceivers.clear();
                    float margin = 0.5f;
                    float cx = (proj.model.bbMin.x + proj.model.bbMax.x) * 0.5f;

                    // Horizontal plane at ear height (floor map)
                    auto& sr1 = proj.AddSurfaceReceiver();
                    sr1.name = "Floor Map (1.2m)";
                    sr1.vertexA = glm::vec3(proj.model.bbMin.x + margin, 1.2f, proj.model.bbMin.z + margin);
                    sr1.vertexB = glm::vec3(proj.model.bbMax.x - margin, 1.2f, proj.model.bbMin.z + margin);
                    sr1.vertexC = glm::vec3(proj.model.bbMax.x - margin, 1.2f, proj.model.bbMax.z - margin);

                    // Vertical cross-section down the center (longitudinal slice)
                    auto& sr2 = proj.AddSurfaceReceiver();
                    sr2.name = "Cross Section (center)";
                    sr2.vertexA = glm::vec3(cx, proj.model.bbMin.y + margin, proj.model.bbMin.z + margin);
                    sr2.vertexB = glm::vec3(cx, proj.model.bbMax.y - margin, proj.model.bbMin.z + margin);
                    sr2.vertexC = glm::vec3(cx, proj.model.bbMax.y - margin, proj.model.bbMax.z - margin);

                    ConsoleLog("[Auto] Auto-added 2 surface receivers (floor + cross-section)", 0);
                }
            } else if (!ok) {
                AutoLog("[Auto] FAILED to load: %s", cmd.arg.c_str());
                ConsoleLog("[Auto] Failed to load: " + cmd.arg, 2);
            }
            break;
        }
        case AutoCmd::AddSource: {
            // Parse "x,y,z" or "x,y,z,name"
            auto& proj = GetProject();
            auto& src = proj.AddSource();
            float x = 0, y = 1.5f, z = 0;
            char name[64] = {};
            if (sscanf(cmd.arg.c_str(), "%f,%f,%f,%63s", &x, &y, &z, name) >= 3) {
                src.position = glm::vec3(x, y, z);
                if (name[0]) src.name = name;
            }
            ConsoleLog("[Auto] Added source: " + src.name + " at (" +
                       std::to_string(x) + ", " + std::to_string(y) + ", " + std::to_string(z) + ")", 0);
            break;
        }
        case AutoCmd::AddReceiver: {
            auto& proj = GetProject();
            auto& rcv = proj.AddReceiver();
            float x = 0, y = 1.2f, z = 0;
            char name[64] = {};
            if (sscanf(cmd.arg.c_str(), "%f,%f,%f,%63s", &x, &y, &z, name) >= 3) {
                rcv.position = glm::vec3(x, y, z);
                if (name[0]) rcv.name = name;
            }
            ConsoleLog("[Auto] Added receiver: " + rcv.name, 0);
            break;
        }
        case AutoCmd::AddSurfaceReceiver: {
            auto& proj = GetProject();
            auto& sr = proj.AddSurfaceReceiver();
            // Auto-size to room bounding box at ear height
            if (!proj.model.IsEmpty()) {
                float margin = 0.5f;
                sr.vertexA = glm::vec3(proj.model.bbMin.x + margin, 1.2f, proj.model.bbMin.z + margin);
                sr.vertexB = glm::vec3(proj.model.bbMax.x - margin, 1.2f, proj.model.bbMin.z + margin);
                sr.vertexC = glm::vec3(proj.model.bbMax.x - margin, 1.2f, proj.model.bbMax.z - margin);
            }
            ConsoleLog("[Auto] Added surface receiver: " + sr.name, 0);
            break;
        }

        case AutoCmd::RunSPPS:
            ConsoleLog("[Auto] Starting SPPS simulation...", 0);
            RunSimulationAsync("spps");
            break;

        case AutoCmd::RunTCR:
            ConsoleLog("[Auto] Starting TCR simulation...", 0);
            RunSimulationAsync("tcr");
            break;

        case AutoCmd::LoadResults:
            ConsoleLog("[Auto] Load results — click 'Load Results' in Results panel", 0);
            break;

        case AutoCmd::BuildHeatmap:
            // Heatmap feature removed
            break;

        case AutoCmd::FocusCamera:
            ViewportFocusModel();
            ConsoleLog("[Auto] Camera focused on model", 0);
            break;

        case AutoCmd::Wait:
            ConsoleLog("[Auto] Waited " + std::to_string(cmd.delay) + "s", 0);
            break;

        case AutoCmd::Quit:
            AutoLog("[Auto] Quit requested");
            ConsoleLog("[Auto] Exiting...", 0);
            m_running = false;
            break;
    }

    m_autoCmdIdx++;
    if (m_autoCmdIdx < (int)m_autoCmds.size()) {
        m_autoTimer = m_autoCmds[m_autoCmdIdx].delay;
    } else {
        ConsoleLog("[Auto] All automation commands complete!", 0);
    }
}

void App::Run() {
    while (!glfwWindowShouldClose(m_window) && m_running) {
        glfwPollEvents();
        BeginFrame();

        // ── Splash screen ───────────────────────────────────────────────────
        if (!m_splashDone) {
            m_splashTimer += ImGui::GetIO().DeltaTime;
            ImGuiViewport* vp = ImGui::GetMainViewport();
            ImGui::SetNextWindowPos(vp->WorkPos);
            ImGui::SetNextWindowSize(vp->WorkSize);
            ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 0));
            ImGui::PushStyleColor(ImGuiCol_WindowBg, ImVec4(0.04f, 0.04f, 0.06f, 1.0f));
            ImGui::Begin("##Splash", nullptr,
                ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoMove |
                ImGuiWindowFlags_NoScrollbar | ImGuiWindowFlags_NoDocking | ImGuiWindowFlags_NoNav);

            ImVec2 center(vp->WorkPos.x + vp->WorkSize.x * 0.5f, vp->WorkPos.y + vp->WorkSize.y * 0.5f);
            ImDrawList* dl = ImGui::GetWindowDrawList();

            // Glow title
            float alpha = std::min(m_splashTimer * 2.0f, 1.0f);
            ImU32 titleCol = ImGui::GetColorU32(ImVec4(0.95f, 0.1f, 0.15f, alpha));
            ImU32 subCol = ImGui::GetColorU32(ImVec4(0.72f, 0.73f, 0.78f, alpha));
            ImU32 dimCol = ImGui::GetColorU32(ImVec4(0.45f, 0.46f, 0.52f, alpha));

            const char* title = "I-SIMPA";
            const char* sub = "Dark Neon Edition";
            const char* ver = "v0.2.0  //  GPU Accelerated Acoustic Simulation";

            ImVec2 titleSize = ImGui::CalcTextSize(title);
            ImVec2 subSize = ImGui::CalcTextSize(sub);
            ImVec2 verSize = ImGui::CalcTextSize(ver);

            dl->AddText(ImVec2(center.x - titleSize.x * 0.5f, center.y - 40), titleCol, title);
            dl->AddText(ImVec2(center.x - subSize.x * 0.5f, center.y - 10), subCol, sub);
            dl->AddText(ImVec2(center.x - verSize.x * 0.5f, center.y + 20), dimCol, ver);

            // Progress bar
            float barW = 300, barH = 3;
            float progress = std::min(m_splashTimer / 1.5f, 1.0f);
            ImVec2 barMin(center.x - barW * 0.5f, center.y + 50);
            dl->AddRectFilled(barMin, ImVec2(barMin.x + barW, barMin.y + barH),
                ImGui::GetColorU32(ImVec4(0.15f, 0.15f, 0.2f, alpha)));
            dl->AddRectFilled(barMin, ImVec2(barMin.x + barW * progress, barMin.y + barH),
                ImGui::GetColorU32(ImVec4(0.95f, 0.1f, 0.15f, alpha)));

            ImGui::End();
            ImGui::PopStyleColor();
            ImGui::PopStyleVar();

            if (m_splashTimer > 2.0f || ImGui::IsMouseClicked(0) || ImGui::IsKeyPressed(ImGuiKey_Escape)) {
                m_splashDone = true;
            }
            EndFrame();
            continue; // Skip main UI during splash
        }

        // ── Process automation commands ─────────────────────────────────────
        ProcessAutomation();

        // ── Keyboard shortcuts ──────────────────────────────────────────────
        ImGuiIO& io = ImGui::GetIO();
        if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_P)) {
            m_commandPalette.Open();
        }
        if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_S)) {
            // Save
            Project& proj = GetProject();
            if (proj.projectPath.empty()) {
                std::string path = SaveFileDialog(
                    "I-Simpa Project (*.isimpa)\0*.isimpa\0", "Save Project", "isimpa");
                if (!path.empty()) proj.projectPath = path;
            }
            if (!proj.projectPath.empty()) {
                SaveProject(proj, proj.projectPath);
                ConsoleLog("[Project] Saved: " + proj.projectPath);
            }
        }
        if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_O)) {
            std::string path = OpenFileDialog(
                "I-Simpa Project (*.isimpa;*.proj)\0*.isimpa;*.proj\0Original I-Simpa (*.proj)\0*.proj\0All Files\0*.*\0",
                "Open Project");
            if (!path.empty()) {
                Project& proj = GetProject();
                if (SmartLoadProject(proj, path)) {
                    proj.projectPath = path;
                    ViewportLoadModel(proj.model);
                    ConsoleLog("[Project] Loaded: " + path);
                }
            }
        }

        // ── Undo/Redo ────────────────────────────────────────────────────────
        if (io.KeyCtrl && ImGui::IsKeyPressed(ImGuiKey_Z) && !io.KeyShift) {
            if (GetUndoManager().Undo(GetProject())) {
                ViewportLoadModel(GetProject().model);
                m_dirty = true;
                ConsoleLog("[Undo] Restored previous state");
            }
        }
        if (io.KeyCtrl && (ImGui::IsKeyPressed(ImGuiKey_Y) ||
            (io.KeyShift && ImGui::IsKeyPressed(ImGuiKey_Z)))) {
            if (GetUndoManager().Redo(GetProject())) {
                ViewportLoadModel(GetProject().model);
                m_dirty = true;
                ConsoleLog("[Redo] Restored next state");
            }
        }

        // ── Delete key ───────────────────────────────────────────────────────
        // Escape: clear selection
        if (ImGui::IsKeyPressed(ImGuiKey_Escape) && !io.WantTextInput) {
            GetSelection().Clear();
            ViewportRefreshGPU();
        }

        if (ImGui::IsKeyPressed(ImGuiKey_Delete) && !io.WantTextInput) {
            Selection& sel = GetSelection();
            Project& proj = GetProject();
            if (sel.source >= 0 && sel.source < (int)proj.sources.size()) {
                GetUndoManager().SaveState(proj, "Delete Source");
                ConsoleLog("[Source] Deleted: " + proj.sources[sel.source].name);
                proj.sources.erase(proj.sources.begin() + sel.source);
                sel.Clear();
                m_dirty = true;
            } else if (sel.receiver >= 0 && sel.receiver < (int)proj.punctualReceivers.size()) {
                GetUndoManager().SaveState(proj, "Delete Receiver");
                ConsoleLog("[Receiver] Deleted: " + proj.punctualReceivers[sel.receiver].name);
                proj.punctualReceivers.erase(proj.punctualReceivers.begin() + sel.receiver);
                sel.Clear();
                m_dirty = true;
            }
        }

        UpdateWorkflowStatus();
        UpdateWindowTitle();
        DrawMainDockspace();

        // Demo window toggle (F1)
        if (ImGui::IsKeyPressed(ImGuiKey_F1)) m_showDemo = !m_showDemo;

        // Focus mode: F5=Viewport only, F6=Results only, F7=All panels
        auto setFocus = [&](bool out, bool prop, bool mat, bool con, bool res, bool sim, bool vp) {
            m_showOutliner = out; m_showProperties = prop; m_showMaterials = mat;
            m_showConsole = con; m_showResults = res; m_showSimulate = sim; m_showViewport = vp;
        };
        if (ImGui::IsKeyPressed(ImGuiKey_F5)) setFocus(false, false, false, false, false, false, true);  // Viewport only
        if (ImGui::IsKeyPressed(ImGuiKey_F6)) setFocus(false, false, false, false, true, false, false);  // Results only
        if (ImGui::IsKeyPressed(ImGuiKey_F7)) setFocus(true, true, true, true, true, true, true);        // All panels
        if (ImGui::IsKeyPressed(ImGuiKey_F8)) setFocus(false, false, false, false, true, false, true);   // Viewport + Results
        if (m_showDemo) ImGui::ShowDemoWindow(&m_showDemo);

        // ── Exit confirmation with unsaved changes ──────────────────────────
        if (glfwWindowShouldClose(m_window) && m_dirty) {
            glfwSetWindowShouldClose(m_window, GLFW_FALSE);
            ImGui::OpenPopup("Unsaved Changes##Exit");
        }
        if (ImGui::BeginPopupModal("Unsaved Changes##Exit", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
            ImGui::Text("You have unsaved changes. Save before exiting?");
            ImGui::Spacing();
            if (ImGui::Button("Save & Exit", ImVec2(120, 0))) {
                Project& proj = GetProject();
                if (proj.projectPath.empty()) {
                    std::string path = SaveFileDialog(
                        "I-Simpa Project (*.isimpa)\0*.isimpa\0", "Save Project", "isimpa");
                    if (!path.empty()) proj.projectPath = path;
                }
                if (!proj.projectPath.empty()) SaveProject(proj, proj.projectPath);
                m_dirty = false;
                m_running = false;
                ImGui::CloseCurrentPopup();
            }
            ImGui::SameLine();
            if (ImGui::Button("Discard & Exit", ImVec2(120, 0))) {
                m_dirty = false;
                m_running = false;
                ImGui::CloseCurrentPopup();
            }
            ImGui::SameLine();
            if (ImGui::Button("Cancel", ImVec2(120, 0))) {
                ImGui::CloseCurrentPopup();
            }
            ImGui::EndPopup();
        }

        EndFrame();
    }
}

void App::Shutdown() {
    // Wait for simulation thread to finish before destroying resources
    JoinSimThread();

    if (m_window) {
        ImGui_ImplOpenGL3_Shutdown();
        ImGui_ImplGlfw_Shutdown();
        ImPlot::DestroyContext();
        ImGui::DestroyContext();
        glfwDestroyWindow(m_window);
        glfwTerminate();
        m_window = nullptr;
    }
}

void App::BeginFrame() {
    ImGui_ImplOpenGL3_NewFrame();
    ImGui_ImplGlfw_NewFrame();
    ImGui::NewFrame();
}

void App::EndFrame() {
    ImGui::Render();
    int w, h;
    glfwGetFramebufferSize(m_window, &w, &h);
    glViewport(0, 0, w, h);
    glClearColor(m_clearColor[0], m_clearColor[1], m_clearColor[2], m_clearColor[3]);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());

    glfwSwapBuffers(m_window);
}

void App::DrawMainDockspace() {
    // Full-screen dockspace window
    const ImGuiViewport* viewport = ImGui::GetMainViewport();
    ImGui::SetNextWindowPos(viewport->WorkPos);
    ImGui::SetNextWindowSize(viewport->WorkSize);
    ImGui::SetNextWindowViewport(viewport->ID);

    ImGuiWindowFlags hostFlags =
        ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoCollapse |
        ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoMove |
        ImGuiWindowFlags_NoBringToFrontOnFocus | ImGuiWindowFlags_NoNavFocus |
        ImGuiWindowFlags_MenuBar | ImGuiWindowFlags_NoDocking;

    ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0f);
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 0));

    ImGui::Begin("##DockspaceHost", nullptr, hostFlags);
    ImGui::PopStyleVar(3);

    // Menu bar
    DrawMenuBar();

    // ── Layout: Workflow Rail | Dockable Area ───────────────────────────────
    ImGui::PushStyleVar(ImGuiStyleVar_ItemSpacing, ImVec2(0, 0));

    // Workflow rail on the far left
    m_workflowRail.Draw();
    ImGui::SameLine();

    // Dockspace fills the rest
    ImGuiID dockspaceId = ImGui::GetID("MainDockspace");
    ImGui::DockSpace(dockspaceId, ImVec2(0, 0),
        ImGuiDockNodeFlags_PassthruCentralNode);

    ImGui::PopStyleVar(); // ItemSpacing

    // ── Draw panels (only if visible) ──────────────────────────────────────
    if (m_showOutliner)   DrawOutliner();
    if (m_showProperties) DrawProperties();
    if (m_showConsole)    DrawConsole();
    if (m_showResults)    DrawResults();
    if (m_showMaterials)  DrawMaterials();
    if (m_showSimulate)   DrawSimulatePanel();

    // Viewport panel
    if (m_showViewport) {
        ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 0));
        if (ImGui::Begin("3D Viewport", &m_showViewport)) {
            ImVec2 size = ImGui::GetContentRegionAvail();
            DrawViewport(size.x, size.y);
        }
        ImGui::End();
        ImGui::PopStyleVar();
    }

    // Status bar at bottom
    DrawStatusBar();

    // Command palette overlay
    DrawCommandPalettePopup(m_commandPalette);

    // About dialog
    if (m_showAbout) {
        ImGui::OpenPopup("About I-Simpa##Dialog");
        m_showAbout = false;
    }
    if (ImGui::BeginPopupModal("About I-Simpa##Dialog", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::Text("I-SIMPA  //  Dark Neon Edition");
        ImGui::PopStyleColor();
        ImGui::Spacing();
        ImGui::Text("Version 0.2.0");
        ImGui::Text("GPU-Accelerated Acoustic Simulation GUI");
        ImGui::Spacing();
        ImGui::Separator();
        ImGui::Spacing();
        ImGui::Text("Based on I-Simpa by Universite Gustave Eiffel");
        ImGui::Text("Solvers: SPPS (Particle Tracing) + TCR (Classical Theory)");
        ImGui::Text("Renderer: OpenGL 4.6 + Dear ImGui (Docking)");
        ImGui::Spacing();
        ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
            "Built with GLFW, ImGui, ImPlot, ImGuizmo, GLM");
        ImGui::Spacing();
        if (ImGui::Button("OK", ImVec2(120, 0))) ImGui::CloseCurrentPopup();
        ImGui::EndPopup();
    }

    // Measurement display overlay
    if (g_viewportMeasureState == 3) {
        float dist = glm::length(g_viewportMeasureB - g_viewportMeasureA);
        char measBuf[128];
        snprintf(measBuf, sizeof(measBuf), "Distance: %.3f m  (%.1f, %.1f, %.1f) to (%.1f, %.1f, %.1f)",
                 dist, g_viewportMeasureA.x, g_viewportMeasureA.y, g_viewportMeasureA.z,
                 g_viewportMeasureB.x, g_viewportMeasureB.y, g_viewportMeasureB.z);
        ImGui::SetNextWindowPos(ImVec2(ImGui::GetMainViewport()->WorkPos.x + 200,
                                       ImGui::GetMainViewport()->WorkPos.y + 30));
        ImGui::SetNextWindowSize(ImVec2(500, 0));
        ImGui::PushStyleColor(ImGuiCol_WindowBg, ImVec4(0.08f, 0.08f, 0.11f, 0.95f));
        ImGui::Begin("##Measurement", nullptr,
            ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoDocking |
            ImGuiWindowFlags_AlwaysAutoResize);
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f));
        ImGui::Text("%s", measBuf);
        ImGui::PopStyleColor();
        ImGui::SameLine();
        if (ImGui::SmallButton("Close")) { g_viewportMeasureState = 0; g_viewportMeasureMode = false; }
        ImGui::SameLine();
        if (ImGui::SmallButton("New")) { g_viewportMeasureState = 1; g_viewportMeasureMode = true; ConsoleLog("[Measure] Click first point..."); }
        ImGui::End();
        ImGui::PopStyleColor();
    }

    // Error popup
    if (s_showError) {
        ImGui::OpenPopup("Error##Popup");
        s_showError = false;
    }
    if (ImGui::BeginPopupModal("Error##Popup", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(NeonColors::Red[0], NeonColors::Red[1], NeonColors::Red[2], 1.0f));
        ImGui::TextWrapped("%s", s_errorMsg.c_str());
        ImGui::PopStyleColor();
        ImGui::Spacing();
        if (ImGui::Button("OK", ImVec2(120, 0))) ImGui::CloseCurrentPopup();
        ImGui::EndPopup();
    }

    ImGui::End(); // DockspaceHost
}

void App::DrawMenuBar() {
    // New Room dialog
    if (m_showNewRoom) {
        ImGui::OpenPopup("New Room##Dialog");
        m_showNewRoom = false;
    }
    if (ImGui::BeginPopupModal("New Room##Dialog", nullptr, ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::Text("Create a rectangular room");
        ImGui::Spacing();
        ImGui::DragFloat("Width (m)",  &m_newRoomDims[0], 0.1f, 0.5f, 100.0f);
        ImGui::DragFloat("Length (m)", &m_newRoomDims[1], 0.1f, 0.5f, 100.0f);
        ImGui::DragFloat("Height (m)", &m_newRoomDims[2], 0.1f, 0.5f, 50.0f);
        ImGui::Spacing();
        ImGui::Separator();
        ImGui::Spacing();

        if (ImGui::Button("Create", ImVec2(120, 0))) {
            Project& proj = GetProject();
            GetUndoManager().SaveState(proj, "New Room");
            proj.NewProject();
            proj.CreateDefaultRoom(m_newRoomDims[0], m_newRoomDims[1], m_newRoomDims[2]);
            m_workflowRail.SetPhaseStatus(WorkflowPhase::Room, PhaseStatus::Ready);
            m_workflowRail.SetPhaseCount(WorkflowPhase::Room, (int)proj.model.groups.size());
            m_dirty = true;
            ConsoleLog("[Room] Created " + std::to_string(m_newRoomDims[0]) + "x" +
                       std::to_string(m_newRoomDims[1]) + "x" + std::to_string(m_newRoomDims[2]) + "m room");
            ImGui::CloseCurrentPopup();
        }
        ImGui::SameLine();
        if (ImGui::Button("Cancel", ImVec2(120, 0))) {
            ImGui::CloseCurrentPopup();
        }
        ImGui::EndPopup();
    }

    if (ImGui::BeginMenuBar()) {
        if (ImGui::BeginMenu("File")) {
            if (ImGui::MenuItem("New Project",    "Ctrl+N"))  {
                m_newRoomDims[0] = 6.0f; m_newRoomDims[1] = 10.0f; m_newRoomDims[2] = 3.0f;
                m_showNewRoom = true;
            }
            if (ImGui::MenuItem("New Scene"))                  { m_showNewRoom = true; }
            if (ImGui::MenuItem("Open Project",   "Ctrl+O"))  {
                std::string path = OpenFileDialog(
                    "I-Simpa Project (*.isimpa;*.proj)\0*.isimpa;*.proj\0Original I-Simpa (*.proj)\0*.proj\0All Files\0*.*\0",
                    "Open Project");
                if (!path.empty()) {
                    Project& proj = GetProject();
                    if (SmartLoadProject(proj, path)) {
                        proj.projectPath = path;
                        ViewportLoadModel(proj.model);
                        m_dirty = false;
                        AddRecentFile(path);
                        ConsoleLog("[Project] Loaded: " + path);
                    }
                }
            }
            if (ImGui::BeginMenu("Recent Projects", !m_recentFiles.empty())) {
                for (auto& recent : m_recentFiles) {
                    auto fname = fs::path(recent).filename().string();
                    if (ImGui::MenuItem(fname.c_str())) {
                        Project& proj = GetProject();
                        if (SmartLoadProject(proj, recent)) {
                            proj.projectPath = recent;
                            ViewportLoadModel(proj.model);
                            m_dirty = false;
                            AddRecentFile(recent);
                            ConsoleLog("[Project] Loaded: " + recent);
                        }
                    }
                    if (ImGui::IsItemHovered()) ImGui::SetTooltip("%s", recent.c_str());
                }
                ImGui::EndMenu();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Import Scene (.ply)"))       {
                std::string path = OpenFileDialog(
                    "PLY Files (*.ply)\0*.ply\0All Files\0*.*\0", "Import 3D Scene");
                if (!path.empty()) {
                    GetProject().LoadScenePLY(path);
                    ConsoleLog("[Scene] Imported: " + path);
                }
            }
            if (ImGui::MenuItem("Import Scene (.obj)"))       {
                std::string path = OpenFileDialog(
                    "Wavefront OBJ (*.obj)\0*.obj\0All Files\0*.*\0", "Import OBJ");
                if (!path.empty()) {
                    SceneModel model;
                    if (LoadOBJ(path, model)) {
                        GetProject().model = model;
                        ViewportLoadModel(model);
                        m_dirty = true;
                        ConsoleLog("[Scene] Imported OBJ: " + path);
                    }
                }
            }
            if (ImGui::MenuItem("Import Scene (.3ds)"))       {
                std::string path = OpenFileDialog(
                    "3D Studio (*.3ds)\0*.3ds\0All Files\0*.*\0", "Import 3DS");
                if (!path.empty()) {
                    SceneModel model;
                    if (Load3DS(path, model)) {
                        GetProject().model = model;
                        ViewportLoadModel(model);
                        m_dirty = true;
                        ConsoleLog("[Scene] Imported 3DS: " + path);
                    }
                }
            }
            if (ImGui::MenuItem("Import Scene (.stl)"))       {
                std::string path = OpenFileDialog(
                    "STL Files (*.stl)\0*.stl\0All Files\0*.*\0", "Import STL");
                if (!path.empty()) {
                    SceneModel model;
                    if (LoadSTL(path, model)) {
                        GetProject().model = model;
                        ViewportLoadModel(model);
                        ConsoleLog("[Scene] Imported STL: " + path);
                    }
                }
            }
            if (ImGui::MenuItem("Export Scene (.cbin)"))      {
                std::string path = SaveFileDialog(
                    "I-Simpa Binary (*.cbin)\0*.cbin\0", "Export Scene", "cbin");
                if (!path.empty()) {
                    WriteMeshBinary(GetProject(), path);
                    ConsoleLog("[Export] Wrote: " + path);
                }
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Save",           "Ctrl+S"))  {
                Project& proj = GetProject();
                if (proj.projectPath.empty()) {
                    std::string path = SaveFileDialog(
                        "I-Simpa Project (*.isimpa)\0*.isimpa\0", "Save Project", "isimpa");
                    if (!path.empty()) proj.projectPath = path;
                }
                if (!proj.projectPath.empty()) {
                    SaveProject(proj, proj.projectPath);
                    m_dirty = false;
                    ConsoleLog("[Project] Saved: " + proj.projectPath);
                }
            }
            if (ImGui::MenuItem("Save As..."))                 {
                std::string path = SaveFileDialog(
                    "I-Simpa Project (*.isimpa)\0*.isimpa\0", "Save Project As", "isimpa");
                if (!path.empty()) {
                    Project& proj = GetProject();
                    proj.projectPath = path;
                    SaveProject(proj, path);
                    ConsoleLog("[Project] Saved as: " + path);
                }
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Exit"))           { m_running = false; }
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Edit")) {
            auto& undo = GetUndoManager();
            char undoLabel[128] = "Undo";
            char redoLabel[128] = "Redo";
            if (undo.UndoActionName()) snprintf(undoLabel, sizeof(undoLabel), "Undo %s", undo.UndoActionName());
            if (undo.RedoActionName()) snprintf(redoLabel, sizeof(redoLabel), "Redo %s", undo.RedoActionName());

            if (ImGui::MenuItem(undoLabel, "Ctrl+Z", false, undo.CanUndo())) {
                if (undo.Undo(GetProject())) {
                    ViewportLoadModel(GetProject().model);
                    m_dirty = true;
                }
            }
            if (ImGui::MenuItem(redoLabel, "Ctrl+Y", false, undo.CanRedo())) {
                if (undo.Redo(GetProject())) {
                    ViewportLoadModel(GetProject().model);
                    m_dirty = true;
                }
            }
            ImGui::Separator();
            ImGui::Separator();
            if (ImGui::MenuItem("Merge Selected Groups", nullptr, false, GetSelection().selectedGroups.size() >= 2)) {
                // Inline merge logic
                Selection& sel = GetSelection();
                Project& proj = GetProject();
                GetUndoManager().SaveState(proj, "Merge Groups");
                int firstIdx = *sel.selectedGroups.begin();
                auto& target = proj.model.groups[firstIdx];
                std::vector<int> toRemove;
                for (int gi : sel.selectedGroups) {
                    if (gi == firstIdx) continue;
                    target.faces.insert(target.faces.end(), proj.model.groups[gi].faces.begin(), proj.model.groups[gi].faces.end());
                    toRemove.push_back(gi);
                }
                std::sort(toRemove.rbegin(), toRemove.rend());
                for (int gi : toRemove) proj.model.groups.erase(proj.model.groups.begin() + gi);
                ViewportLoadModel(proj.model);
                sel.Clear();
                ConsoleLog("[Scene] Merged groups");
            }
            if (ImGui::MenuItem("Delete Selected Group", "Del", false, GetSelection().group >= 0)) {
                Project& proj = GetProject();
                GetUndoManager().SaveState(proj, "Delete Group");
                proj.model.groups.erase(proj.model.groups.begin() + GetSelection().group);
                ViewportLoadModel(proj.model);
                GetSelection().Clear();
            }
            if (ImGui::MenuItem("Flip Group Normals", nullptr, false, GetSelection().group >= 0)) {
                Project& proj = GetProject();
                GetUndoManager().SaveState(proj, "Flip Normals");
                for (auto& f : proj.model.groups[GetSelection().group].faces) {
                    std::swap(f.v[1], f.v[2]);
                    f.normal = -f.normal;
                }
                ViewportRefreshGPU();
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Preferences"))    {
                ConsoleLog("[Info] Preferences dialog coming soon", 1);
            }
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Simulation")) {
            if (ImGui::MenuItem("Generate Mesh", nullptr, false, !s_simRunning.load())) {
                if (GetProject().HasGeometry()) {
                    ConsoleLog("[Mesh] Starting mesh generation...");
                    std::string workDir = s_exeDir + "sim_output/mesh";
                    fs::create_directories(workDir);
                    RunMeshGeneration(workDir, GetProject());
                    ConsoleLog("[Mesh] Done");
                }
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Run SPPS", nullptr, false, !s_simRunning.load())) {
                RunSimulationAsync("spps");
            }
            if (ImGui::MenuItem("Run TCR (Classical Theory)", nullptr, false, !s_simRunning.load())) {
                RunSimulationAsync("tcr");
            }
            ImGui::Separator();
            if (ImGui::MenuItem("Load Results...")) {
                std::string dir = BrowseFolderDialog("Select results directory");
                if (!dir.empty()) {
                    GetProject().lastResultDir = dir;
                    ConsoleLog("[Results] Set result directory: " + dir);
                }
            }
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("View")) {
            bool showWire = ViewportGetShowWireframe();
            bool showFaces = ViewportGetShowFaces();
            if (ImGui::MenuItem("Show Wireframe", nullptr, &showWire)) ViewportToggleWireframe();
            if (ImGui::MenuItem("Show Faces", nullptr, &showFaces)) ViewportToggleFaces();
            ImGui::Separator();
            if (ImGui::MenuItem("Top View"))    ViewportCameraTop();
            if (ImGui::MenuItem("Front View"))  ViewportCameraFront();
            if (ImGui::MenuItem("Right View"))  ViewportCameraRight();
            if (ImGui::MenuItem("Reset View"))  ViewportCameraReset();
            if (ImGui::MenuItem("Focus Model")) ViewportFocusModel();
            ImGui::Separator();
            ImGui::MenuItem("Show Demo Window", "F1", &m_showDemo);
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Window")) {
            ImGui::MenuItem("Outliner", nullptr, &m_showOutliner);
            ImGui::MenuItem("Properties", nullptr, &m_showProperties);
            ImGui::MenuItem("Materials", nullptr, &m_showMaterials);
            ImGui::MenuItem("Console", nullptr, &m_showConsole);
            ImGui::MenuItem("Results", nullptr, &m_showResults);
            ImGui::MenuItem("Simulate", nullptr, &m_showSimulate);
            ImGui::MenuItem("3D Viewport", nullptr, &m_showViewport);
            ImGui::Separator();
            if (ImGui::MenuItem("Show All Panels")) {
                m_showOutliner = m_showProperties = m_showMaterials = true;
                m_showConsole = m_showResults = m_showSimulate = m_showViewport = true;
            }
            if (ImGui::MenuItem("Reset Layout")) {
                std::remove("imgui_isimpa.ini");
                m_showOutliner = m_showProperties = m_showMaterials = true;
                m_showConsole = m_showResults = m_showSimulate = m_showViewport = true;
            }
            ImGui::Separator();
            ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
                "Focus Modes:");
            if (ImGui::MenuItem("Viewport Only", "F5")) {
                m_showOutliner = m_showProperties = m_showMaterials = false;
                m_showConsole = m_showResults = m_showSimulate = false; m_showViewport = true;
            }
            if (ImGui::MenuItem("Results Only", "F6")) {
                m_showOutliner = m_showProperties = m_showMaterials = false;
                m_showConsole = m_showSimulate = m_showViewport = false; m_showResults = true;
            }
            if (ImGui::MenuItem("Viewport + Results", "F8")) {
                m_showOutliner = m_showProperties = m_showMaterials = false;
                m_showConsole = m_showSimulate = false; m_showViewport = m_showResults = true;
            }
            if (ImGui::MenuItem("All Panels", "F7")) {
                m_showOutliner = m_showProperties = m_showMaterials = true;
                m_showConsole = m_showResults = m_showSimulate = m_showViewport = true;
            }
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Tools")) {
            if (ImGui::MenuItem("Save Screenshot", "")) {
                std::string path = SaveFileDialog(
                    "TGA Image (*.tga)\0*.tga\0", "Save Screenshot", "tga");
                if (!path.empty()) {
                    ViewportSaveScreenshot(path);
                    ConsoleLog("[Screenshot] Saved: " + path);
                }
            }
            if (ImGui::MenuItem("Measure Distance")) {
                g_viewportMeasureMode = true;
                g_viewportMeasureState = 1;
                ConsoleLog("[Measure] Click first point in viewport...");
            }
            ImGui::Separator();
            ImGui::MenuItem("Clipping Plane", nullptr, &m_clipEnabled);
            if (m_clipEnabled) {
                const char* axes[] = {"X", "Y (horizontal)", "Z"};
                ImGui::Combo("Axis", &m_clipAxis, axes, 3);
                float range = GetProject().HasGeometry() ? GetProject().model.extent * 2.0f : 10.0f;
                ImGui::SliderFloat("Height", &m_clipHeight, -range, range);
                ViewportSetClipPlane(true, m_clipAxis, m_clipHeight);
            } else {
                ViewportSetClipPlane(false, 0, 0);
            }
            ImGui::EndMenu();
        }
        if (ImGui::BeginMenu("Help")) {
            if (ImGui::MenuItem("About I-Simpa"))  { m_showAbout = true; }
            ImGui::EndMenu();
        }

        // Right-aligned phase indicator
        float phaseTextWidth = 200.0f;
        ImGui::SameLine(ImGui::GetWindowWidth() - phaseTextWidth);
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        const char* phaseName = m_workflowRail.GetPhase(m_workflowRail.GetActivePhase()).label;
        ImGui::Text("Phase: %s", phaseName);
        ImGui::PopStyleColor();

        ImGui::EndMenuBar();
    }
}

// ─── Simulate Panel ─────────────────────────────────────────────────────────

void App::DrawSimulatePanel() {
    if (!ImGui::Begin("Simulate")) {
        ImGui::End();
        return;
    }

    Project& proj = GetProject();

    // Validation section
    ImGui::PushStyleColor(ImGuiCol_Text,
        ImVec4(NeonColors::TextBright[0], NeonColors::TextBright[1], NeonColors::TextBright[2], 1.0f));
    ImGui::Text("Pre-flight Checks");
    ImGui::PopStyleColor();
    ImGui::Separator();
    ImGui::Spacing();

    auto checkItem = [](const char* label, bool ok) {
        ImVec4 col = ok ? ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f)
                        : ImVec4(NeonColors::Red[0], NeonColors::Red[1], NeonColors::Red[2], 1.0f);
        ImGui::PushStyleColor(ImGuiCol_Text, col);
        ImGui::Text("%s %s", ok ? "[OK]" : "[!!]", label);
        ImGui::PopStyleColor();
    };

    bool hasGeo = proj.HasGeometry();
    bool hasSrc = proj.HasSources();
    bool hasRcv = proj.HasReceivers();
    bool allMat = true;
    if (hasGeo) {
        for (int i = 0; i < (int)proj.model.groups.size(); i++) {
            if (proj.groupMaterialMap.find(i) == proj.groupMaterialMap.end()) {
                allMat = false; break;
            }
        }
    } else allMat = false;

    checkItem("Geometry loaded", hasGeo);
    checkItem("Materials assigned", allMat);
    checkItem("Sound sources placed", hasSrc);
    checkItem("Receivers placed", hasRcv);

    bool canRun = hasGeo && hasSrc && hasRcv;

    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();

    // Solver selection and launch
    ImGui::PushStyleColor(ImGuiCol_Text,
        ImVec4(NeonColors::TextBright[0], NeonColors::TextBright[1], NeonColors::TextBright[2], 1.0f));
    ImGui::Text("Solver");
    ImGui::PopStyleColor();
    ImGui::Spacing();

    static int solverChoice = 0;
    ImGui::RadioButton("SPPS (Particle Tracing)", &solverChoice, 0);
    ImGui::SameLine(250);
    ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
        "Monte Carlo ray tracing");
    ImGui::RadioButton("TCR (Classical Theory)", &solverChoice, 1);
    ImGui::SameLine(250);
    ImGui::TextColored(ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f),
        "Sabine/Eyring analytical");

    ImGui::Spacing();

    // Solver descriptions
    if (ImGui::CollapsingHeader("About Solvers")) {
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::Text("SPPS — Sound Particle Propagation Simulation");
        ImGui::PopStyleColor();
        ImGui::TextWrapped(
            "Stochastic particle tracing solver. Emits thousands of sound particles "
            "from each source and traces them as they reflect off walls, losing energy "
            "based on material absorption. Produces: per-receiver SPL and echograms, "
            "particle animation, intensity vectors, surface colormaps. Best for complex "
            "rooms where geometry matters (concert halls, studios, factories).");
        ImGui::Spacing();

        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f));
        ImGui::Text("TCR — Classical Theory of Reverberation");
        ImGui::PopStyleColor();
        ImGui::TextWrapped(
            "Analytical solver using Sabine and Eyring formulas. Computes reverberation "
            "time (RT60) and steady-state SPL from total room volume, surface areas, and "
            "average absorption coefficients. Very fast (< 1 second). Does not model "
            "geometry details — best for quick estimates on simple rooms.");
        ImGui::Spacing();
    }

    // Run button
    bool simRunning = s_simRunning.load();
    float simPercent = s_simPercent.load();
    std::string simStatus = GetSimStatus();

    if (simRunning) {
        // Progress display
        ImGui::PushStyleColor(ImGuiCol_PlotHistogram,
            ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
        ImGui::ProgressBar(simPercent / 100.0f, ImVec2(-1, 0),
            simStatus.empty() ? "Running..." : simStatus.c_str());
        ImGui::PopStyleColor();

        ImGui::Spacing();
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1], NeonColors::Yellow[2], 1.0f));
        ImGui::Text("Simulation in progress...");
        ImGui::PopStyleColor();
    } else {
        // Show last status message persistently
        if (!simStatus.empty()) {
            bool isSuccess = simStatus.find("DONE") != std::string::npos;
            bool isFail = simStatus.find("FAILED") != std::string::npos;
            if (isSuccess) {
                ImGui::PushStyleColor(ImGuiCol_Text,
                    ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f));
            } else if (isFail) {
                ImGui::PushStyleColor(ImGuiCol_Text,
                    ImVec4(NeonColors::Red[0], NeonColors::Red[1], NeonColors::Red[2], 1.0f));
            } else {
                ImGui::PushStyleColor(ImGuiCol_Text,
                    ImVec4(NeonColors::TextNormal[0], NeonColors::TextNormal[1], NeonColors::TextNormal[2], 1.0f));
            }
            ImGui::TextWrapped("%s", simStatus.c_str());
            ImGui::PopStyleColor();
            ImGui::Spacing();
        }

        ImGui::BeginDisabled(!canRun);
        ImVec4 btnCol(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f);
        ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(btnCol.x * 0.3f, btnCol.y * 0.3f, btnCol.z * 0.3f, 1.0f));
        ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4(btnCol.x * 0.5f, btnCol.y * 0.5f, btnCol.z * 0.5f, 1.0f));
        ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImVec4(btnCol.x * 0.7f, btnCol.y * 0.7f, btnCol.z * 0.7f, 1.0f));
        ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 1.0f, 1.0f, 1.0f));

        if (ImGui::Button("Run Simulation", ImVec2(-1, 40))) {
            RunSimulationAsync(solverChoice == 0 ? "spps" : "tcr");
            ConsoleLog("[Solver] Launching " + std::string(solverChoice == 0 ? "SPPS" : "TCR") + " simulation...");
        }

        ImGui::PopStyleColor(4);
        ImGui::EndDisabled();

        if (!canRun) {
            ImGui::PushStyleColor(ImGuiCol_Text,
                ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1], NeonColors::Yellow[2], 0.8f));
            ImGui::TextWrapped("Please complete all pre-flight checks before running.");
            ImGui::PopStyleColor();
        }
    }

    // Show last results
    if (!proj.lastResultDir.empty() && !simRunning) {
        ImGui::Spacing();
        ImGui::Separator();
        ImGui::Spacing();
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Green[0], NeonColors::Green[1], NeonColors::Green[2], 1.0f));
        ImGui::Text("Last results: %s", proj.lastResultDir.c_str());
        ImGui::PopStyleColor();
    }

    // Show errors/warnings from last run
    if (!proj.simProgress.errors.empty()) {
        ImGui::Spacing();
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Red[0], NeonColors::Red[1], NeonColors::Red[2], 1.0f));
        for (auto& err : proj.simProgress.errors) {
            ImGui::TextWrapped("Error: %s", err.c_str());
        }
        ImGui::PopStyleColor();
    }
    if (!proj.simProgress.warnings.empty()) {
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1], NeonColors::Yellow[2], 1.0f));
        for (auto& warn : proj.simProgress.warnings) {
            ImGui::TextWrapped("Warning: %s", warn.c_str());
        }
        ImGui::PopStyleColor();
    }

    ImGui::End();
}

void App::DrawStatusBar() {
    float barHeight = ImGui::GetFrameHeight();
    ImVec2 windowSize = ImGui::GetWindowSize();
    ImVec2 windowPos = ImGui::GetWindowPos();

    ImGui::SetNextWindowPos(ImVec2(windowPos.x, windowPos.y + windowSize.y - barHeight));
    ImGui::SetNextWindowSize(ImVec2(windowSize.x, barHeight));

    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(10, 2));
    ImGui::PushStyleColor(ImGuiCol_WindowBg, ImVec4(0.06f, 0.06f, 0.08f, 1.0f));

    ImGuiWindowFlags statusFlags =
        ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize |
        ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoScrollbar |
        ImGuiWindowFlags_NoDocking | ImGuiWindowFlags_NoNav;

    if (ImGui::Begin("##StatusBar", nullptr, statusFlags)) {
        ImGui::PushStyleColor(ImGuiCol_Text,
            ImVec4(NeonColors::TextDim[0], NeonColors::TextDim[1], NeonColors::TextDim[2], 1.0f));

        if (s_simRunning.load()) {
            ImGui::PushStyleColor(ImGuiCol_Text,
                ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
            std::string barStatus = GetSimStatus();
            ImGui::Text("Simulating... %.0f%%  |  %s", s_simPercent.load(), barStatus.c_str());
            ImGui::PopStyleColor();
        } else {
            ImGui::Text("I-Simpa // GPU Accelerated  |  OpenGL %s", glGetString(GL_VERSION));
        }
        ImGui::SameLine(ImGui::GetWindowWidth() - 180);
        ImGui::Text("Ctrl+P: Command Palette");
        ImGui::PopStyleColor();
    }
    ImGui::End();

    ImGui::PopStyleColor();
    ImGui::PopStyleVar();
}

// ─── Recent Files ───────────────────────────────────────────────────────────

void App::LoadRecentFiles() {
    m_recentFiles.clear();
    std::ifstream f("isimpa_recent.txt");
    if (!f.is_open()) return;
    std::string line;
    while (std::getline(f, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (!line.empty() && fs::exists(line)) m_recentFiles.push_back(line);
    }
}

void App::SaveRecentFiles() {
    std::ofstream f("isimpa_recent.txt");
    for (auto& path : m_recentFiles) f << path << "\n";
}

void App::AddRecentFile(const std::string& path) {
    // Remove if already in list, then push to front
    m_recentFiles.erase(
        std::remove(m_recentFiles.begin(), m_recentFiles.end(), path),
        m_recentFiles.end());
    m_recentFiles.insert(m_recentFiles.begin(), path);
    if (m_recentFiles.size() > 10) m_recentFiles.resize(10);
    SaveRecentFiles();
}

void App::UpdateWindowTitle() {
    Project& proj = GetProject();
    std::string title = "I-Simpa // Dark Neon";
    if (!proj.projectPath.empty()) {
        auto fname = fs::path(proj.projectPath).filename().string();
        title = fname + (m_dirty ? " *" : "") + " - I-Simpa // Dark Neon";
    } else if (proj.HasGeometry()) {
        title = std::string(m_dirty ? "* " : "") + "Untitled - I-Simpa // Dark Neon";
    }
    glfwSetWindowTitle(m_window, title.c_str());
}

void App::UpdateWorkflowStatus() {
    Project& proj = GetProject();

    // Pick up result directory from solver thread (thread-safe)
    {
        std::lock_guard<std::mutex> lock(s_simResultMutex);
        if (!s_simResultDir.empty()) {
            proj.lastResultDir = s_simResultDir;
            s_simResultDir.clear();
        }
    }

    // Room
    if (proj.HasGeometry()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Room, PhaseStatus::Ready);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Room, (int)proj.model.groups.size());
    } else {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Room, PhaseStatus::NotStarted);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Room, 0);
    }

    // Materials
    if (!proj.groupMaterialMap.empty()) {
        bool allAssigned = true;
        for (int i = 0; i < (int)proj.model.groups.size(); i++) {
            if (proj.groupMaterialMap.find(i) == proj.groupMaterialMap.end()) {
                allAssigned = false; break;
            }
        }
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Materials,
            allAssigned ? PhaseStatus::Ready : PhaseStatus::Incomplete);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Materials, (int)proj.groupMaterialMap.size());
    } else if (proj.HasGeometry()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Materials, PhaseStatus::Incomplete);
    }

    // Sources
    if (proj.HasSources()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Sources, PhaseStatus::Ready);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Sources, (int)proj.sources.size());
    } else {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Sources, PhaseStatus::NotStarted);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Sources, 0);
    }

    // Receivers
    if (proj.HasReceivers()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Receivers, PhaseStatus::Ready);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Receivers, (int)proj.punctualReceivers.size());
    } else {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Receivers, PhaseStatus::NotStarted);
        m_workflowRail.SetPhaseCount(WorkflowPhase::Receivers, 0);
    }

    // Mesh
    if (proj.meshGenerated) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Mesh, PhaseStatus::Ready);
    }

    // Simulate
    if (s_simRunning.load()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Simulate, PhaseStatus::Incomplete);
    } else if (!proj.lastResultDir.empty()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Simulate, PhaseStatus::Ready);
    }

    // Results
    if (!proj.lastResultDir.empty()) {
        m_workflowRail.SetPhaseStatus(WorkflowPhase::Results, PhaseStatus::Ready);
    }
}

} // namespace isimpa
