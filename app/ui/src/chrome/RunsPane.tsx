// The Runs tab (design:259-274): the run history's table, empty until M11 wires Run.
export function RunsPane() {
  return (
    <div className="runs" data-part="runs">
      <div className="runs-head label" role="row">
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
