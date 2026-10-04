// The DOM contract's two marks (app/e2e/lib/acoustics.ts), shared by the Acoustics tab and its
// response window: a number of the report at its decimals, and a string of the report.
import type { Num, Str } from './model';

export function N({ n, unit }: { n: Num | null; unit?: string }) {
  if (!n) return <span className="ac-none">–</span>;
  return (
    <span className="ac-n">
      <span data-num data-json={n.path} data-digits={n.digits} data-scale={n.scale}>
        {n.text}
      </span>
      {unit ? <span className="ac-unit"> {unit}</span> : null}
    </span>
  );
}

export function S({ s, className }: { s: Str | null; className?: string }) {
  if (!s) return null;
  return (
    <span data-str data-json={s.path} className={className}>
      {s.text}
    </span>
  );
}
