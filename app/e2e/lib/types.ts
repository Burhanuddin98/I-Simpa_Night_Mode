// The few shapes the specs build, kept local: the specs import only `node:` built-ins and
// relative files (PLAN.md 4.2). They follow app/ui/src/bindings/schema.ts's `Op`, loosely; the
// backend reads every op with the core's exact reader and refuses a wrong one with a code.
export type Vec3 = [number, number, number];

export type Op = { op: string } & Record<string, unknown>;

/** Environment the gate passes (tools/gates/m10.ps1). */
export function env(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`${name} unset: run tools/gates/m10.ps1`);
  return v;
}
