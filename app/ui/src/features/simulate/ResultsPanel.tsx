// The Results step's properties panel (design:428-470): whether the selected run's results
// verify, and never a value in M11 (only M12 may show one). A stub from the M11 foundation,
// mounted by chrome/PropertiesPanel.tsx; the simulate package builds it
// (docs/investigations/2026-09-29-m11/PLAN.md 3.3, 9.1). It reads the stores; it takes no props.
export function ResultsPanel() {
  return (
    <>
      <div className="props-head">
        <div className="title">Results</div>
        <div className="sub">Checked values only</div>
      </div>
      <div className="props-body empty" data-part="results-stub">
        Only values with a passing physics check are shown.
      </div>
    </>
  );
}
