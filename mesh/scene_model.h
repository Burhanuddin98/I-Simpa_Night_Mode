#pragma once

#include <glm/glm.hpp>
#include <glad/glad.h>
#include <string>
#include <vector>
#include <set>
#include <cstdint>

namespace isimpa {

// ─── Data structures matching I-Simpa's model ────────────────────────────────

struct Face {
    uint32_t v[3];       // Vertex indices
    glm::vec3 normal;    // Face normal
    int16_t materialId = -1;
    bool selected = false;
};

struct SurfaceGroup {
    std::string name;
    std::vector<Face> faces;
    int materialId = -1;
    glm::vec3 color{0.5f, 0.5f, 0.5f};
    float surfaceArea = 0.0f;
};

struct SceneModel {
    std::vector<glm::vec3> vertices;
    std::vector<SurfaceGroup> groups;

    // Bounding box
    glm::vec3 bbMin{0}, bbMax{0};
    glm::vec3 center{0};
    float extent = 1.0f;

    void ComputeBounds();
    void ComputeNormals();
    float ComputeGroupArea(int groupIdx);
    void Clear();
    bool IsEmpty() const { return vertices.empty(); }

    // Create a simple box room
    static SceneModel CreateBox(float width, float length, float height);
};

// ─── File loaders ────────────────────────────────────────────────────────────

bool LoadPLY(const std::string& path, SceneModel& model);
bool LoadSTL(const std::string& path, SceneModel& model);
bool Load3DS(const std::string& path, SceneModel& model);
bool LoadOBJ(const std::string& path, SceneModel& model);

// ─── GPU mesh for rendering ──────────────────────────────────────────────────

struct GPUMesh {
    GLuint vao = 0;
    GLuint vboPos = 0;   // interleaved pos+normal+color buffer
    int indexCount = 0;

    // Wireframe overlay
    GLuint wireVao = 0;
    GLuint wireVbo = 0;
    int wireVertCount = 0;

    void Upload(const SceneModel& model, const std::set<int>& selectedGroups = {});
    void Draw(GLuint shader, const glm::mat4& viewProj, bool wireframe);
    void Destroy();
};

// Shaders for mesh rendering
GLuint CreateMeshShader();
GLuint CreateWireShader();

} // namespace isimpa
