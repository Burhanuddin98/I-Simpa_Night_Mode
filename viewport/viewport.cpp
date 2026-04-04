#include "viewport/viewport.h"
#include "viewport/camera.h"
#include "app/theme.h"
#include "app/selection.h"
#include "project/project.h"
#include "project/result_parser.h"

#include <glad/glad.h>
#include <imgui.h>
#include <glm/glm.hpp>
#include <vector>
#include <set>
#include <fstream>
#include <filesystem>
#include <cmath>
#include <algorithm>
#include <unordered_map>

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif
namespace fs = std::filesystem;
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>

namespace isimpa {

// Forward declarations for colormap legend data (defined in results.cpp)
float GetColormapMin();
float GetColormapMax();
const char* GetColormapName();
const SimulationResults& GetSimResults();

// ── Grid shader ─────────────────────────────────────────────────────────────

static const char* gridVertSrc = R"(
#version 430 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec4 aColor;
uniform mat4 uViewProj;
out vec4 vColor;
void main() {
    vColor = aColor;
    gl_Position = uViewProj * vec4(aPos, 1.0);
}
)";

static const char* gridFragSrc = R"(
#version 430 core
in vec4 vColor;
out vec4 FragColor;
void main() {
    FragColor = vColor;
}
)";

// ── State ───────────────────────────────────────────────────────────────────

static GLuint g_gridVAO = 0, g_gridVBO = 0, g_gridShader = 0;
static int    g_gridVertCount = 0;
static GLuint g_fbo = 0, g_fboColor = 0, g_fboDepth = 0;
static int    g_fboWidth = 0, g_fboHeight = 0;
static Camera g_camera;

// Scene model + GPU mesh
static SceneModel g_model;
static GPUMesh    g_gpuMesh;
static GLuint     g_meshShader = 0;
static GLuint     g_wireShader = 0;
static bool       g_showWireframe = true;
static bool       g_showFaces = true;
static int        g_faceMode = 0; // 0=both sides, 1=outside only, 2=inside only

// Measurement mode globals
bool g_viewportMeasureMode = false;
int  g_viewportMeasureState = 0;
glm::vec3 g_viewportMeasureA{0}, g_viewportMeasureB{0};

// Icon rendering (sources/receivers as glowing spheres)
static GLuint g_iconShader = 0;
static GLuint g_iconVAO = 0;
static GLuint g_iconVBO = 0;

static const char* iconVertSrc = R"(
#version 430 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec4 aColor;
layout(location = 2) in float aSize;
uniform mat4 uViewProj;
uniform vec2 uViewportSize;
out vec4 vColor;
void main() {
    vColor = aColor;
    gl_Position = uViewProj * vec4(aPos, 1.0);
    // Scale point size by distance
    float dist = gl_Position.w;
    gl_PointSize = clamp(aSize * uViewportSize.y / (dist * 2.0), 4.0, 60.0);
}
)";

static const char* iconFragSrc = R"(
#version 430 core
in vec4 vColor;
out vec4 FragColor;
void main() {
    // Circular point with soft glow edge
    vec2 coord = gl_PointCoord * 2.0 - 1.0;
    float r = dot(coord, coord);
    if (r > 1.0) discard;
    float alpha = 1.0 - smoothstep(0.3, 1.0, r);
    // Bright core + soft glow
    float core = smoothstep(0.5, 0.0, r);
    vec3 col = mix(vColor.rgb, vec3(1.0), core * 0.5);
    FragColor = vec4(col, vColor.a * alpha);
}
)";

struct IconVertex { float x, y, z, r, g, b, a, size; };

static GLuint CompileShader(GLenum type, const char* src);  // forward decl

static void InitIcons() {
    GLuint vs = CompileShader(GL_VERTEX_SHADER, iconVertSrc);
    GLuint fs = CompileShader(GL_FRAGMENT_SHADER, iconFragSrc);
    g_iconShader = glCreateProgram();
    glAttachShader(g_iconShader, vs);
    glAttachShader(g_iconShader, fs);
    glLinkProgram(g_iconShader);
    {
        int ok;
        glGetProgramiv(g_iconShader, GL_LINK_STATUS, &ok);
        if (!ok) {
            char log[512];
            glGetProgramInfoLog(g_iconShader, 512, nullptr, log);
            fprintf(stderr, "[Icon Shader Link] %s\n", log);
        }
    }
    glDeleteShader(vs);
    glDeleteShader(fs);

    glGenVertexArrays(1, &g_iconVAO);
    glGenBuffers(1, &g_iconVBO);
    glBindVertexArray(g_iconVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_iconVBO);
    // pos(3) + color(4) + size(1) = 8 floats
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(IconVertex), (void*)0);
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(IconVertex), (void*)(3*sizeof(float)));
    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 1, GL_FLOAT, GL_FALSE, sizeof(IconVertex), (void*)(7*sizeof(float)));
    glBindVertexArray(0);
}

