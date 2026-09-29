// Validator issues by entity and by field (PLAN.md 2.2). A pure module, tested by
// issues.test.ts under `node --test`.
import type { UiIssue } from './bindings/ipc.ts';

/**
 * The key a field's issues and refusals are filed under: `${kind}:${id}:${field}`, for example
 * `point_receiver:<uuid>:position` or `material:<uuid>:absorption/3`. `field` is empty for the
 * entity itself.
 */
export function fieldKey(kind: string, id: string, field = ''): string {
  return `${kind}:${id}:${field}`;
}

/** The field key of an issue, or null when its pointer names no entity. */
export function issueKey(issue: UiIssue): string | null {
  return issue.entity ? fieldKey(issue.entity.kind, issue.entity.id, issue.field) : null;
}

/** Issues grouped by `${kind}:${id}`, each entity's in validator order. */
export function issuesByEntity(issues: readonly UiIssue[]): Map<string, UiIssue[]> {
  const out = new Map<string, UiIssue[]>();
  for (const i of issues) {
    if (!i.entity) continue;
    const k = `${i.entity.kind}:${i.entity.id}`;
    const list = out.get(k);
    if (list) list.push(i);
    else out.set(k, [i]);
  }
  return out;
}

/** The issues of one field of one entity. A field also collects its sub-fields' issues
 * (`position` takes `position/0`). */
export function issuesForField(issues: readonly UiIssue[], kind: string, id: string, field: string): UiIssue[] {
  return issues.filter(
    (i) =>
      i.entity?.kind === kind &&
      i.entity.id === id &&
      (i.field === field || i.field.startsWith(`${field}/`)),
  );
}

/** Issues whose pointer names no entity (band set, environment, solver settings, ...). */
export function projectIssues(issues: readonly UiIssue[]): UiIssue[] {
  return issues.filter((i) => !i.entity);
}
