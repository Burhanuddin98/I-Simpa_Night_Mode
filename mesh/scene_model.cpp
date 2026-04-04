#include "mesh/scene_model.h"
#include <algorithm>
#include <cmath>

namespace isimpa {

void SceneModel::ComputeBounds() {
    if (vertices.empty()) return;
    bbMin = bbMax = vertices[0];
    for (auto& v : vertices) {
        bbMin = glm::min(bbMin, v);
        bbMax = glm::max(bbMax, v);
    }
    center = (bbMin + bbMax) * 0.5f;
    extent = glm::length(bbMax - bbMin) * 0.5f;
    if (extent < 0.001f) extent = 1.0f;
}

void SceneModel::ComputeNormals() {
    for (auto& group : groups) {
        for (auto& face : group.faces) {
            if (face.v[0] < vertices.size() && face.v[1] < vertices.size() && face.v[2] < vertices.size()) {
                glm::vec3 a = vertices[face.v[0]];
                glm::vec3 b = vertices[face.v[1]];
                glm::vec3 c = vertices[face.v[2]];
                glm::vec3 n = glm::cross(b - a, c - a);
                float len = glm::length(n);
                face.normal = (len > 1e-8f) ? n / len : glm::vec3(0, 1, 0);
            }
        }
    }
}

float SceneModel::ComputeGroupArea(int groupIdx) {
    if (groupIdx < 0 || groupIdx >= (int)groups.size()) return 0;
    float area = 0;
    for (auto& face : groups[groupIdx].faces) {
        glm::vec3 a = vertices[face.v[0]];
        glm::vec3 b = vertices[face.v[1]];
        glm::vec3 c = vertices[face.v[2]];
        area += 0.5f * glm::length(glm::cross(b - a, c - a));
    }
    groups[groupIdx].surfaceArea = area;
    return area;
}

void SceneModel::Clear() {
    vertices.clear();
    groups.clear();
    bbMin = bbMax = center = glm::vec3(0);
    extent = 1.0f;
}

SceneModel SceneModel::CreateBox(float w, float l, float h) {
    SceneModel m;

    // 8 vertices of a box centered at (w/2, h/2, l/2) so origin is a corner
    m.vertices = {
        {0, 0, 0}, {w, 0, 0}, {w, 0, l}, {0, 0, l},  // floor
        {0, h, 0}, {w, h, 0}, {w, h, l}, {0, h, l},  // ceiling
    };

    // Floor (Y=0)
    SurfaceGroup floor;
    floor.name = "Floor";
    floor.color = {0.4f, 0.4f, 0.45f};
    floor.faces.push_back({{0, 2, 1}, {0,-1,0}});
    floor.faces.push_back({{0, 3, 2}, {0,-1,0}});
    m.groups.push_back(floor);

    // Ceiling (Y=h)
    SurfaceGroup ceiling;
    ceiling.name = "Ceiling";
    ceiling.color = {0.6f, 0.6f, 0.65f};
    ceiling.faces.push_back({{4, 5, 6}, {0,1,0}});
    ceiling.faces.push_back({{4, 6, 7}, {0,1,0}});
    m.groups.push_back(ceiling);

    // Front wall (Z=0)
    SurfaceGroup front;
    front.name = "Wall Front";
    front.color = {0.45f, 0.35f, 0.35f};
    front.faces.push_back({{0, 1, 5}, {0,0,-1}});
    front.faces.push_back({{0, 5, 4}, {0,0,-1}});
    m.groups.push_back(front);

    // Back wall (Z=l)
    SurfaceGroup back;
    back.name = "Wall Back";
    back.color = {0.45f, 0.35f, 0.35f};
    back.faces.push_back({{2, 3, 7}, {0,0,1}});
    back.faces.push_back({{2, 7, 6}, {0,0,1}});
    m.groups.push_back(back);

    // Left wall (X=0)
    SurfaceGroup left;
    left.name = "Wall Left";
    left.color = {0.35f, 0.45f, 0.35f};
    left.faces.push_back({{3, 0, 4}, {-1,0,0}});
    left.faces.push_back({{3, 4, 7}, {-1,0,0}});
    m.groups.push_back(left);

    // Right wall (X=w)
    SurfaceGroup right;
    right.name = "Wall Right";
    right.color = {0.35f, 0.45f, 0.35f};
    right.faces.push_back({{1, 2, 6}, {1,0,0}});
    right.faces.push_back({{1, 6, 5}, {1,0,0}});
    m.groups.push_back(right);

    m.ComputeBounds();
    m.ComputeNormals();
    for (int i = 0; i < (int)m.groups.size(); i++) m.ComputeGroupArea(i);

    return m;
}

} // namespace isimpa
