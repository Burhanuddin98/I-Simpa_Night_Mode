#pragma once
#include <string>
#include <array>

namespace isimpa {

enum class WorkflowPhase {
    Room = 0,
    Materials,
    Sources,
    Receivers,
    Mesh,
    Simulate,
    Results,
    COUNT
};

enum class PhaseStatus {
    NotStarted, // Gray dot
    Incomplete, // Yellow dot
    Ready,      // Green dot
    Error       // Red dot
};

struct PhaseInfo {
    const char* label;
    const char* icon;       // Short icon text for the rail
    const char* tooltip;    // Hover tooltip
    PhaseStatus status = PhaseStatus::NotStarted;
    int         count  = 0; // e.g., "2 sources", "3 receivers"
};

class WorkflowRail {
public:
    WorkflowRail();

    // Draw the rail; returns true if the active phase changed
    bool Draw();

    WorkflowPhase GetActivePhase() const { return m_activePhase; }
    void SetActivePhase(WorkflowPhase phase) { m_activePhase = phase; }

    void SetPhaseStatus(WorkflowPhase phase, PhaseStatus status);
    void SetPhaseCount(WorkflowPhase phase, int count);
    PhaseInfo& GetPhase(WorkflowPhase phase);

private:
    WorkflowPhase m_activePhase = WorkflowPhase::Room;
    std::array<PhaseInfo, (size_t)WorkflowPhase::COUNT> m_phases;
};

} // namespace isimpa
