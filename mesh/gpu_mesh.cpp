#include "mesh/scene_model.h"
#include <glm/gtc/type_ptr.hpp>
#include <vector>
#include <cstdio>

namespace isimpa {

// ─── Mesh shader (lit faces with per-group color) ────────────────────────────

static const char* meshVertSrc = R"(
#version 430 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNormal;
layout(location = 2) in vec3 aColor;

uniform mat4 uViewProj;

out vec3 vNormal;
out vec3 vColor;
out vec3 vWorldPos;

void main() {
    vNormal = aNormal;
    vColor = aColor;
    vWorldPos = aPos;
    gl_Position = uViewProj * vec4(aPos, 1.0);
}
)";

static const char* meshFragSrc = R"(
#version 430 core
in vec3 vNormal;
in vec3 vColor;
in vec3 vWorldPos;

out vec4 FragColor;

uniform vec3 uLightDir;
uniform vec3 uCameraPos;
uniform float uAmbient;
uniform float uAlpha;  // 0 = use 1.0 (opaque), >0 = translucent

void main() {
    vec3 N = normalize(vNormal);
    vec3 L = normalize(uLightDir);
    vec3 V = normalize(uCameraPos - vWorldPos);
    vec3 H = normalize(L + V);

    // Two-sided lighting
    float NdotL = abs(dot(N, L));
    float NdotH = abs(dot(N, H));

    float diffuse = NdotL * 0.6;
    float specular = pow(NdotH, 32.0) * 0.15;
    float ambient = uAmbient;

    vec3 color = vColor * (ambient + diffuse) + vec3(specular);

    // Subtle neon edge glow
    float rim = 1.0 - abs(dot(N, V));
    rim = pow(rim, 3.0) * 0.15;
    color += vec3(0.95, 0.1, 0.15) * rim;

    float alpha = uAlpha > 0.0 ? uAlpha : 1.0;
    FragColor = vec4(color, alpha);
}
)";

// ─── Wire shader (neon edges) ────────────────────────────────────────────────

static const char* wireVertSrc = R"(
#version 430 core
layout(location = 0) in vec3 aPos;
uniform mat4 uViewProj;
void main() {
    gl_Position = uViewProj * vec4(aPos, 1.0);
}
)";

static const char* wireFragSrc = R"(
#version 430 core
out vec4 FragColor;
uniform vec4 uColor;
void main() {
    FragColor = uColor;
}
)";

// ─── Shader compilation ──────────────────────────────────────────────────────

static GLuint CompileShader(GLenum type, const char* src) {
    GLuint s = glCreateShader(type);
    glShaderSource(s, 1, &src, nullptr);
    glCompileShader(s);
    int ok;
    glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
    if (!ok) {
        char log[512];
        glGetShaderInfoLog(s, 512, nullptr, log);
        fprintf(stderr, "[Shader] %s\n", log);
    }
    return s;
}

static GLuint LinkProgram(const char* vs, const char* fs) {
    GLuint v = CompileShader(GL_VERTEX_SHADER, vs);
    GLuint f = CompileShader(GL_FRAGMENT_SHADER, fs);
    GLuint p = glCreateProgram();
    glAttachShader(p, v);
    glAttachShader(p, f);
    glLinkProgram(p);
    int ok;
    glGetProgramiv(p, GL_LINK_STATUS, &ok);
    if (!ok) {
        char log[512];
        glGetProgramInfoLog(p, 512, nullptr, log);
        fprintf(stderr, "[Mesh Shader Link] %s\n", log);
    }
    glDeleteShader(v);
    glDeleteShader(f);
    return p;
}

GLuint CreateMeshShader() { return LinkProgram(meshVertSrc, meshFragSrc); }
GLuint CreateWireShader() { return LinkProgram(wireVertSrc, wireFragSrc); }

// ─── GPU Upload ──────────────────────────────────────────────────────────────

