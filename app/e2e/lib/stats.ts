// Distributions for gate (b) (PLAN.md 4.1): nearest-rank percentiles, and the one-line summary
// the gate prints beside each threshold.

/** The nearest-rank percentile of `xs` (unsorted is fine): the value at rank ceil(q · n), 1-based.
 * p99 of 200 values is the 198th smallest. */
export function nearestRank(xs: readonly number[], q: number): number {
  if (xs.length === 0) throw new Error('nearestRank: no values');
  if (!(q > 0 && q <= 1)) throw new Error(`nearestRank: q must be in (0, 1], got ${q}`);
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.ceil(q * s.length) - 1];
}

/** `n, min, median, p95, p99, max` in ms, to 0.01 ms. */
export function distribution(xs: readonly number[]): string {
  const f = (v: number) => v.toFixed(2);
  return (
    `n ${xs.length}, min ${f(Math.min(...xs))}, median ${f(nearestRank(xs, 0.5))}, ` +
    `p95 ${f(nearestRank(xs, 0.95))}, p99 ${f(nearestRank(xs, 0.99))}, max ${f(Math.max(...xs))} ms`
  );
}