static void DrawIcons(const glm::mat4& viewProj, float vpWidth, float vpHeight) {
    Project& proj = GetProject();
    std::vector<IconVertex> icons;

    // Sources: orange glowing spheres
    for (auto& src : proj.sources) {
        float a = src.active ? 1.0f : 0.3f;
        icons.push_back({src.position.x, src.position.y, src.position.z,
                         src.displayColor.r, src.displayColor.g, src.displayColor.b, a, 0.5f});
    }

    // Punctual receivers: green glowing spheres
    for (auto& rcv : proj.punctualReceivers) {
        icons.push_back({rcv.position.x, rcv.position.y, rcv.position.z,
                         rcv.displayColor.r, rcv.displayColor.g, rcv.displayColor.b, 1.0f, 0.4f});
    }

    if (icons.empty()) return;

    glUseProgram(g_iconShader);
    glUniformMatrix4fv(glGetUniformLocation(g_iconShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
    glUniform2f(glGetUniformLocation(g_iconShader, "uViewportSize"), vpWidth, vpHeight);

    glBindVertexArray(g_iconVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_iconVBO);
    glBufferData(GL_ARRAY_BUFFER, icons.size() * sizeof(IconVertex), icons.data(), GL_DYNAMIC_DRAW);

    glEnable(GL_PROGRAM_POINT_SIZE);
    glDisable(GL_DEPTH_TEST); // Draw on top
    glDrawArrays(GL_POINTS, 0, (int)icons.size());
    glEnable(GL_DEPTH_TEST);
    glDisable(GL_PROGRAM_POINT_SIZE);

    glBindVertexArray(0);
}

// ── Surface Receiver Grid Rendering ─────────────────────────────────────────

static GLuint g_surfRecVAO = 0;
static GLuint g_surfRecVBO = 0;

static void InitSurfRecRenderer() {
    glGenVertexArrays(1, &g_surfRecVAO);
    glGenBuffers(1, &g_surfRecVBO);
    glBindVertexArray(g_surfRecVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_surfRecVBO);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(glm::vec3), (void*)0);
    glBindVertexArray(0);
}

static void DrawSurfaceReceivers(const glm::mat4& viewProj) {
    Project& proj = GetProject();
    std::vector<glm::vec3> lines;

    for (auto& sr : proj.surfaceReceivers) {
        if (sr.type != SurfaceReceiver::Plane) continue;

        glm::vec3 A = sr.vertexA;
        glm::vec3 B = sr.vertexB;
        glm::vec3 C = sr.vertexC;
        glm::vec3 D = A + (C - B); // Fourth corner: A + BC vector

        // Edge AB direction and edge AC direction
        glm::vec3 edgeAB = B - A;
        glm::vec3 edgeAC = C - B; // B->C direction, used as the second axis
        // Actually: the plane is A, B, C where AB is one edge and BC is perpendicular-ish
        // The grid spans from A to D = A + (B-A) + (C-B) = C + (A-B) ...
        // Let's just define it properly:
        // u-axis: A -> B
        // v-axis: B -> C
        // Grid corners: A, B, C, A+(C-B)
        glm::vec3 u = B - A;
        glm::vec3 v = C - B;
        D = A + u + v;

        float uLen = glm::length(u);
        float vLen = glm::length(v);
        if (uLen < 0.01f || vLen < 0.01f) continue;

        float res = sr.gridResolution;
        if (res < 0.01f) res = 0.5f;

        int uSteps = std::max(1, (int)(uLen / res));
        int vSteps = std::max(1, (int)(vLen / res));

        glm::vec3 uDir = u / (float)uSteps;
        glm::vec3 vDir = v / (float)vSteps;

        // Grid lines along u
        for (int j = 0; j <= vSteps; j++) {
            glm::vec3 start = A + vDir * (float)j;
            glm::vec3 end = start + u;
            lines.push_back(start);
            lines.push_back(end);
        }
        // Grid lines along v
        for (int i = 0; i <= uSteps; i++) {
            glm::vec3 start = A + uDir * (float)i;
            glm::vec3 end = start + v;
            lines.push_back(start);
            lines.push_back(end);
        }

        // Outline (bright)
        lines.push_back(A); lines.push_back(B);
        lines.push_back(B); lines.push_back(C + (A - A)); // B -> C
        // Actually draw the full quad outline
        glm::vec3 corners[4] = {A, B, B + v, A + v};
        for (int i = 0; i < 4; i++) {
            lines.push_back(corners[i]);
            lines.push_back(corners[(i+1)%4]);
        }
    }

    if (lines.empty()) return;

    glUseProgram(g_wireShader);
    glUniformMatrix4fv(glGetUniformLocation(g_wireShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
    glUniform4f(glGetUniformLocation(g_wireShader, "uColor"), 0.2f, 0.92f, 0.4f, 0.5f); // Green grid

    glBindVertexArray(g_surfRecVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_surfRecVBO);
    glBufferData(GL_ARRAY_BUFFER, lines.size() * sizeof(glm::vec3), lines.data(), GL_DYNAMIC_DRAW);
    glDrawArrays(GL_LINES, 0, (int)lines.size());
    glBindVertexArray(0);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

static GLuint CompileShader(GLenum type, const char* src) {
    GLuint s = glCreateShader(type);
    glShaderSource(s, 1, &src, nullptr);
    glCompileShader(s);
    int ok;
    glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
    if (!ok) {
        char log[512];
        glGetShaderInfoLog(s, 512, nullptr, log);
        fprintf(stderr, "[Shader Compile] %s\n", log);
        glDeleteShader(s);
        return 0;
    }
    return s;
}

static GLuint CreateProgram(const char* vertSrc, const char* fragSrc) {
    GLuint vs = CompileShader(GL_VERTEX_SHADER, vertSrc);
    GLuint fs = CompileShader(GL_FRAGMENT_SHADER, fragSrc);
    if (!vs || !fs) {
        if (vs) glDeleteShader(vs);
        if (fs) glDeleteShader(fs);
        fprintf(stderr, "[Shader] Program creation aborted due to compile failure\n");
        return 0;
    }
    GLuint prog = glCreateProgram();
    glAttachShader(prog, vs);
    glAttachShader(prog, fs);
    glLinkProgram(prog);
    int ok;
    glGetProgramiv(prog, GL_LINK_STATUS, &ok);
    if (!ok) {
        char log[512];
        glGetProgramInfoLog(prog, 512, nullptr, log);
        fprintf(stderr, "[Shader Link] %s\n", log);
        glDeleteProgram(prog);
        prog = 0;
    }
    glDeleteShader(vs);
    glDeleteShader(fs);
    return prog;
}

static void BuildGrid(float extent, float step) {
    struct Vertex { float x, y, z, r, g, b, a; };
    std::vector<Vertex> verts;

    for (float i = -extent; i <= extent; i += step) {
        float alpha = (i == 0.0f) ? 0.5f : 0.15f;
        float r = (i == 0.0f) ? 0.0f : 0.25f;
        float g = (i == 0.0f) ? 0.0f : 0.25f;
        float b = (i == 0.0f) ? 0.9f : 0.32f;
        verts.push_back({i, 0, -extent, r, g, b, alpha});
        verts.push_back({i, 0,  extent, r, g, b, alpha});

        r = (i == 0.0f) ? 0.9f : 0.25f;
        g = (i == 0.0f) ? 0.0f : 0.25f;
        b = (i == 0.0f) ? 0.0f : 0.32f;
        verts.push_back({-extent, 0, i, r, g, b, alpha});
        verts.push_back({ extent, 0, i, r, g, b, alpha});
    }
    verts.push_back({0, 0, 0, 0.0f, 0.9f, 0.0f, 0.5f});
    verts.push_back({0, extent, 0, 0.0f, 0.9f, 0.0f, 0.5f});

    g_gridVertCount = (int)verts.size();
    glGenVertexArrays(1, &g_gridVAO);
    glGenBuffers(1, &g_gridVBO);
    glBindVertexArray(g_gridVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_gridVBO);
    glBufferData(GL_ARRAY_BUFFER, verts.size() * sizeof(Vertex), verts.data(), GL_STATIC_DRAW);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)0);
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)(3 * sizeof(float)));
    glBindVertexArray(0);
}

static void EnsureFBO(int w, int h) {
    if (w == g_fboWidth && h == g_fboHeight && g_fbo != 0) return;
    if (w <= 0 || h <= 0) return;
    if (g_fbo) {
        glDeleteFramebuffers(1, &g_fbo);
        glDeleteTextures(1, &g_fboColor);
        glDeleteRenderbuffers(1, &g_fboDepth);
    }
    g_fboWidth = w; g_fboHeight = h;
    glGenFramebuffers(1, &g_fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, g_fbo);
    glGenTextures(1, &g_fboColor);
    glBindTexture(GL_TEXTURE_2D, g_fboColor);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, g_fboColor, 0);
    glGenRenderbuffers(1, &g_fboDepth);
    glBindRenderbuffer(GL_RENDERBUFFER, g_fboDepth);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, w, h);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_RENDERBUFFER, g_fboDepth);
    GLenum status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
    if (status != GL_FRAMEBUFFER_COMPLETE) {
        fprintf(stderr, "[FBO] Framebuffer incomplete: 0x%X\n", status);
        // Clean up the broken FBO to avoid rendering to invalid target
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
        glDeleteFramebuffers(1, &g_fbo); g_fbo = 0;
        glDeleteTextures(1, &g_fboColor); g_fboColor = 0;
        glDeleteRenderbuffers(1, &g_fboDepth); g_fboDepth = 0;
        return;
    }
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
}

// ── Public API ──────────────────────────────────────────────────────────────

void InitViewport() {
    g_gridShader = CreateProgram(gridVertSrc, gridFragSrc);
    g_meshShader = CreateMeshShader();
    g_wireShader = CreateWireShader();
    BuildGrid(20.0f, 1.0f);

    g_camera.SetPosition(glm::vec3(8.0f, 6.0f, 8.0f));
    g_camera.SetTarget(glm::vec3(0.0f, 0.0f, 0.0f));

    InitIcons();
    InitSurfRecRenderer();

    // Start with a default room pre-configured for immediate use
    {
        Project& proj = GetProject();
        proj.CreateDefaultRoom(6.0f, 10.0f, 3.0f);

        // Check for previous simulation results next to exe
        std::string exeDir;
#ifdef _WIN32
        char exeBuf[MAX_PATH] = {};
        GetModuleFileNameA(nullptr, exeBuf, MAX_PATH);
        exeDir = std::string(exeBuf);
        auto sp = exeDir.find_last_of("\\/");
        if (sp != std::string::npos) exeDir = exeDir.substr(0, sp + 1);
#endif
        std::string resultsDir = exeDir + "sim_output";
        if (fs::exists(resultsDir)) {
            for (auto& entry : fs::directory_iterator(resultsDir)) {
                if (!entry.is_directory()) continue;
                std::string d = entry.path().string();
                bool hasResults = false;
                for (auto& f : fs::directory_iterator(d)) {
                    auto ext = f.path().extension().string();
                    if (ext == ".gabe" || ext == ".recp") { hasResults = true; break; }
                }
                if (hasResults) {
                    proj.lastResultDir = d;
                    printf("[Init] Found previous results in: %s\n", d.c_str());
                    break;
                }
            }
        }

        // Pre-configure materials: Floor=Carpet, Ceiling+Walls=Plaster
        proj.AssignMaterial(0, 3);  // Floor -> Carpet
        proj.AssignMaterial(1, 6);  // Ceiling -> Plaster
        proj.AssignMaterial(2, 6);  // Wall Front -> Plaster
        proj.AssignMaterial(3, 6);  // Wall Back -> Plaster
        proj.AssignMaterial(4, 6);  // Wall Left -> Plaster
        proj.AssignMaterial(5, 6);  // Wall Right -> Plaster

        // Source at front of room
        auto& src = proj.AddSource();
        src.name = "Speaker";
        src.position = glm::vec3(3.0f, 1.5f, 2.0f);
        src.globalPowerDb = 90.0f;

        // Receivers in audience area
        auto& r1 = proj.AddReceiver();
        r1.name = "R1 (near)";
        r1.position = glm::vec3(3.0f, 1.2f, 5.0f);

        auto& r2 = proj.AddReceiver();
        r2.name = "R2 (far)";
        r2.position = glm::vec3(3.0f, 1.2f, 8.0f);

        // Surface receiver: horizontal plane at ear height covering room floor
        auto& sr = proj.AddSurfaceReceiver();
        sr.name = "Floor Map";
        sr.type = SurfaceReceiver::Plane;
        sr.vertexA = glm::vec3(0.5f, 1.2f, 0.5f);
        sr.vertexB = glm::vec3(5.5f, 1.2f, 0.5f);
        sr.vertexC = glm::vec3(5.5f, 1.2f, 9.5f);
        sr.gridResolution = 0.5f;
    }
}