void GPUMesh::Upload(const SceneModel& model, const std::set<int>& selectedGroups) {
    Destroy(); // Clean any previous data

    if (model.vertices.empty()) return;

    // Build interleaved vertex data: pos(3) + normal(3) + color(3) per triangle vertex
    // We expand indexed to non-indexed for per-face normals and per-group colors
    struct Vertex { float px, py, pz, nx, ny, nz, cr, cg, cb; };
    std::vector<Vertex> verts;
    std::vector<glm::vec3> wireVerts;

    int totalFaces = 0;
    for (auto& grp : model.groups) totalFaces += (int)grp.faces.size();
    verts.reserve(totalFaces * 3);
    wireVerts.reserve(totalFaces * 6);

    size_t numVerts = model.vertices.size();
    for (int gi = 0; gi < (int)model.groups.size(); gi++) {
        auto& grp = model.groups[gi];
        for (auto& face : grp.faces) {
            if (face.v[0] >= numVerts || face.v[1] >= numVerts || face.v[2] >= numVerts) continue;
            glm::vec3 a = model.vertices[face.v[0]];
            glm::vec3 b = model.vertices[face.v[1]];
            glm::vec3 c = model.vertices[face.v[2]];
            glm::vec3 n = face.normal;
            glm::vec3 col = grp.color;

            // Highlight selected groups
            if (selectedGroups.count(gi)) {
                col = glm::mix(col, glm::vec3(0.95f, 0.1f, 0.15f), 0.4f); // Blend with neon red
            }

            verts.push_back({a.x, a.y, a.z, n.x, n.y, n.z, col.r, col.g, col.b});
            verts.push_back({b.x, b.y, b.z, n.x, n.y, n.z, col.r, col.g, col.b});
            verts.push_back({c.x, c.y, c.z, n.x, n.y, n.z, col.r, col.g, col.b});

            // Wireframe edges
            wireVerts.push_back(a); wireVerts.push_back(b);
            wireVerts.push_back(b); wireVerts.push_back(c);
            wireVerts.push_back(c); wireVerts.push_back(a);
        }
    }

    indexCount = (int)verts.size();
    wireVertCount = (int)wireVerts.size();

    // Solid mesh VAO
    glGenVertexArrays(1, &vao);
    glGenBuffers(1, &vboPos);
    glBindVertexArray(vao);
    glBindBuffer(GL_ARRAY_BUFFER, vboPos);
    glBufferData(GL_ARRAY_BUFFER, verts.size() * sizeof(Vertex), verts.data(), GL_STATIC_DRAW);

    // Position
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)0);
    // Normal
    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)(3 * sizeof(float)));
    // Color
    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex), (void*)(6 * sizeof(float)));

    glBindVertexArray(0);

    // Wireframe VAO
    glGenVertexArrays(1, &wireVao);
    glGenBuffers(1, &wireVbo);
    glBindVertexArray(wireVao);
    glBindBuffer(GL_ARRAY_BUFFER, wireVbo);
    glBufferData(GL_ARRAY_BUFFER, wireVerts.size() * sizeof(glm::vec3), wireVerts.data(), GL_STATIC_DRAW);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(glm::vec3), (void*)0);
    glBindVertexArray(0);

    printf("[GPU] Uploaded %d triangles, %d wireframe verts\n", indexCount / 3, wireVertCount);
}

void GPUMesh::Draw(GLuint shader, const glm::mat4& viewProj, bool wireframe) {
    if (indexCount == 0) return;

    if (!wireframe) {
        // Draw solid faces
        glUseProgram(shader);
        glUniformMatrix4fv(glGetUniformLocation(shader, "uViewProj"), 1, GL_FALSE, glm::value_ptr(viewProj));
        glUniform3f(glGetUniformLocation(shader, "uLightDir"), 0.3f, 0.8f, 0.5f);
        glUniform1f(glGetUniformLocation(shader, "uAmbient"), 0.25f);

        glBindVertexArray(vao);
        glDrawArrays(GL_TRIANGLES, 0, indexCount);
        glBindVertexArray(0);
    } else {
        // Draw wireframe
        glBindVertexArray(wireVao);
        glDrawArrays(GL_LINES, 0, wireVertCount);
        glBindVertexArray(0);
    }
}

void GPUMesh::Destroy() {
    if (vao)     { glDeleteVertexArrays(1, &vao); vao = 0; }
    if (vboPos)  { glDeleteBuffers(1, &vboPos); vboPos = 0; }
    if (wireVao) { glDeleteVertexArrays(1, &wireVao); wireVao = 0; }
    if (wireVbo) { glDeleteBuffers(1, &wireVbo); wireVbo = 0; }
    indexCount = 0;
    wireVertCount = 0;
}

} // namespace isimpa
