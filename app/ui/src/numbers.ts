// Numbers typed or pasted by the user (PLAN.md 2.2). A pure module: no store, no backend, only
// erasable TypeScript, tested by numbers.test.ts under `node --test`.

/** The only decimal spelling the UI accepts: `0,5`, `NaN`, `Infinity`, hex and '' are refused. */
export const STRICT_DECIMAL = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;

/** The UI code of a cell or field that is not a strict decimal. */
export const NOT_A_NUMBER = 'NOT_A_NUMBER';

export type Parsed = { ok: true; value: number } | { ok: false; code: typeof NOT_A_NUMBER; text: string };

/**
 * Reads `text` (surrounding spaces ignored) as a strict decimal, correctly rounded (`Number()`,
 * as Rust's `str::parse::<f64>` is). A value that overflows to an infinity is refused too.
 */
export function parseStrictDecimal(text: string): Parsed {
  const t = text.trim();
  if (!STRICT_DECIMAL.test(t)) return { ok: false, code: NOT_A_NUMBER, text };
  const value = Number(t);
  if (!Number.isFinite(value)) return { ok: false, code: NOT_A_NUMBER, text };
  return { ok: true, value };
}