void ViewportLoadModel(const SceneModel& model) {
    // Sync: viewport's GPU copy AND the project's model must match
    g_model = model;
    GetProject().model = model; // keep project in sync
    g_gpuMesh.Upload(g_model);
    ViewportFocusModel();
}

void ViewportRefreshGPU() {
    // Re-upload from project model (e.g. after material color change or selection change)
    g_model = GetProject().model;
    g_gpuMesh.Upload(g_model, GetSelection().selectedGroups);
}

void ViewportFocusModel() {
    if (g_model.IsEmpty()) return;
    g_camera.SetTarget(g_model.center);
    g_camera.SetPosition(g_model.center + glm::vec3(g_model.extent * 1.5f, g_model.extent * 1.0f, g_model.extent * 1.5f));
}

SceneModel& GetActiveModel() { return g_model; }
GPUMesh& GetActiveGPUMesh() { return g_gpuMesh; }

// Forward declarations for colormap/particles/clip (defined below DrawViewport)
static GLuint g_colormapVAO = 0, g_colormapVBO = 0;
static int    g_colormapVertCount = 0;
static bool   g_hasColormap = false;
// Iso-contour lines for surface colormap
static GLuint g_isoVAO = 0, g_isoVBO = 0;
static int    g_isoVertCount = 0;
static bool   g_hasIsoContours = false;
static ParticleData g_particleData;
static int g_particleTimeStep = 0;
static bool g_hasParticles = false;
static void DrawParticles(const glm::mat4& viewProj, float vpWidth, float vpHeight);
static bool  g_clipEnabled = false;
static int   g_clipAxis = 1;
static float g_clipValue = 1.2f;

// ── Intensity Vector Arrows ────────────────────────────────────────────────

