// The Runs tab: M9's empty run history, until M11 wires Run.
// Hand-over stub from the M10 foundation; the scene package owns it from here.
export function RunsPane() {
  return (
    <div className="runs">
      <div className="runs-head label">
        <span>Run</span>
        <span>Variant</span>
        <span>Solver</span>
        <span>Status</span>
        <span>Check</span>
      </div>
      <div className="runs-empty empty">No runs yet.</div>
    </div>
  );
}
