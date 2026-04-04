#include "viewport/camera.h"
#include <algorithm>
#include <cmath>

namespace isimpa {

Camera::Camera() {
    UpdateFromSpherical();
}

glm::mat4 Camera::GetViewMatrix() const {
    return glm::lookAt(m_position, m_target, m_up);
}

void Camera::SetPosition(const glm::vec3& pos) {
    m_position = pos;
    glm::vec3 dir = m_position - m_target;
    m_distance = glm::length(dir);
    if (m_distance > 0.001f) {
        dir /= m_distance;
        m_pitch = glm::degrees(asinf(std::clamp(dir.y, -1.0f, 1.0f)));
        m_yaw = glm::degrees(atan2f(dir.x, dir.z));
    }
}

void Camera::SetTarget(const glm::vec3& target) {
    m_target = target;
    SetPosition(m_position);
}

// ─── Orbit: drag to rotate around target ───────────────────────────────────
// Responsive, scales with screen size. Works on left-drag OR right-drag.

void Camera::ProcessMouseOrbit(float dx, float dy) {
    m_yaw   -= dx * m_orbitSpeed;
    m_pitch += dy * m_orbitSpeed;
    m_pitch = std::clamp(m_pitch, -89.0f, 89.0f);
    UpdateFromSpherical();
}

// ─── Pan: shift+drag or middle-drag ────────────────────────────────────────
// Speed scales with distance for consistent feel at any zoom level.

void Camera::ProcessMousePan(float dx, float dy) {
    glm::vec3 forward = glm::normalize(m_target - m_position);
    glm::vec3 right = glm::normalize(glm::cross(forward, m_up));
    glm::vec3 up = glm::normalize(glm::cross(right, forward));

    float scale = m_distance * m_panSpeed;
    glm::vec3 offset = -right * dx * scale + up * dy * scale;
    m_position += offset;
    m_target += offset;
}

// ─── Zoom: scroll wheel ───────────────────────────────────────────────────
// Multiplicative — feels consistent at any distance.

void Camera::ProcessMouseScroll(float delta) {
    if (delta == 0) return;
    float factor = 1.0f - delta * m_zoomSpeed;
    factor = std::clamp(factor, 0.3f, 3.0f);
    m_distance *= factor;
    m_distance = std::clamp(m_distance, 0.05f, 5000.0f);
    UpdateFromSpherical();
}

// ─── WASD fly mode ─────────────────────────────────────────────────────────

void Camera::ProcessKeyboard(float dt, bool forward, bool back, bool left, bool right,
                             bool up, bool down, bool sprint) {
    float speed = m_flySpeed * dt;
    if (sprint) speed *= 3.0f;

    glm::vec3 fwd = glm::normalize(m_target - m_position);
    glm::vec3 rgt = glm::normalize(glm::cross(fwd, m_up));

    glm::vec3 move(0);
    if (forward) move += fwd;
    if (back)    move -= fwd;
    if (right)   move += rgt;
    if (left)    move -= rgt;
    if (up)      move += m_up;
    if (down)    move -= m_up;

    if (glm::length(move) > 0.001f) {
        move = glm::normalize(move) * speed;
        m_position += move;
        m_target += move;
    }
}

void Camera::FocusOnPoint(const glm::vec3& point) {
    m_target = point;
    m_distance = std::max(2.0f, m_distance * 0.3f);
    UpdateFromSpherical();
}

void Camera::UpdateFromSpherical() {
    float pitchRad = glm::radians(m_pitch);
    float yawRad = glm::radians(m_yaw);

    m_position.x = m_target.x + m_distance * cosf(pitchRad) * sinf(yawRad);
    m_position.y = m_target.y + m_distance * sinf(pitchRad);
    m_position.z = m_target.z + m_distance * cosf(pitchRad) * cosf(yawRad);
}

// ─── Preset views ──────────────────────────────────────────────────────────

void Camera::SetTopView()   { m_yaw = 0; m_pitch = 89.0f; UpdateFromSpherical(); }
void Camera::SetFrontView() { m_yaw = 0; m_pitch = 0; UpdateFromSpherical(); }
void Camera::SetRightView() { m_yaw = 90.0f; m_pitch = 0; UpdateFromSpherical(); }
void Camera::ResetView()    { m_yaw = 45.0f; m_pitch = 30.0f; m_distance = 14.0f; UpdateFromSpherical(); }

} // namespace isimpa