static void DrawIntensityArrows(const glm::mat4& viewProj) {
    Project& proj = GetProject();
    const SimulationResults& simRes = GetSimResults();

    if (proj.sources.empty() || proj.punctualReceivers.empty()) return;
    if (!simRes.loaded) return;

    std::vector<glm::vec3> lines;

    for (int ri = 0; ri < (int)proj.punctualReceivers.size(); ri++) {
        auto& rcv = proj.punctualReceivers[ri];

        // Get SPL for this receiver from simulation results (if available)
        float spl = 0.0f;
        if (ri < (int)simRes.receivers.size()) {
            spl = simRes.receivers[ri].totalSplDb;
        }
        if (spl < 1.0f) continue; // skip silent receivers

        // Find nearest active source
        float minDist = 1e30f;
        glm::vec3 nearestSrcPos = rcv.position;
        bool foundSource = false;
        for (auto& src : proj.sources) {
            if (!src.active) continue;
            float d = glm::length(src.position - rcv.position);
            if (d < minDist) {
                minDist = d;
                nearestSrcPos = src.position;
                foundSource = true;
            }
        }
        if (!foundSource || minDist < 0.01f) continue;

        // Arrow direction: receiver toward source
        glm::vec3 dir = glm::normalize(nearestSrcPos - rcv.position);

        // Arrow length proportional to SPL (normalized: 40-100 dB -> 0.2-1.5 m)
        float normalizedSpl = glm::clamp((spl - 40.0f) / 60.0f, 0.0f, 1.0f);
        float arrowLen = 0.2f + normalizedSpl * 1.3f;

        glm::vec3 tip = rcv.position + dir * arrowLen;

        // Main line
        lines.push_back(rcv.position);
        lines.push_back(tip);

        // Arrowhead (two small lines)
        glm::vec3 right = glm::normalize(glm::cross(dir, glm::vec3(0, 1, 0)));
        if (glm::length(right) < 0.01f)
            right = glm::normalize(glm::cross(dir, glm::vec3(1, 0, 0)));
        float headLen = arrowLen * 0.2f;
        lines.push_back(tip);
        lines.push_back(tip - dir * headLen + right * headLen * 0.5f);
        lines.push_back(tip);
        lines.push_back(tip - dir * headLen - right * headLen * 0.5f);
    }

    if (lines.empty()) return;

    glUseProgram(g_wireShader);
    glUniformMatrix4fv(glGetUniformLocation(g_wireShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
    glUniform4f(glGetUniformLocation(g_wireShader, "uColor"), 0.3f, 0.3f, 1.0f, 0.9f); // Blue
    glLineWidth(3.0f);

    glBindVertexArray(g_surfRecVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_surfRecVBO);
    glBufferData(GL_ARRAY_BUFFER, lines.size() * sizeof(glm::vec3), lines.data(), GL_DYNAMIC_DRAW);
    glDrawArrays(GL_LINES, 0, (int)lines.size());
    glBindVertexArray(0);
    glLineWidth(1.0f);
}

void DrawViewport(float width, float height) {
    if (width < 1 || height < 1) return;
    int w = (int)width, h = (int)height;

    // Auto-refresh GPU mesh when selection changes
    static std::set<int> s_lastSelectedGroups;
    auto& curSel = GetSelection().selectedGroups;
    if (curSel != s_lastSelectedGroups) {
        s_lastSelectedGroups = curSel;
        g_model = GetProject().model;
        g_gpuMesh.Upload(g_model, curSel);
    }

    // ── Camera & interaction input ─────────────────────────────────────────
    bool viewportHovered = ImGui::IsWindowHovered();
    bool viewportFocused = ImGui::IsWindowFocused();

    if (viewportHovered) {
        ImGuiIO& io = ImGui::GetIO();

        // ── Mouse & Touchpad Controls ──────────────────────────────────────
        //
        // TOUCHPAD:
        //   Two-finger scroll       = ZOOM  (natural, like web/maps)
        //   Left-click drag         = ORBIT (single finger drag on touchpad)
        //   Shift + left-drag       = PAN
        //
        // MOUSE:
        //   Scroll wheel            = ZOOM
        //   Right-drag              = ORBIT
        //   Middle-drag             = PAN
        //   Shift + right-drag      = PAN
        //   Alt + left-drag         = ORBIT (Blender)
        //
        // Left-click (no drag) = SELECT

        // Track whether left mouse was dragged (for click-vs-drag detection)
        static bool s_leftWasDragged = false;
        if (ImGui::IsMouseClicked(ImGuiMouseButton_Left))
            s_leftWasDragged = false;

        // Zoom: scroll (two-finger on touchpad, wheel on mouse)
        g_camera.ProcessMouseScroll(io.MouseWheel);

        // Left-drag: orbit on touchpad (only if actually dragging, not just clicking)
        if (ImGui::IsMouseDragging(ImGuiMouseButton_Left, 3.0f) && !io.KeyShift && !io.KeyAlt && !io.KeyCtrl)
            g_camera.ProcessMouseOrbit(io.MouseDelta.x, io.MouseDelta.y);

        // Shift + left-drag: pan on touchpad
        if (io.KeyShift && ImGui::IsMouseDragging(ImGuiMouseButton_Left))
            g_camera.ProcessMousePan(io.MouseDelta.x, io.MouseDelta.y);

        // Right-drag: PAN (move scene without rotating — what users expect)
        if (ImGui::IsMouseDragging(ImGuiMouseButton_Right))
            g_camera.ProcessMousePan(io.MouseDelta.x, io.MouseDelta.y);

        // Middle-drag: also pan
        if (ImGui::IsMouseDragging(ImGuiMouseButton_Middle))
            g_camera.ProcessMousePan(io.MouseDelta.x, io.MouseDelta.y);

        // Alt+left-drag: orbit (Blender users)
        if (io.KeyAlt && ImGui::IsMouseDragging(ImGuiMouseButton_Left))
            g_camera.ProcessMouseOrbit(io.MouseDelta.x, io.MouseDelta.y);

        // WASD fly-through (when right mouse is held OR viewport is focused)
        if (ImGui::IsMouseDown(ImGuiMouseButton_Right) || viewportFocused) {
            bool wKey = ImGui::IsKeyDown(ImGuiKey_W);
            bool sKey = ImGui::IsKeyDown(ImGuiKey_S);
            bool aKey = ImGui::IsKeyDown(ImGuiKey_A);
            bool dKey = ImGui::IsKeyDown(ImGuiKey_D);
            bool qKey = ImGui::IsKeyDown(ImGuiKey_Q) || ImGui::IsKeyDown(ImGuiKey_Space);
            bool eKey = ImGui::IsKeyDown(ImGuiKey_E) || io.KeyCtrl;
            bool sprint = io.KeyShift;

            if (wKey || sKey || aKey || dKey || qKey || eKey) {
                g_camera.ProcessKeyboard(io.DeltaTime, wKey, sKey, aKey, dKey, qKey, eKey, sprint);
            }
        }

        // Double-click: focus camera on clicked point
        if (ImGui::IsMouseDoubleClicked(ImGuiMouseButton_Left) && !io.KeyAlt) {
            ImVec2 mousePos = ImGui::GetMousePos();
            float localX = mousePos.x - ImGui::GetWindowPos().x;
            float localY = mousePos.y - ImGui::GetWindowPos().y - ImGui::GetFrameHeight();
            glm::vec3 hitPoint = ViewportScreenToWorld(localX, localY);
            g_camera.FocusOnPoint(hitPoint);
        }

        // Single left-click (released without dragging): face picking / measurement
        // Uses MouseReleased so it doesn't conflict with left-drag orbit
        if (ImGui::IsMouseDragging(ImGuiMouseButton_Left, 3.0f))
            s_leftWasDragged = true;
        if (ImGui::IsMouseReleased(ImGuiMouseButton_Left) && !s_leftWasDragged && !io.KeyAlt && !io.KeyShift) {
            ImVec2 mousePos = ImGui::GetMousePos();
            float localX = mousePos.x - ImGui::GetWindowPos().x;
            float localY = mousePos.y - ImGui::GetWindowPos().y - ImGui::GetFrameHeight();

            if (g_viewportMeasureMode && (g_viewportMeasureState == 1 || g_viewportMeasureState == 2)) {
                glm::vec3 worldPos = ViewportScreenToWorld(localX, localY);
                if (g_viewportMeasureState == 1) {
                    g_viewportMeasureA = worldPos;
                    g_viewportMeasureState = 2;
                } else {
                    g_viewportMeasureB = worldPos;
                    g_viewportMeasureState = 3;
                    g_viewportMeasureMode = false;
                }
            } else {
                int hitGroup = ViewportPickFace(localX, localY);
                if (hitGroup >= 0) {
                    if (io.KeyShift) {
                        // Shift+click: add/remove from multi-selection
                        GetSelection().ToggleGroup(hitGroup);
                    } else if (GetSelection().IsGroupSelected(hitGroup)) {
                        // Click already-selected surface: deselect it
                        GetSelection().Clear();
                    } else {
                        // Click new surface: select it
                        GetSelection().SelectGroup(hitGroup);
                    }
                    ViewportRefreshGPU();
                } else {
                    // Clicked empty space — clear selection
                    GetSelection().Clear();
                    ViewportRefreshGPU();
                }
            }
        }
    }

    // Render to FBO
    EnsureFBO(w, h);
    glBindFramebuffer(GL_FRAMEBUFFER, g_fbo);
    glViewport(0, 0, w, h);
    glClearColor(0.06f, 0.06f, 0.08f, 1.0f);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    glEnable(GL_DEPTH_TEST);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    glm::mat4 view = g_camera.GetViewMatrix();
    glm::mat4 proj = glm::perspective(glm::radians(45.0f), width / height,
        std::max(0.01f, g_model.extent * 0.001f),
        std::max(500.0f, g_model.extent * 20.0f));
    glm::mat4 viewProj = proj * view;

    // Draw grid
    glUseProgram(g_gridShader);
    glUniformMatrix4fv(glGetUniformLocation(g_gridShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
    glBindVertexArray(g_gridVAO);
    glDrawArrays(GL_LINES, 0, g_gridVertCount);
    glBindVertexArray(0);

    // Draw scene mesh (solid faces) with face culling mode
    if (g_showFaces && g_gpuMesh.indexCount > 0) {
        glUseProgram(g_meshShader);
        glUniformMatrix4fv(glGetUniformLocation(g_meshShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
        glUniform3f(glGetUniformLocation(g_meshShader, "uLightDir"), 0.3f, 0.8f, 0.5f);
        glUniform3fv(glGetUniformLocation(g_meshShader, "uCameraPos"), 1, glm::value_ptr(g_camera.GetPosition()));
        glUniform1f(glGetUniformLocation(g_meshShader, "uAmbient"), 0.25f);
        glUniform1f(glGetUniformLocation(g_meshShader, "uAlpha"), 0.0f);
        if (g_faceMode == 1) { glEnable(GL_CULL_FACE); glCullFace(GL_BACK); }
        else if (g_faceMode == 2) { glEnable(GL_CULL_FACE); glCullFace(GL_FRONT); }
        g_gpuMesh.Draw(g_meshShader, viewProj, false);
        glDisable(GL_CULL_FACE);
    }

    // Draw wireframe overlay (neon edges)
    if (g_showWireframe && g_gpuMesh.wireVertCount > 0) {
        glUseProgram(g_wireShader);
        glUniformMatrix4fv(glGetUniformLocation(g_wireShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
        glUniform4f(glGetUniformLocation(g_wireShader, "uColor"), 0.75f, 0.10f, 0.15f, 0.25f);
        glEnable(GL_POLYGON_OFFSET_LINE);
        glPolygonOffset(-1.0f, -1.0f);
        g_gpuMesh.Draw(g_wireShader, viewProj, true);
        glDisable(GL_POLYGON_OFFSET_LINE);
    }

    // Draw surface receiver grids
    DrawSurfaceReceivers(viewProj);

    // Draw encumbrance boxes (translucent wireframe cubes)
    {
        Project& proj = GetProject();
        if (!proj.encumbrances.empty()) {
            std::vector<glm::vec3> encLines;
            for (auto& enc : proj.encumbrances) {
                if (!enc.active || enc.type != Encumbrance::Cuboid) continue;
                glm::vec3 mn = enc.boxMin, mx = enc.boxMax;
                // 12 edges of the box
                glm::vec3 corners[8] = {
                    {mn.x, mn.y, mn.z}, {mx.x, mn.y, mn.z},
                    {mx.x, mn.y, mx.z}, {mn.x, mn.y, mx.z},
                    {mn.x, mx.y, mn.z}, {mx.x, mx.y, mn.z},
                    {mx.x, mx.y, mx.z}, {mn.x, mx.y, mx.z},
                };
                int edges[12][2] = {
                    {0,1},{1,2},{2,3},{3,0}, // bottom
                    {4,5},{5,6},{6,7},{7,4}, // top
                    {0,4},{1,5},{2,6},{3,7}, // verticals
                };
                for (auto& e : edges) {
                    encLines.push_back(corners[e[0]]);
                    encLines.push_back(corners[e[1]]);
                }
            }
            if (!encLines.empty()) {
                glUseProgram(g_wireShader);
                glUniformMatrix4fv(glGetUniformLocation(g_wireShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
                glUniform4f(glGetUniformLocation(g_wireShader, "uColor"), 0.9f, 0.5f, 0.1f, 0.7f); // Orange
                glLineWidth(2.0f);
                glBindVertexArray(g_surfRecVAO);
                glBindBuffer(GL_ARRAY_BUFFER, g_surfRecVBO);
                glBufferData(GL_ARRAY_BUFFER, encLines.size() * sizeof(glm::vec3), encLines.data(), GL_DYNAMIC_DRAW);
                glDrawArrays(GL_LINES, 0, (int)encLines.size());
                glBindVertexArray(0);
                glLineWidth(1.0f);
            }
        }
    }

    // Draw clipping plane indicator
    if (g_clipEnabled && !g_model.IsEmpty()) {
        float ext = g_model.extent * 1.5f;
        glm::vec3 ctr = g_model.center;
        std::vector<glm::vec3> clipLines;
        float val = g_clipValue;

        if (g_clipAxis == 1) { // Y (horizontal)
            clipLines.push_back(glm::vec3(ctr.x - ext, val, ctr.z - ext));
            clipLines.push_back(glm::vec3(ctr.x + ext, val, ctr.z - ext));
            clipLines.push_back(glm::vec3(ctr.x + ext, val, ctr.z - ext));
            clipLines.push_back(glm::vec3(ctr.x + ext, val, ctr.z + ext));
            clipLines.push_back(glm::vec3(ctr.x + ext, val, ctr.z + ext));
            clipLines.push_back(glm::vec3(ctr.x - ext, val, ctr.z + ext));
            clipLines.push_back(glm::vec3(ctr.x - ext, val, ctr.z + ext));
            clipLines.push_back(glm::vec3(ctr.x - ext, val, ctr.z - ext));
            // Cross lines
            clipLines.push_back(glm::vec3(ctr.x - ext, val, ctr.z));
            clipLines.push_back(glm::vec3(ctr.x + ext, val, ctr.z));
            clipLines.push_back(glm::vec3(ctr.x, val, ctr.z - ext));
            clipLines.push_back(glm::vec3(ctr.x, val, ctr.z + ext));
        }

        if (!clipLines.empty()) {
            glUseProgram(g_wireShader);
            glUniformMatrix4fv(glGetUniformLocation(g_wireShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
            glUniform4f(glGetUniformLocation(g_wireShader, "uColor"), 0.95f, 0.22f, 0.3f, 0.6f); // Red
            glBindVertexArray(g_surfRecVAO);
            glBindBuffer(GL_ARRAY_BUFFER, g_surfRecVBO);
            glBufferData(GL_ARRAY_BUFFER, clipLines.size() * sizeof(glm::vec3), clipLines.data(), GL_DYNAMIC_DRAW);
            glDrawArrays(GL_LINES, 0, (int)clipLines.size());
            glBindVertexArray(0);
        }
    }

    // Draw surface receiver colormap overlay (semi-transparent, lit)
    if (g_hasColormap && g_colormapVertCount > 0) {
        glUseProgram(g_meshShader);
        glUniformMatrix4fv(glGetUniformLocation(g_meshShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
        glUniform3f(glGetUniformLocation(g_meshShader, "uLightDir"), 0.3f, 0.8f, 0.5f);
        glUniform3fv(glGetUniformLocation(g_meshShader, "uCameraPos"), 1, glm::value_ptr(g_camera.GetPosition()));
        glUniform1f(glGetUniformLocation(g_meshShader, "uAmbient"), 0.45f);
        glUniform1f(glGetUniformLocation(g_meshShader, "uAlpha"), 0.75f); // semi-transparent overlay
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        glEnable(GL_POLYGON_OFFSET_FILL); // prevent z-fighting with room mesh
        glPolygonOffset(-1.0f, -1.0f);
        glBindVertexArray(g_colormapVAO);
        glDrawArrays(GL_TRIANGLES, 0, g_colormapVertCount);
        glBindVertexArray(0);
        glDisable(GL_POLYGON_OFFSET_FILL);
        glUniform1f(glGetUniformLocation(g_meshShader, "uAlpha"), 0.0f); // reset to opaque
    }

    // Draw iso-contour lines on colormap (white lines at constant dB)
    if (g_hasIsoContours && g_isoVertCount > 0) {
        glUseProgram(g_wireShader);
        glUniformMatrix4fv(glGetUniformLocation(g_wireShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
        glUniform4f(glGetUniformLocation(g_wireShader, "uColor"), 1.0f, 1.0f, 1.0f, 0.8f); // white
        glLineWidth(1.5f);
        glDisable(GL_DEPTH_TEST); // draw on top of colormap
        glBindVertexArray(g_isoVAO);
        glDrawArrays(GL_LINES, 0, g_isoVertCount);
        glBindVertexArray(0);
        glEnable(GL_DEPTH_TEST);
        glLineWidth(1.0f);
    }

    // Draw source/receiver icons
    DrawIcons(viewProj, width, height);

    // Draw intensity vector arrows (receiver -> nearest source)
    DrawIntensityArrows(viewProj);

    // Draw particles (cinematic trails + glow)
    DrawParticles(viewProj, width, height);

    glBindFramebuffer(GL_FRAMEBUFFER, 0);

    // Display in ImGui
    ImGui::Image((ImTextureID)(intptr_t)g_fboColor, ImVec2(width, height), ImVec2(0, 1), ImVec2(1, 0));

    // Viewport overlay info
    ImVec2 overlayPos = ImGui::GetItemRectMin();
    ImVec2 overlaySize = ImGui::GetItemRectSize();
    ImDrawList* dl = ImGui::GetWindowDrawList();
    if (!g_model.IsEmpty()) {
        char info[128];
        int totalFaces = 0;
        for (auto& g : g_model.groups) totalFaces += (int)g.faces.size();
        snprintf(info, sizeof(info), "%zu verts | %d faces | %zu groups",
                 g_model.vertices.size(), totalFaces, g_model.groups.size());
        dl->AddText(ImVec2(overlayPos.x + 10, overlayPos.y + 10),
            ImGui::GetColorU32(ImVec4(0.95f, 0.10f, 0.15f, 0.7f)), info);
    }

    // Draw 2D labels for sources and receivers
    {
        Project& proj = GetProject();
        auto worldToScreen = [&](glm::vec3 worldPos) -> ImVec2 {
            glm::vec4 clip = viewProj * glm::vec4(worldPos, 1.0f);
            if (clip.w <= 0.001f) return ImVec2(-1, -1); // behind camera
            glm::vec3 ndc = glm::vec3(clip) / clip.w;
            float sx = overlayPos.x + (ndc.x * 0.5f + 0.5f) * overlaySize.x;
            float sy = overlayPos.y + (1.0f - (ndc.y * 0.5f + 0.5f)) * overlaySize.y;
            return ImVec2(sx, sy);
        };

        for (auto& src : proj.sources) {
            ImVec2 sp = worldToScreen(src.position);
            if (sp.x < 0) continue;
            ImU32 col = ImGui::GetColorU32(ImVec4(src.displayColor.r, src.displayColor.g, src.displayColor.b, 0.9f));
            dl->AddText(ImVec2(sp.x + 12, sp.y - 6), col, src.name.c_str());
        }
        for (auto& rcv : proj.punctualReceivers) {
            ImVec2 sp = worldToScreen(rcv.position);
            if (sp.x < 0) continue;
            ImU32 col = ImGui::GetColorU32(ImVec4(rcv.displayColor.r, rcv.displayColor.g, rcv.displayColor.b, 0.9f));
            dl->AddText(ImVec2(sp.x + 12, sp.y - 6), col, rcv.name.c_str());
        }
        for (auto& sr : proj.surfaceReceivers) {
            if (sr.type != SurfaceReceiver::Plane) continue;
            glm::vec3 center = (sr.vertexA + sr.vertexB + sr.vertexC) / 3.0f;
            ImVec2 sp = worldToScreen(center);
            if (sp.x < 0) continue;
            ImU32 col = ImGui::GetColorU32(ImVec4(0.2f, 0.92f, 0.4f, 0.9f));
            dl->AddText(ImVec2(sp.x + 8, sp.y - 6), col, sr.name.c_str());
        }
        // Encumbrance labels
        for (auto& enc : proj.encumbrances) {
            if (!enc.active || !enc.showLabel) continue;
            glm::vec3 center = (enc.boxMin + enc.boxMax) * 0.5f;
            ImVec2 sp = worldToScreen(center);
            if (sp.x < 0) continue;
            ImU32 col = ImGui::GetColorU32(ImVec4(enc.color.r, enc.color.g, enc.color.b, 0.9f));
            dl->AddText(ImVec2(sp.x + 8, sp.y - 6), col, enc.name.c_str());
        }
    }

    // ── Color Legend Bar (vertical jet gradient on right side) ──────────────
    if (g_hasColormap) {
        float legendW = 20.0f;
        float legendH = 200.0f;
        float marginR = 30.0f;
        float marginT = 50.0f;
        float legendX = overlayPos.x + overlaySize.x - marginR - legendW;
        float legendY = overlayPos.y + marginT;

        float minVal = GetColormapMin();
        float maxVal = GetColormapMax();
        const char* cmapName = GetColormapName();

        // Draw background behind legend for readability
        dl->AddRectFilled(
            ImVec2(legendX - 4, legendY - 18),
            ImVec2(legendX + legendW + 54, legendY + legendH + 18),
            ImGui::GetColorU32(ImVec4(0.0f, 0.0f, 0.0f, 0.5f)));

        // Draw vertical gradient bar (bottom = blue/min, top = red/max)
        for (int row = 0; row < (int)legendH; row++) {
            float t = 1.0f - (float)row / legendH; // top=1 (red), bottom=0 (blue)
            glm::vec3 c;
            if (t < 0.25f) c = glm::mix(glm::vec3(0,0,1), glm::vec3(0,1,1), t*4.0f);
            else if (t < 0.50f) c = glm::mix(glm::vec3(0,1,1), glm::vec3(0,1,0), (t-0.25f)*4.0f);
            else if (t < 0.75f) c = glm::mix(glm::vec3(0,1,0), glm::vec3(1,1,0), (t-0.50f)*4.0f);
            else c = glm::mix(glm::vec3(1,1,0), glm::vec3(1,0,0), (t-0.75f)*4.0f);
            dl->AddRectFilled(
                ImVec2(legendX, legendY + row),
                ImVec2(legendX + legendW, legendY + row + 1),
                ImGui::GetColorU32(ImVec4(c.r, c.g, c.b, 1.0f)));
        }

        // Border around the gradient bar
        dl->AddRect(
            ImVec2(legendX, legendY),
            ImVec2(legendX + legendW, legendY + legendH),
            ImGui::GetColorU32(ImVec4(0.7f, 0.7f, 0.7f, 0.8f)));

        // Max label (top)
        char maxLabel[32];
        snprintf(maxLabel, sizeof(maxLabel), "%.1f", maxVal);
        dl->AddText(ImVec2(legendX + legendW + 4, legendY - 6),
            ImGui::GetColorU32(ImVec4(0.95f, 0.95f, 0.95f, 0.9f)), maxLabel);

        // Min label (bottom)
        char minLabel[32];
        snprintf(minLabel, sizeof(minLabel), "%.1f", minVal);
        dl->AddText(ImVec2(legendX + legendW + 4, legendY + legendH - 8),
            ImGui::GetColorU32(ImVec4(0.95f, 0.95f, 0.95f, 0.9f)), minLabel);

        // Colormap name label (above bar)
        if (cmapName && cmapName[0]) {
            dl->AddText(ImVec2(legendX - 2, legendY - 16),
                ImGui::GetColorU32(ImVec4(0.8f, 0.8f, 0.8f, 0.9f)), cmapName);
        }
    }
}

// ── Colormap implementation ─────────────────────────────────────────────────

// Jet-like colormap: blue -> cyan -> green -> yellow -> red
static glm::vec3 JetColor(float t) {
    t = glm::clamp(t, 0.0f, 1.0f);
    if (t < 0.25f) return glm::mix(glm::vec3(0, 0, 1), glm::vec3(0, 1, 1), t * 4.0f);
    if (t < 0.50f) return glm::mix(glm::vec3(0, 1, 1), glm::vec3(0, 1, 0), (t - 0.25f) * 4.0f);
    if (t < 0.75f) return glm::mix(glm::vec3(0, 1, 0), glm::vec3(1, 1, 0), (t - 0.50f) * 4.0f);
    return glm::mix(glm::vec3(1, 1, 0), glm::vec3(1, 0, 0), (t - 0.75f) * 4.0f);
}

void ViewportLoadColormap(const SurfaceRecResult& result) {
    if (g_colormapVAO) { glDeleteVertexArrays(1, &g_colormapVAO); g_colormapVAO = 0; }
    if (g_colormapVBO) { glDeleteBuffers(1, &g_colormapVBO); g_colormapVBO = 0; }
    g_colormapVertCount = 0;
    g_hasColormap = false;

    if (!result.loaded || result.faces.empty()) return;

    // Use log-scale (dB) normalization for meaningful color spread
    const float P0 = 2.5e9f; // 1/(20e-6)^2
    float minDb = 200, maxDb = -200;
    for (auto& face : result.faces) {
        if (face.energySum > 0) {
            float db = 10.0f * log10f(face.energySum * P0);
            if (db < minDb) minDb = db;
            if (db > maxDb) maxDb = db;
        }
    }
    if (maxDb <= minDb) { minDb = 0; maxDb = 1; }
    // Store dB range for legend display (in static vars — accessor functions read these)
    static float s_colormapMinDb = 0, s_colormapMaxDb = 1;
    s_colormapMinDb = minDb;
    s_colormapMaxDb = maxDb;

    struct CVertex { float px, py, pz, nx, ny, nz, cr, cg, cb; };
    std::vector<CVertex> verts;
    verts.reserve(result.faces.size() * 3);

    for (auto& face : result.faces) {
        if (face.v[0] >= result.nodes.size() || face.v[1] >= result.nodes.size() ||
            face.v[2] >= result.nodes.size()) continue;

        glm::vec3 a = result.nodes[face.v[0]];
        glm::vec3 b = result.nodes[face.v[1]];
        glm::vec3 c = result.nodes[face.v[2]];
        glm::vec3 n = glm::normalize(glm::cross(b - a, c - a));

        float db = (face.energySum > 0) ? 10.0f * log10f(face.energySum * P0) : minDb;
        float t = (db - minDb) / (maxDb - minDb);
        t = glm::clamp(t, 0.0f, 1.0f);
        glm::vec3 col = JetColor(t);

        verts.push_back({a.x, a.y, a.z, n.x, n.y, n.z, col.r, col.g, col.b});
        verts.push_back({b.x, b.y, b.z, n.x, n.y, n.z, col.r, col.g, col.b});
        verts.push_back({c.x, c.y, c.z, n.x, n.y, n.z, col.r, col.g, col.b});
    }

    g_colormapVertCount = (int)verts.size();
    if (g_colormapVertCount == 0) return;

    glGenVertexArrays(1, &g_colormapVAO);
    glGenBuffers(1, &g_colormapVBO);
    glBindVertexArray(g_colormapVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_colormapVBO);
    glBufferData(GL_ARRAY_BUFFER, verts.size() * sizeof(CVertex), verts.data(), GL_STATIC_DRAW);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(CVertex), (void*)0);
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, sizeof(CVertex), (void*)(3 * sizeof(float)));
    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 3, GL_FLOAT, GL_FALSE, sizeof(CVertex), (void*)(6 * sizeof(float)));
    glBindVertexArray(0);

    g_hasColormap = true;
    printf("[Viewport] Colormap loaded: %d triangles, dB range %.1f — %.1f\n",
           g_colormapVertCount / 3, minDb, maxDb);

    // ── Build iso-contour lines (marching triangles) ────────────────────────
    // Compute per-VERTEX dB values by averaging all faces touching each vertex
    std::vector<float> vertDb(result.nodes.size(), -999);
    std::vector<int> vertCount(result.nodes.size(), 0);
    for (auto& face : result.faces) {
        float db = (face.energySum > 0) ? 10.0f * log10f(face.energySum * P0) : minDb;
        for (int i = 0; i < 3; i++) {
            if (face.v[i] < result.nodes.size()) {
                if (vertDb[face.v[i]] < -998) vertDb[face.v[i]] = 0;
                vertDb[face.v[i]] += db;
                vertCount[face.v[i]]++;
            }
        }
    }
    for (size_t i = 0; i < vertDb.size(); i++) {
        if (vertCount[i] > 0) vertDb[i] /= vertCount[i];
        else vertDb[i] = minDb;
    }

    // Generate iso-levels: every 3 dB within the range
    float isoStep = 3.0f;
    if (maxDb - minDb < 6.0f) isoStep = 1.0f;
    else if (maxDb - minDb > 30.0f) isoStep = 6.0f;

    std::vector<glm::vec3> isoLines;
    for (float level = ceilf(minDb / isoStep) * isoStep; level < maxDb; level += isoStep) {
        for (auto& face : result.faces) {
            if (face.v[0] >= result.nodes.size() || face.v[1] >= result.nodes.size() ||
                face.v[2] >= result.nodes.size()) continue;

            float d[3] = { vertDb[face.v[0]], vertDb[face.v[1]], vertDb[face.v[2]] };
            glm::vec3 p[3];
            for (int i = 0; i < 3; i++) p[i] = result.nodes[face.v[i]];

            // Find edge crossings
            glm::vec3 crossings[2];
            int nCross = 0;
            for (int e = 0; e < 3 && nCross < 2; e++) {
                int i0 = e, i1 = (e + 1) % 3;
                if ((d[i0] < level && d[i1] >= level) || (d[i0] >= level && d[i1] < level)) {
                    float t = (level - d[i0]) / (d[i1] - d[i0]);
                    t = glm::clamp(t, 0.0f, 1.0f);
                    crossings[nCross++] = glm::mix(p[i0], p[i1], t);
                }
            }
            if (nCross == 2) {
                // Lift slightly above colormap surface to prevent z-fighting
                glm::vec3 n = glm::normalize(glm::cross(p[1] - p[0], p[2] - p[0]));
                crossings[0] += n * 0.005f;
                crossings[1] += n * 0.005f;
                isoLines.push_back(crossings[0]);
                isoLines.push_back(crossings[1]);
            }
        }
    }

    // Upload iso-contour lines to GPU
    if (g_isoVAO) { glDeleteVertexArrays(1, &g_isoVAO); g_isoVAO = 0; }
    if (g_isoVBO) { glDeleteBuffers(1, &g_isoVBO); g_isoVBO = 0; }
    g_isoVertCount = 0;
    g_hasIsoContours = false;

    if (!isoLines.empty()) {
        glGenVertexArrays(1, &g_isoVAO);
        glGenBuffers(1, &g_isoVBO);
        glBindVertexArray(g_isoVAO);
        glBindBuffer(GL_ARRAY_BUFFER, g_isoVBO);
        glBufferData(GL_ARRAY_BUFFER, isoLines.size() * sizeof(glm::vec3), isoLines.data(), GL_STATIC_DRAW);
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(glm::vec3), (void*)0);
        glBindVertexArray(0);
        g_isoVertCount = (int)isoLines.size();
        g_hasIsoContours = true;
        int nLevels = (int)((maxDb - ceilf(minDb / isoStep) * isoStep) / isoStep);
        printf("[Viewport] Iso-contours: %d line segments, %d levels (every %.0f dB)\n",
               g_isoVertCount / 2, nLevels, isoStep);
    }
}

void ViewportClearColormap() {
    if (g_colormapVAO) { glDeleteVertexArrays(1, &g_colormapVAO); g_colormapVAO = 0; }
    if (g_colormapVBO) { glDeleteBuffers(1, &g_colormapVBO); g_colormapVBO = 0; }
    g_colormapVertCount = 0;
    g_hasColormap = false;
    if (g_isoVAO) { glDeleteVertexArrays(1, &g_isoVAO); g_isoVAO = 0; }
    if (g_isoVBO) { glDeleteBuffers(1, &g_isoVBO); g_isoVBO = 0; }
    g_isoVertCount = 0;
    g_hasIsoContours = false;
}

bool ViewportHasColormap() { return g_hasColormap; }

// ── Particle animation implementation ────────────────────────────────────────

void ViewportLoadParticles(const ParticleData& data) {
    g_particleData = data;
    g_particleTimeStep = 0;
    g_hasParticles = data.loaded && !data.particles.empty();
    printf("[Viewport] Particles loaded: %zu particles, %d max steps\n",
           data.particles.size(), data.maxTimeSteps);
}

void ViewportClearParticles() {
    g_particleData = {};
    g_hasParticles = false;
    g_particleTimeStep = 0;
}

void ViewportSetParticleTimeStep(int step) {
    g_particleTimeStep = step;
}

int ViewportGetParticleMaxSteps() {
    return g_hasParticles ? g_particleData.maxTimeSteps : 0;
}

bool ViewportHasParticles() { return g_hasParticles; }

// ── Cinematic particle trail shader ─────────────────────────────────────────

static GLuint g_trailShader = 0;
static GLuint g_trailVAO = 0, g_trailVBO = 0;

static const char* trailVertSrc = R"(
#version 430 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec4 aColor;
uniform mat4 uViewProj;
out vec4 vColor;
void main() {
    vColor = aColor;
    gl_Position = uViewProj * vec4(aPos, 1.0);
}
)";

static const char* trailFragSrc = R"(
#version 430 core
in vec4 vColor;
out vec4 FragColor;
void main() {
    FragColor = vColor;
}
)";

static void InitTrailRenderer() {
    if (g_trailShader) return;
    g_trailShader = CreateProgram(trailVertSrc, trailFragSrc);
    glGenVertexArrays(1, &g_trailVAO);
    glGenBuffers(1, &g_trailVBO);
    glBindVertexArray(g_trailVAO);
    glBindBuffer(GL_ARRAY_BUFFER, g_trailVBO);
    // pos(3) + color(4) = 7 floats
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 7 * sizeof(float), (void*)0);
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, 7 * sizeof(float), (void*)(3 * sizeof(float)));
    glBindVertexArray(0);
}

// Vibrant rainbow color per particle index — each particle gets a unique hue
static glm::vec3 RainbowColor(int index) {
    float hue = fmodf(index * 0.618033988f, 1.0f); // golden ratio spacing
    // HSV to RGB (S=1, V=1)
    float h = hue * 6.0f;
    int i = (int)h;
    float f = h - i;
    float q = 1.0f - f, t = f;
    switch (i % 6) {
        case 0: return {1, t, 0};
        case 1: return {q, 1, 0};
        case 2: return {0, 1, t};
        case 3: return {0, q, 1};
        case 4: return {t, 0, 1};
        default: return {1, 0, q};
    }
}

static void DrawParticles(const glm::mat4& viewProj, float vpWidth, float vpHeight) {
    if (!g_hasParticles) return;
    InitTrailRenderer();

    const int TRAIL_LENGTH = 24;  // longer trails for smoother look

    std::vector<IconVertex> heads;
    struct TrailVert { float x, y, z, r, g, b, a; };
    std::vector<TrailVert> trails;

    int particleIdx = 0;
    for (auto& p : g_particleData.particles) {
        int localStep = g_particleTimeStep - (int)p.firstTimeStep;
        if (localStep < 0 || localStep >= (int)p.steps.size()) { particleIdx++; continue; }

        auto& step = p.steps[localStep];
        float e = step.energy;
        float eNorm = (e > 0) ? glm::clamp(log10f(e + 1e-20f) / -6.0f + 1.0f, 0.0f, 1.0f) : 0;

        // Each particle gets a vibrant rainbow color based on its index
        glm::vec3 baseColor = RainbowColor(particleIdx);
        // Brighten with energy: low energy → dimmer, high energy → saturated + white boost
        glm::vec3 headRGB = glm::mix(baseColor * 0.4f, baseColor + glm::vec3(0.3f), eNorm);
        headRGB = glm::clamp(headRGB, 0.0f, 1.0f);
        float headAlpha = 0.6f + eNorm * 0.4f;
        float sz = 0.3f + eNorm * 0.4f; // bigger heads

        heads.push_back({step.position.x, step.position.y, step.position.z,
                         headRGB.r, headRGB.g, headRGB.b, headAlpha, sz});

        // Trail — smooth gradient fading to transparent
        for (int t = 1; t < TRAIL_LENGTH; t++) {
            int prevStep = localStep - t;
            int curStep = localStep - (t - 1);
            if (prevStep < 0 || curStep < 0) break;

            auto& pPrev = p.steps[prevStep];
            auto& pCur = p.steps[curStep];

            float fade = 1.0f - (float)t / (float)TRAIL_LENGTH;
            fade = fade * fade * fade; // cubic falloff — sharp bright head, long dim tail

            // Trail color: same hue as head but fading
            glm::vec3 trailRGB = baseColor * (0.5f + fade * 0.5f);
            float trailAlpha = fade * 0.7f;

            trails.push_back({pCur.position.x, pCur.position.y, pCur.position.z,
                              trailRGB.r, trailRGB.g, trailRGB.b, trailAlpha});
            trails.push_back({pPrev.position.x, pPrev.position.y, pPrev.position.z,
                              trailRGB.r * 0.7f, trailRGB.g * 0.7f, trailRGB.b * 0.7f, trailAlpha * 0.5f});
        }
        particleIdx++;
    }

    // Draw trails first (additive blending for glow)
    if (!trails.empty()) {
        glUseProgram(g_trailShader);
        glUniformMatrix4fv(glGetUniformLocation(g_trailShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));

        glBindVertexArray(g_trailVAO);
        glBindBuffer(GL_ARRAY_BUFFER, g_trailVBO);
        glBufferData(GL_ARRAY_BUFFER, trails.size() * sizeof(TrailVert), trails.data(), GL_DYNAMIC_DRAW);

        glDisable(GL_DEPTH_TEST);
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE); // Additive blending — trails glow
        glLineWidth(1.5f);
        glDrawArrays(GL_LINES, 0, (int)trails.size());
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA); // Restore normal blending
        glLineWidth(1.0f);
        glEnable(GL_DEPTH_TEST);
        glBindVertexArray(0);
    }

    // Draw particle heads (bright points on top)
    if (!heads.empty()) {
        glUseProgram(g_iconShader);
        glUniformMatrix4fv(glGetUniformLocation(g_iconShader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
        glUniform2f(glGetUniformLocation(g_iconShader, "uViewportSize"), vpWidth, vpHeight);

        glBindVertexArray(g_iconVAO);
        glBindBuffer(GL_ARRAY_BUFFER, g_iconVBO);
        glBufferData(GL_ARRAY_BUFFER, heads.size() * sizeof(IconVertex), heads.data(), GL_DYNAMIC_DRAW);

        glEnable(GL_PROGRAM_POINT_SIZE);
        glDisable(GL_DEPTH_TEST);
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE); // Additive glow for heads too
        glDrawArrays(GL_POINTS, 0, (int)heads.size());
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        glEnable(GL_DEPTH_TEST);
        glDisable(GL_PROGRAM_POINT_SIZE);
        glBindVertexArray(0);
    }
}

// ── 3D Energy Density Heatmap ───────────────────────────────────────────────

// ── Viewport toggles ────────────────────────────────────────────────────────

void ViewportToggleWireframe() { g_showWireframe = !g_showWireframe; }
void ViewportToggleFaces()     { g_showFaces = !g_showFaces; }
bool ViewportGetShowWireframe() { return g_showWireframe; }
bool ViewportGetShowFaces()     { return g_showFaces; }
void ViewportSetFaceMode(int mode) { g_faceMode = mode; }
int  ViewportGetFaceMode()     { return g_faceMode; }

// ── Camera presets ──────────────────────────────────────────────────────────

void ViewportCameraTop()   { g_camera.SetTopView(); }
void ViewportCameraFront() { g_camera.SetFrontView(); }
void ViewportCameraRight() { g_camera.SetRightView(); }
void ViewportCameraReset() { g_camera.ResetView(); ViewportFocusModel(); }

// ── Face picking (CPU ray-triangle intersection) ────────────────────────────

static bool RayTriIntersect(const glm::vec3& orig, const glm::vec3& dir,
                            const glm::vec3& v0, const glm::vec3& v1, const glm::vec3& v2,
                            float& t) {
    glm::vec3 e1 = v1 - v0, e2 = v2 - v0;
    glm::vec3 h = glm::cross(dir, e2);
    float a = glm::dot(e1, h);
    if (fabsf(a) < 1e-8f) return false;
    float f = 1.0f / a;
    glm::vec3 s = orig - v0;
    float u = f * glm::dot(s, h);
    if (u < 0 || u > 1) return false;
    glm::vec3 q = glm::cross(s, e1);
    float v = f * glm::dot(dir, q);
    if (v < 0 || u + v > 1) return false;
    t = f * glm::dot(e2, q);
    return t > 0.001f;
}

int ViewportPickFace(float screenX, float screenY) {
    if (g_model.IsEmpty() || g_fboWidth <= 0 || g_fboHeight <= 0) return -1;

    // Convert screen coords to NDC
    float ndcX = (screenX / (float)g_fboWidth) * 2.0f - 1.0f;
    float ndcY = 1.0f - (screenY / (float)g_fboHeight) * 2.0f;

    // Unproject to world ray
    glm::mat4 view = g_camera.GetViewMatrix();
    glm::mat4 proj = glm::perspective(glm::radians(45.0f), (float)g_fboWidth / (float)g_fboHeight,
        std::max(0.01f, g_model.extent * 0.001f),
        std::max(500.0f, g_model.extent * 20.0f));
    glm::mat4 invVP = glm::inverse(proj * view);

    glm::vec4 nearPt = invVP * glm::vec4(ndcX, ndcY, -1.0f, 1.0f);
    glm::vec4 farPt  = invVP * glm::vec4(ndcX, ndcY,  1.0f, 1.0f);
    nearPt /= nearPt.w;
    farPt /= farPt.w;

    glm::vec3 rayOrig = glm::vec3(nearPt);
    glm::vec3 rayDir = glm::normalize(glm::vec3(farPt) - rayOrig);

    // Test all faces, find closest hit
    float minT = 1e30f;
    int hitGroup = -1;

    for (int gi = 0; gi < (int)g_model.groups.size(); gi++) {
        for (auto& face : g_model.groups[gi].faces) {
            if (face.v[0] >= g_model.vertices.size() ||
                face.v[1] >= g_model.vertices.size() ||
                face.v[2] >= g_model.vertices.size()) continue;

            float t;
            if (RayTriIntersect(rayOrig, rayDir,
                    g_model.vertices[face.v[0]],
                    g_model.vertices[face.v[1]],
                    g_model.vertices[face.v[2]], t)) {
                if (t < minT) {
                    minT = t;
                    hitGroup = gi;
                }
            }
        }
    }

    return hitGroup;
}

// ── Screen to world ray ─────────────────────────────────────────────────────

glm::vec3 ViewportScreenToWorld(float screenX, float screenY, float groundY) {
    if (g_fboWidth <= 0 || g_fboHeight <= 0) return glm::vec3(0);

    float ndcX = (screenX / (float)g_fboWidth) * 2.0f - 1.0f;
    float ndcY = 1.0f - (screenY / (float)g_fboHeight) * 2.0f;

    glm::mat4 view = g_camera.GetViewMatrix();
    glm::mat4 proj = glm::perspective(glm::radians(45.0f), (float)g_fboWidth / (float)g_fboHeight,
        std::max(0.01f, g_model.extent * 0.001f),
        std::max(500.0f, g_model.extent * 20.0f));
    glm::mat4 invVP = glm::inverse(proj * view);

    glm::vec4 nearPt = invVP * glm::vec4(ndcX, ndcY, -1.0f, 1.0f);
    glm::vec4 farPt  = invVP * glm::vec4(ndcX, ndcY,  1.0f, 1.0f);
    nearPt /= nearPt.w;
    farPt /= farPt.w;

    glm::vec3 rayOrig = glm::vec3(nearPt);
    glm::vec3 rayDir = glm::normalize(glm::vec3(farPt) - rayOrig);

    // First try ray-mesh intersection
    float minT = 1e30f;
    bool hit = false;
    glm::vec3 hitPos;
    for (auto& grp : g_model.groups) {
        for (auto& face : grp.faces) {
            if (face.v[0] >= g_model.vertices.size()) continue;
            float t;
            if (RayTriIntersect(rayOrig, rayDir,
                    g_model.vertices[face.v[0]], g_model.vertices[face.v[1]],
                    g_model.vertices[face.v[2]], t)) {
                if (t < minT) { minT = t; hit = true; hitPos = rayOrig + rayDir * t; }
            }
        }
    }
    if (hit) return hitPos;

    // Fallback: intersect with ground plane at groundY
    if (fabsf(rayDir.y) > 1e-6f) {
        float t = (groundY - rayOrig.y) / rayDir.y;
        if (t > 0) return rayOrig + rayDir * t;
    }
    return rayOrig + rayDir * 10.0f; // fallback
}

// ── Screenshot export (raw TGA — no external lib needed) ────────────────────

bool ViewportSaveScreenshot(const std::string& path) {
    if (g_fboWidth <= 0 || g_fboHeight <= 0 || g_fbo == 0) return false;

    int w = g_fboWidth, h = g_fboHeight;
    std::vector<uint8_t> pixels(w * h * 3);

    glBindFramebuffer(GL_FRAMEBUFFER, g_fbo);
    glReadPixels(0, 0, w, h, GL_RGB, GL_UNSIGNED_BYTE, pixels.data());
    glBindFramebuffer(GL_FRAMEBUFFER, 0);

    // Write as TGA (uncompressed, no external dependencies)
    std::ofstream f(path, std::ios::binary);
    if (!f.is_open()) return false;

    uint8_t header[18] = {};
    header[2] = 2; // uncompressed RGB
    header[12] = w & 0xFF; header[13] = (w >> 8) & 0xFF;
    header[14] = h & 0xFF; header[15] = (h >> 8) & 0xFF;
    header[16] = 24; // bits per pixel
    f.write((char*)header, 18);

    // TGA stores BGR, bottom-to-top (which is how glReadPixels gives it)
    for (int y = 0; y < h; y++) {
        for (int x = 0; x < w; x++) {
            int idx = (y * w + x) * 3;
            uint8_t bgr[3] = { pixels[idx + 2], pixels[idx + 1], pixels[idx] };
            f.write((char*)bgr, 3);
        }
    }

    f.close();
    printf("[Screenshot] Saved: %s (%dx%d)\n", path.c_str(), w, h);
    return true;
}

// ── Clipping plane ──────────────────────────────────────────────────────────

void ViewportSetClipPlane(bool enabled, int axis, float value) {
    g_clipEnabled = enabled;
    g_clipAxis = axis;
    g_clipValue = value;
}

void ShutdownViewport() {
    g_gpuMesh.Destroy();
    if (g_gridVAO) glDeleteVertexArrays(1, &g_gridVAO);
    if (g_gridVBO) glDeleteBuffers(1, &g_gridVBO);
    if (g_gridShader) glDeleteProgram(g_gridShader);
    if (g_meshShader) glDeleteProgram(g_meshShader);
    if (g_wireShader) glDeleteProgram(g_wireShader);
    if (g_iconShader) glDeleteProgram(g_iconShader);
    if (g_iconVAO) glDeleteVertexArrays(1, &g_iconVAO);
    if (g_iconVBO) glDeleteBuffers(1, &g_iconVBO);
    if (g_surfRecVAO) glDeleteVertexArrays(1, &g_surfRecVAO);
    if (g_surfRecVBO) glDeleteBuffers(1, &g_surfRecVBO);
    if (g_fbo) glDeleteFramebuffers(1, &g_fbo);
    if (g_fboColor) glDeleteTextures(1, &g_fboColor);
    if (g_fboDepth) glDeleteRenderbuffers(1, &g_fboDepth);
    ViewportClearColormap();
}

} // namespace isimpa
