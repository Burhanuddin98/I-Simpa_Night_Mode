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
 * The page's visible text with every `[data-input]` and `[data-geometry]` element hidden: inputs
 * that carry units (a source's power) and geometry facts (volume, area) are allowed to show a
 * number next to a unit; nothing else is (PLAN.md 2.4, rule 2).
 */
export async function textOutsideInputsAndGeometry(): Promise<string> {
  return browser.execute(() => {
    const hidden = [...document.querySelectorAll<HTMLElement>('[data-input], [data-geometry]')];
    const before = hidden.map((el) => el.style.display);
    hidden.forEach((el) => (el.style.display = 'none'));
    try {
      return document.body.innerText;
    } finally {
      hidden.forEach((el, i) => (el.style.display = before[i]));
    }
  });
}

/** A solver-computed acoustic number: a digit next to dB, s, ms or % (PLAN.md 3, m10-h). */
export const ACOUSTIC_NUMBER = /\d\s*(dB|s|ms|%)(?![\p{L}\p{N}])/u;

/**
 * A room-acoustic parameter's name followed by a number, whatever its unit or none (M11 PLAN.md
 * 4.2 rule 2, closing M10 MINOR A-2: `STI 0.62 · D50 0.45` carries no unit, so
 * `ACOUSTIC_NUMBER` cannot see it). Read over the whole page, nothing hidden.
 */
export const PARAMETER_NUMBER = /\b(T15|T20|T30|EDT|RT|C50|C80|D50|Ts|STI|SPL|LF|LFC|G)\b\s*[:=]?\s*[-+]?\d/;

export async function clickSelector(selector: string): Promise<void> {
  const el = await $(selector);
  await el.waitForExist({ timeout: 30_000 });
  await el.click();
}
