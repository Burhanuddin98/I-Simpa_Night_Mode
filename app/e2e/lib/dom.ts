// DOM reads for the gates: what a gate says is "shown" is read here, from the page.

/** The text of every Console line of `cls` (FAIL, WARN, INFO, OK), in order. */
export async function consoleLines(cls: 'FAIL' | 'WARN' | 'INFO' | 'OK'): Promise<string[]> {
  return browser.execute(
    (c: string) => [...document.querySelectorAll(`.console-line.${c} .text`)].map((e) => e.textContent ?? ''),
    cls,
  );
}

/** Every Console line as `CLASS text`. */
export async function allConsoleLines(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('.console-line')].map(
      (e) => `${e.querySelector('.tag')?.textContent ?? '?'} ${e.querySelector('.text')?.textContent ?? ''}`,
    ),
  );
}

/** The step bar's step names, read the way the M9 self-test reads them. */
export async function stepNames(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[data-step]')].map((el) => el.querySelector('[data-part="name"]')?.textContent ?? ''),
  );
}

/**
 * Where the two exemptions hold (M11 review F2). An element marked `[data-input]` (an input
 * that carries a unit: a source's power, a setting) or `[data-geometry]` (a geometry fact: a
 * volume, an area, a dimension) may show a number next to a unit only inside one of its regions
 * here: each attribute, region name to selector, the element or an ancestor matching it. The
 * same attribute anywhere else is not exempt: its text is read like any other, and m11-h flags
 * the element itself (rule 1x). Before this list, either attribute hid its text wherever it
 * was, so `<span data-geometry>Sabine 1.52 s</span>` in any panel passed m10-h, m11-h and
 * m11-sim-numbers (the review's mutation M13).
 *
 * Every use in `app/ui/src` sits in one of these regions: the Geometry panel's fact sections,
 * the status bar's model fact, the plan inset's dimensions, the Materials panel (its group's
 * area line; the names, grid and library list as inputs), the scene list's rows, the Sources
 * panel and the Simulate settings. A new use elsewhere fails the checks until it is listed here.
 */
export const EXEMPT_REGIONS: Record<'data-input' | 'data-geometry', Record<string, string>> = {
  'data-geometry': {
    'geometry-panel': '[data-props-step="geometry"]',
    'statusbar-model-fact': '.statusbar [data-part="model-fact"]',
    'plan-inset': '[data-part="viewport"] [data-part="plan-inset"]',
    'material-group': '[data-props-step="materials"] [data-part="material-group"]',
  },
  'data-input': {
    'scene-list': '[data-part="scene-list"]',
    'sources-panel': '[data-props-step="sources"]',
    'materials-panel': '[data-props-step="materials"]',
    'simulate-settings': '[data-props-step="simulate"] [data-part="settings"]',
  },
};

/**
 * The page's visible text with every `[data-input]` and `[data-geometry]` element hidden that
 * sits in its own region (`EXEMPT_REGIONS`): inputs that carry units (a source's power) and
 * geometry facts (volume, area) may show a number next to a unit there; nothing else may
 * (PLAN.md 2.4, rule 2). An exempt attribute outside its regions hides nothing.
 */
export async function textOutsideInputsAndGeometry(): Promise<string> {
  return browser.execute((regions: Record<string, Record<string, string>>) => {
    const hidden: HTMLElement[] = [];
    for (const attr of Object.keys(regions)) {
      for (const el of document.querySelectorAll<HTMLElement>(`[${attr}]`)) {
        if (Object.values(regions[attr]).some((sel) => el.closest(sel) !== null)) hidden.push(el);
      }
    }
    const before = hidden.map((el) => el.style.display);
    hidden.forEach((el) => (el.style.display = 'none'));
    try {
      return document.body.innerText;
    } finally {
      hidden.forEach((el, i) => (el.style.display = before[i]));
    }
  }, EXEMPT_REGIONS);
}

/** A solver-computed acoustic number: a digit next to dB, s, ms or % (PLAN.md 3, m10-h). */
export const ACOUSTIC_NUMBER = /\d\s*(dB|s|ms|%)(?![\p{L}\p{N}])/u;

/**
 * A room-acoustic parameter's name followed by a number, whatever its unit or none (M11 PLAN.md
 * 4.2 rule 2, closing M10 MINOR A-2: `STI 0.62 · D50 0.45` carries no unit, so
 * `ACOUSTIC_NUMBER` cannot see it). Read over the whole page, nothing hidden. The names include
 * the reverberation time's other spellings (RT60, T60, Sabine, Eyring, "reverberation time"; M11
 * review F2), and a `·` may sit between a name and its number. The short codes are matched in
 * capitals only, so words such as "g" or "rt" are not read as parameters.
 */
export const PARAMETER_NUMBER =
  /(?:\b(?:T15|T20|T30|T60|RT60|EDT|RT|C50|C80|D50|Ts|STI|SPL|LF|LFC|G)\b|\b(?:[Ss]abine|[Ee]yring|[Rr]everberation time)\b)\s*[:=·]?\s*[-+]?\d/;

export async function clickSelector(selector: string): Promise<void> {
  const el = await $(selector);
  await el.waitForExist({ timeout: 30_000 });
  await el.click();
}
