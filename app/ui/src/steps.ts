// The workflow (Concept B step bar). The self-test reads these back from the DOM.
export const STEPS = [
  { key: 'geometry', name: 'Geometry', sub: 'No model loaded', hint: 'Open a project to see its room model here.' },
  { key: 'materials', name: 'Materials', sub: 'Surface groups', hint: 'Pick a surface to set its material.' },
  { key: 'sources', name: 'Sources & receivers', sub: 'Inside the room', hint: 'Sources and receivers are placed inside the room.' },
  { key: 'simulate', name: 'Simulate', sub: 'Runs in the background', hint: 'A run starts only when every check before it passes.' },
  { key: 'results', name: 'Results', sub: 'Checked values only', hint: 'Only values with a passing physics check are shown.' },
] as const;

export type StepKey = (typeof STEPS)[number]['key'];
export const STEP_NAMES: readonly string[] = STEPS.map((s) => s.name);
