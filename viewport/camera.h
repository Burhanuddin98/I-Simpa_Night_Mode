#pragma once

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>

namespace isimpa {

class Camera {
public:
    Camera();

    glm::mat4 GetViewMatrix() const;

    void SetPosition(const glm::vec3& pos);
    void SetTarget(const glm::vec3& target);

    // Mouse controls
    void ProcessMouseOrbit(float dx, float dy);
    void ProcessMousePan(float dx, float dy);
    void ProcessMouseScroll(float delta);

    // WASD first-person fly mode
    void ProcessKeyboard(float dt, bool forward, bool back, bool left, bool right,
                         bool up, bool down, bool sprint);

    // Focus on a specific world point (smooth zoom toward it)
    void FocusOnPoint(const glm::vec3& point);

    glm::vec3 GetPosition() const { return m_position; }
    glm::vec3 GetTarget() const { return m_target; }
    float GetDistance() const { return m_distance; }

    // Preset views (relative to current target)
    void SetTopView();
    void SetFrontView();
    void SetRightView();
    void ResetView();

private:
    void UpdateFromSpherical();

    glm::vec3 m_position{8.0f, 6.0f, 8.0f};
    glm::vec3 m_target{0.0f, 0.0f, 0.0f};
    glm::vec3 m_up{0.0f, 1.0f, 0.0f};

    // Orbit parameters
    float m_distance = 14.0f;
    float m_yaw   = 45.0f;   // degrees
    float m_pitch = 30.0f;   // degrees

    // Tuned speeds (responsive like Blender/SketchUp)
    float m_orbitSpeed = 0.5f;   // degrees per pixel — snappy orbit
    float m_panSpeed   = 0.008f; // pan scales with distance — responsive
    float m_zoomSpeed  = 0.25f;  // 25% per scroll tick — noticeable zoom
    float m_flySpeed   = 12.0f;  // m/s base — fast WASD
};

} // namespace isimpa
