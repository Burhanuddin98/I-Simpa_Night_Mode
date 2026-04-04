#include "app/theme.h"
#include "project/project.h"
#include <imgui.h>
#include <vector>
#include <string>
#include <fstream>
#include <filesystem>
#include <cstdio>

#ifdef _WIN32
#include <windows.h>
#endif

namespace fs = std::filesystem;

namespace isimpa {

struct LogEntry {
    enum Level { Info, Warning, Error, Python };
    Level level;
    std::string text;
};

static std::vector<LogEntry> s_log;
static bool s_scrollToBottom = true;
static int s_activeTab = 0;

static void AddInitialLogs() {
    static bool initialized = false;
    if (initialized) return;
    initialized = true;
    s_log.push_back({LogEntry::Info, "[I-Simpa] Dark Neon GUI initialized"});
    s_log.push_back({LogEntry::Info, "[I-Simpa] GPU-accelerated viewport active"});
    s_log.push_back({LogEntry::Info, "[I-Simpa] Ctrl+P for command palette"});
}

// Public logging function
void ConsoleLog(const std::string& msg, int level) {
    LogEntry::Level lv = LogEntry::Info;
    if (level == 1) lv = LogEntry::Warning;
    else if (level == 2) lv = LogEntry::Error;
    else if (level == 3) lv = LogEntry::Python;
    s_log.push_back({lv, msg});
    s_scrollToBottom = true;
}

// ─── Python Execution via Subprocess ────────────────────────────────────────

static std::string FindPython() {
    // Try PATH-based Python discovery (no hardcoded user paths)
    const char* candidates[] = {
        "python", "python3", "py",
    };
    for (auto& c : candidates) {
#ifdef _WIN32
        std::string cmd = std::string(c) + " --version >nul 2>&1";
#else
        std::string cmd = std::string(c) + " --version >/dev/null 2>&1";
#endif
        if (system(cmd.c_str()) == 0) return c;
    }
    return "";
}

static std::string RunPythonCode(const std::string& code) {
#ifdef _WIN32
    static std::string pythonExe;
    if (pythonExe.empty()) pythonExe = FindPython();
    if (pythonExe.empty()) return "[Error] Python not found on this system";

    // Write code to temp file
    std::string tmpScript = "isimpa_temp_script.py";
    {
        std::ofstream f(tmpScript);
        // Inject helper to access project data
        f << "import json, os, sys\n";
        f << "def get_project():\n";
        f << "    if os.path.exists('isimpa_project.json'):\n";
        f << "        with open('isimpa_project.json') as f: return json.load(f)\n";
        f << "    return {}\n";
        f << "\n";
        f << code << "\n";
        f.close();
    }

    // Export project state to JSON for Python to read
    {
        Project& proj = GetProject();
        std::ofstream jf("isimpa_project.json");
        jf << "{\n";
        jf << "  \"num_vertices\": " << proj.model.vertices.size() << ",\n";
        jf << "  \"num_groups\": " << proj.model.groups.size() << ",\n";
        jf << "  \"num_sources\": " << proj.sources.size() << ",\n";
        jf << "  \"num_receivers\": " << proj.punctualReceivers.size() << ",\n";
        jf << "  \"temperature\": " << proj.environment.temperature << ",\n";
        jf << "  \"humidity\": " << proj.environment.humidity << ",\n";

        // JSON string escaper
        auto jsonEsc = [](const std::string& s) -> std::string {
            std::string out;
            for (char c : s) {
                if (c == '"') out += "\\\"";
                else if (c == '\\') out += "\\\\";
                else if (c == '\n') out += "\\n";
                else if (c == '\r') out += "\\r";
                else if (c == '\t') out += "\\t";
                else out += c;
            }
            return out;
        };

        // Sources
        jf << "  \"sources\": [";
        for (int i = 0; i < (int)proj.sources.size(); i++) {
            auto& s = proj.sources[i];
            if (i > 0) jf << ",";
            jf << "\n    {\"name\": \"" << jsonEsc(s.name) << "\", \"x\": " << s.position.x
               << ", \"y\": " << s.position.y << ", \"z\": " << s.position.z
               << ", \"power_db\": " << s.globalPowerDb << "}";
        }
        jf << "\n  ],\n";

        // Receivers
        jf << "  \"receivers\": [";
        for (int i = 0; i < (int)proj.punctualReceivers.size(); i++) {
            auto& r = proj.punctualReceivers[i];
            if (i > 0) jf << ",";
            jf << "\n    {\"name\": \"" << jsonEsc(r.name) << "\", \"x\": " << r.position.x
               << ", \"y\": " << r.position.y << ", \"z\": " << r.position.z << "}";
        }
        jf << "\n  ],\n";

        // Materials (with full per-band data)
        jf << "  \"materials\": [";
        for (int i = 0; i < (int)proj.materials.size(); i++) {
            auto& m = proj.materials[i];
            if (i > 0) jf << ",";
            jf << "\n    {\"name\": \"" << jsonEsc(m.name)
               << "\", \"avg_absorption\": " << m.AverageAbsorption()
               << ", \"resistivity\": " << m.resistivity
               << ", \"absorption\": [";
            for (int b = 0; b < 27; b++) { if (b > 0) jf << ","; jf << m.bands[b].absorption; }
            jf << "], \"diffusion\": [";
            for (int b = 0; b < 27; b++) { if (b > 0) jf << ","; jf << m.bands[b].diffusion; }
            jf << "]}";
        }
        jf << "\n  ],\n";

        // Groups with material assignments
        jf << "  \"groups\": [";
        for (int i = 0; i < (int)proj.model.groups.size(); i++) {
            auto& g = proj.model.groups[i];
            if (i > 0) jf << ",";
            int matIdx = -1;
            auto it = proj.groupMaterialMap.find(i);
            if (it != proj.groupMaterialMap.end()) matIdx = it->second;
            jf << "\n    {\"name\": \"" << jsonEsc(g.name) << "\", \"faces\": " << g.faces.size()
               << ", \"area\": " << g.surfaceArea << ", \"material_idx\": " << matIdx << "}";
        }
        jf << "\n  ],\n";

        // Encumbrances
        jf << "  \"encumbrances\": [";
        for (int i = 0; i < (int)proj.encumbrances.size(); i++) {
            auto& e = proj.encumbrances[i];
            if (i > 0) jf << ",";
            jf << "\n    {\"name\": \"" << jsonEsc(e.name) << "\", \"active\": " << (e.active ? "true" : "false")
               << ", \"box_min\": [" << e.boxMin.x << "," << e.boxMin.y << "," << e.boxMin.z
               << "], \"box_max\": [" << e.boxMax.x << "," << e.boxMax.y << "," << e.boxMax.z << "]}";
        }
        jf << "\n  ],\n";

        // Environment
        jf << "  \"sound_speed\": " << (331.3f + 0.606f * proj.environment.temperature) << ",\n";
        jf << "  \"pressure\": " << proj.environment.pressure << ",\n";

        // SPPS config
        jf << "  \"spps\": {\"particles\": " << proj.sppsConfig.particlesPerSource
           << ", \"time_step\": " << proj.sppsConfig.timeStep
           << ", \"sim_length\": " << proj.sppsConfig.simLength
           << ", \"receiver_radius\": " << proj.sppsConfig.receiverRadius << "},\n";

        // Last result dir
        jf << "  \"result_dir\": \"" << jsonEsc(proj.lastResultDir) << "\"\n";
        jf << "}\n";
        jf.close();
    }

    // Run Python and capture output
    std::string cmd = "\"" + pythonExe + "\" \"" + tmpScript + "\" 2>&1";

    SECURITY_ATTRIBUTES sa = {};
    sa.nLength = sizeof(sa);
    sa.bInheritHandle = TRUE;

    HANDLE hRead, hWrite;
    CreatePipe(&hRead, &hWrite, &sa, 0);
    SetHandleInformation(hRead, HANDLE_FLAG_INHERIT, 0);

    STARTUPINFOA si = {};
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = hWrite;
    si.hStdError = hWrite;

    PROCESS_INFORMATION pi = {};
    char cmdBuf[4096];
    strncpy(cmdBuf, cmd.c_str(), sizeof(cmdBuf) - 1);
    cmdBuf[sizeof(cmdBuf) - 1] = '\0';

    BOOL ok = CreateProcessA(nullptr, cmdBuf, nullptr, nullptr, TRUE,
                             CREATE_NO_WINDOW, nullptr, nullptr, &si, &pi);
    CloseHandle(hWrite);

    std::string output;
    if (ok) {
        char buf[4096];
        DWORD bytesRead;
        while (ReadFile(hRead, buf, sizeof(buf) - 1, &bytesRead, nullptr) && bytesRead > 0) {
            buf[bytesRead] = '\0';
            output += buf;
        }
        WaitForSingleObject(pi.hProcess, 5000); // 5s timeout
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
    } else {
        output = "[Error] Failed to launch Python";
    }
    CloseHandle(hRead);

    // Cleanup
    fs::remove(tmpScript);
    fs::remove("isimpa_project.json");

    // Trim trailing newline
    while (!output.empty() && (output.back() == '\n' || output.back() == '\r'))
        output.pop_back();

    return output;
#else
    return "[Error] Python execution not supported on this platform";
#endif
}

// ─── Console Panel ──────────────────────────────────────────────────────────

void DrawConsole() {
    if (!ImGui::Begin("Console")) {
        ImGui::End();
        return;
    }

    AddInitialLogs();

    if (ImGui::BeginTabBar("##ConsoleTabs")) {
        // Messages tab
        if (ImGui::BeginTabItem("Messages")) {
            s_activeTab = 0;

            float footerHeight = ImGui::GetFrameHeightWithSpacing();
            ImGui::BeginChild("##LogRegion", ImVec2(0, -footerHeight), ImGuiChildFlags_None);

            for (auto& entry : s_log) {
                ImVec4 col;
                switch (entry.level) {
                    case LogEntry::Info:
                        col = ImVec4(NeonColors::TextNormal[0], NeonColors::TextNormal[1],
                                     NeonColors::TextNormal[2], 1.0f); break;
                    case LogEntry::Warning:
                        col = ImVec4(NeonColors::Yellow[0], NeonColors::Yellow[1],
                                     NeonColors::Yellow[2], 1.0f); break;
                    case LogEntry::Error:
                        col = ImVec4(NeonColors::Red[0], NeonColors::Red[1],
                                     NeonColors::Red[2], 1.0f); break;
                    case LogEntry::Python:
                        col = ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1],
                                     NeonColors::Cyan[2], 1.0f); break;
                }
                ImGui::PushStyleColor(ImGuiCol_Text, col);
                ImGui::TextWrapped("%s", entry.text.c_str());
                ImGui::PopStyleColor();
            }

            if (s_scrollToBottom) ImGui::SetScrollHereY(1.0f);
            s_scrollToBottom = false;

            ImGui::EndChild();

            if (ImGui::Button("Clear")) { s_log.clear(); }
            ImGui::SameLine();
            if (ImGui::Button("Export")) {
                std::ofstream f("isimpa_console.log");
                for (auto& entry : s_log) f << entry.text << "\n";
                f.close();
                ConsoleLog("[Console] Exported to isimpa_console.log", 0);
            }
            ImGui::SameLine();
            ImGui::Text("%zu messages", s_log.size());

            ImGui::EndTabItem();
        }

