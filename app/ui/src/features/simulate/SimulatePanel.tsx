// The Simulate step's properties panel (design:386-426): the solver choice, the settings as
// drawn, "Before running", and the running block with Cancel. A stub from the M11 foundation,
// mounted by chrome/PropertiesPanel.tsx; the simulate package builds it
// (docs/investigations/2026-09-29-m11/PLAN.md 3.3, 9.1). It reads the stores; it takes no props.
import './simulate.css';

export function SimulatePanel() {
  return (
    <>
      <div className="props-head">
        <div className="title">Simulation</div>
        <div className="sub">Runs in the background · the app stays usable</div>
      </div>
      <div className="props-body empty" data-part="simulate-stub">
        A run starts only when every check before it passes.
      </div>
    </>
  );
}
