// R72: a chart's axes set by hand (chartAxes.ts reads what is typed). Opened from the chart card's
// "Axes" button; Enter or Apply sets them, Auto gives the chart its own range back. A typing error
// is said beside the fields and changes nothing.
import { useState } from 'react';
import { type AxesText, type HandAxes, handAxes } from './chartAxes';

const text = (v: number | undefined) => (v === undefined ? '' : String(v));

export function AxesEditor({ x, xName, yName, value, onChange }: { x: boolean; xName: string; yName: string; value: HandAxes; onChange: (a: HandAxes) => void }) {
  const [t, setT] = useState<AxesText>({
    xFrom: text(value.x?.[0]),
    xTo: text(value.x?.[1]),
    yFrom: text(value.y?.[0]),
    yTo: text(value.y?.[1]),
    yStep: text(value.yStep),
  });
  const [error, setError] = useState<string | null>(null);
  const apply = () => {
    const r = handAxes(t);
    setError(r.error);
    if (r.axes) onChange(r.axes);
  };
  const field = (k: keyof AxesText, label: string) => (
    <input
      className="ac-axis-num"
      type="text"
      inputMode="decimal"
      data-axis-field={k}
      aria-label={label}
      value={t[k] ?? ''}
      onChange={(e) => setT({ ...t, [k]: e.target.value })}
      onKeyDown={(e) => e.key === 'Enter' && apply()}
    />
  );
  return (
    <div className="ac-axes" data-part="axes-editor">
      {x ? (
        <span className="ac-axis">
          <span className="k">{xName}</span> from {field('xFrom', `${xName} from`)} to {field('xTo', `${xName} to`)}
        </span>
      ) : null}
      <span className="ac-axis">
        <span className="k">{yName}</span> from {field('yFrom', `${yName} from`)} to {field('yTo', `${yName} to`)} ticks every {field('yStep', `${yName} tick spacing`)}
      </span>
      <button type="button" className="small-button ac-copy" data-action="axes-apply" onClick={apply}>
        Apply
      </button>
      <button
        type="button"
        className="small-button ac-copy"
        data-action="axes-auto"
        title="The chart's own range and ticks"
        onClick={() => {
          setT({});
          setError(null);
          onChange({});
        }}
      >
        Auto
      </button>
      {error ? (
        <span className="ac-axes-error" data-part="axes-error" data-label="control" role="alert">
          {error}
        </span>
      ) : null}
    </div>
  );
}