        // Python tab
        if (ImGui::BeginTabItem("Python")) {
            s_activeTab = 1;

            ImGui::PushStyleColor(ImGuiCol_Text,
                ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 0.7f));
            ImGui::TextWrapped("Python console. Use get_project() to access project data as dict.");
            ImGui::PopStyleColor();

            // Show Python history and output
            float footerHeight = ImGui::GetFrameHeightWithSpacing() * 2;
            ImGui::BeginChild("##PythonLog", ImVec2(0, -footerHeight), ImGuiChildFlags_None);
            for (auto& entry : s_log) {
                if (entry.level != LogEntry::Python) continue;
                ImGui::PushStyleColor(ImGuiCol_Text,
                    ImVec4(NeonColors::Cyan[0], NeonColors::Cyan[1], NeonColors::Cyan[2], 1.0f));
                ImGui::TextWrapped("%s", entry.text.c_str());
                ImGui::PopStyleColor();
            }
            if (s_scrollToBottom) ImGui::SetScrollHereY(1.0f);
            ImGui::EndChild();

            // Input field
            static char pythonBuf[4096] = {};
            ImGui::PushStyleColor(ImGuiCol_FrameBg, ImVec4(0.08f, 0.08f, 0.11f, 1.0f));
            ImGui::PushItemWidth(-60);
            bool exec = ImGui::InputTextMultiline("##PythonInput", pythonBuf, sizeof(pythonBuf),
                ImVec2(-60, ImGui::GetFrameHeightWithSpacing()),
                ImGuiInputTextFlags_CtrlEnterForNewLine);
            ImGui::PopItemWidth();
            ImGui::PopStyleColor();

            ImGui::SameLine();
            if (ImGui::Button("Run", ImVec2(50, ImGui::GetFrameHeightWithSpacing()))) exec = true;

            if (exec && pythonBuf[0] != '\0') {
                std::string code(pythonBuf);
                s_log.push_back({LogEntry::Python, ">>> " + code});
                s_scrollToBottom = true;

                std::string result = RunPythonCode(code);
                if (!result.empty()) {
                    s_log.push_back({LogEntry::Python, result});
                }
                pythonBuf[0] = '\0';
            }

            ImGui::EndTabItem();
        }

        ImGui::EndTabBar();
    }

    ImGui::End();
}

} // namespace isimpa
