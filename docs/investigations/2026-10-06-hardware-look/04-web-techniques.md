# 04 - Web techniques for the hardware look (no canvas)

Research date 2026-10-06. Items marked (unverified) come from search snippets or background knowledge, not a fetched page.

## 1. Rotary knobs (SVG, DOM only)

**Recommendation: write our own ~80-line `<Knob>`; do not adopt a library.** Every candidate is stale, image-strip based, or lacks the full interaction set (vertical drag, wheel, shift-fine, dblclick reset, arrows, ARIA).

Pattern: a focusable `div role="slider"` wrapping an SVG; pointer capture for drag; value mapped to angle over a 270-degree sweep; the arc is one `<path>` with `pathLength={1}` and `strokeDasharray`.

```tsx
const SWEEP = 270;
export function Knob({value, min, max, step, def, label, fmt, onChange}: Props) {
  const drag = useRef<{y:number; v:number} | null>(null);
  const t = (value - min) / (max - min);
  const set = (v: number) => onChange(Math.min(max, Math.max(min, v)));
  const fine = (e: {shiftKey: boolean}) => (e.shiftKey ? 0.1 : 1);
  return (
    <div role="slider" tabIndex={0} aria-label={label} aria-valuemin={min} aria-valuemax={max}
      aria-valuenow={value} aria-valuetext={fmt(value)} className="knob"
      onDoubleClick={() => set(def)}
      onKeyDown={e => { const k = ({ArrowUp:1,ArrowRight:1,ArrowDown:-1,ArrowLeft:-1} as any)[e.key];
        if (k) { e.preventDefault(); set(value + k*step*fine(e)); }
        if (e.key==='Home') set(min); if (e.key==='End') set(max); }}
      onPointerDown={e => { e.currentTarget.setPointerCapture(e.pointerId); drag.current = {y:e.clientY, v:value}; }}
      onPointerMove={e => drag.current && set(drag.current.v + (drag.current.y-e.clientY)/(e.shiftKey?800:150)*(max-min))}
      onPointerUp={() => (drag.current = null)}>
      <svg viewBox="-20 -20 40 40" style={{touchAction:'none'}}>
        <path d="M-11.3 11.3A16 16 0 1 1 11.3 11.3" className="track" pathLength={1}/>
        <path d="M-11.3 11.3A16 16 0 1 1 11.3 11.3" className="val" pathLength={1} strokeDasharray={`${t} 1`}/>
        <line y1="-6" y2="-14" className="ptr" transform={`rotate(${-135 + t*SWEEP})`}/>
      </svg>
    </div>);
}
```
Wheel: React's `onWheel` is passive, so `preventDefault` fails; attach a native non-passive `wheel` listener via a ref (only while hovered/focused).

APG slider requirements ([W3C APG slider](https://www.w3.org/WAI/ARIA/apg/patterns/slider/)): `role=slider`, `aria-valuenow/min/max`, a label via `aria-label`/`aria-labelledby`; arrows step, Home/End jump, optional PageUp/Down; `aria-valuetext` when the number alone is unclear (give "12.5 dB", "250 Hz"). APG warns touch-based assistive tech may struggle with sliders, so keyboard and text entry should exist too.

Libraries considered:
| Library | Licence | Last release | Verdict |
|---|---|---|---|
| [react-dial-knob](https://registry.npmjs.org/react-dial-knob) (HTML/SVG, TS, no deps, 5 themes) | MIT | 1.3.0 published 2021-01-31; Snyk snippet calls it discontinued ([search](https://snyk.io/advisor/npm-package/react-dial-knob)) | Closest fit but unmaintained; use as reference only |
| [webaudio-controls](https://github.com/g200kg/webaudio-controls) (web components, sample image-based knobs) | Apache-2.0 (compatible with GPL-3) | not shown on the repo page | No keyboard/a11y documented; image strips fight the CSS/SVG goal |
| [NexusUI](https://registry.npmjs.org/nexusui) | MIT | 2.1.6, 2022-06-21; [repo](https://github.com/nexus-js/ui) says maintainers wanted | Render backend not confirmed (unverified); risk of canvas, so excluded |
| rc-knob, @nickyrc/react-knob, Tone.js UI examples | not reached (rc-knob URL 404) | - | unverified |

Pitfalls: needs pointer capture or drag dies outside the element; `touch-action:none`; `user-select:none`; coalesce updates with rAF if they drive the 3D view; no easing on direct manipulation, and any value-sweep animation goes under `prefers-reduced-motion: no-preference`.

## 2. LED and LCD readouts in CSS

**Recommendation: DSEG7 Classic (OFL) bundled as woff2, with a ghost layer of `8`s behind the digits.**
[DSEG](https://github.com/keshikan/DSEG): SIL OFL 1.1, GitHub states v0.50beta, npm `dseg` is 0.46.0 ([registry](https://registry.npmjs.org/dseg/latest)); 7- and 14-segment, Classic and Modern, TTF/WOFF/WOFF2. Special glyphs: `8` or `~` lights every segment, `!` lights none; colons and spaces have equal width, periods zero width (a decimal point does not shift layout). A search snippet showed one project deriving the "off" colour with `color-mix(in srgb, <on> 10%, transparent)` ([search](https://github.com/keshikan/DSEG/blob/master/sample.html) is the official sample).

```css
.lcd{position:relative;font:700 28px/1 'DSEG7 Classic',monospace;white-space:pre;
  color:var(--lcd-on);text-shadow:0 0 6px color-mix(in srgb,var(--lcd-on) 60%,transparent)}
.lcd::before{content:attr(data-ghost);position:absolute;inset:0;
  color:color-mix(in srgb,var(--lcd-on) 9%,transparent);text-shadow:none}
/* data-ghost="88.8" has the same digit count as the value; right-align both */
```
LED lens:
```css
.led{width:10px;height:10px;border-radius:50%;
  background:radial-gradient(circle at 35% 30%,#fff9 0 12%,transparent 30%),var(--led-off)}
.led[data-on]{background:radial-gradient(circle at 35% 30%,#fff 0 10%,transparent 32%),var(--led-on);
  box-shadow:0 0 6px 1px var(--led-on),inset 0 -1px 2px #0006}
@media(prefers-reduced-motion:no-preference){.led[data-blink]{animation:blink 1s steps(1) infinite}}
@keyframes blink{50%{opacity:.25}}
```
Pitfalls: ghost and value must have equal length; 7-segment letters are poor, keep numerals only and put the unit/label in Inter; glow via `text-shadow`/`box-shadow` is cheap for a handful of elements, costly for many animated ones; blinking must not be the only cue (steady on/off differs by shape or text) and stay under 3 flashes per second (WCAG 2.3.1); mark the ghost `aria-hidden` and expose the value as real text.

## 3. Brushed metal / anodised aluminium

**Recommendation: CSS gradients for panels; if a grain is needed, rasterise it once to a small tile at build time; avoid live feTurbulence.**
Hairlines: stack `repeating-linear-gradient`s with different periods so the pattern looks random ([simurai](https://simurai.com/lab/2011/08/21/brushed-metal), [CodePen oobleck](https://codepen.io/oobleck/pen/MYBvGa)).
```css
.bar{background:
  repeating-linear-gradient(90deg,#fff0 0 1px,#ffffff0d 1px 2px,#fff0 2px 4px),
  repeating-linear-gradient(90deg,#0000 0 3px,#0000001a 3px 4px,#0000 4px 7px),
  linear-gradient(#3a3d42,#26282c)}
.cap{border-radius:50%;background:
  repeating-conic-gradient(#0000 0 2deg,#ffffff10 2deg 3deg),
  conic-gradient(#555,#bbb 12%,#555 25%,#aaa 40%,#555 55%,#bbb 70%,#555 85%,#555);
  box-shadow:inset 0 0 0 1px #0008,0 1px 0 #fff2}
```
feTurbulence: a two-value `baseFrequency="0.02 0.9"` streaks the noise into brushed grain ([codefronts](https://codefronts.com/design-styles/css-grain-texture/svg-feturbulence-noise-parameter-breakdown/)). It is a per-pixel filter re-evaluated on repaint; cost grows with filter region size and `numOctaves` (keep at most 2) ([perf tips](https://imagetosvg.com/how-to/svg-filter-performance-tips), [search results](https://css-tricks.com/creating-patterns-with-svg-filters/)). I did not measure it in WebView2. A pre-rendered PNG/WebP tile used as `background-image` costs nothing at runtime and, being a `url()` image, survives forced-colors.
Pitfalls: text over hairlines loses contrast, so put labels on a flat plate or scrim and keep hairline alpha at or below ~6% under text; no transitions on `background`; combining textures with `backdrop-filter` glass over the 3D view stacks paint cost, so pick one.

## 4. Toggle switches and segmented LED meters

Toggle: a real `<button role="switch" aria-checked>` with a CSS-drawn lever.
```css
.sw{width:28px;height:40px;border-radius:6px;background:var(--well);position:relative}
.sw::after{content:"";position:absolute;left:10px;top:14px;width:8px;height:20px;border-radius:4px;
  background:linear-gradient(90deg,#888,#eee 40%,#777);transform-origin:50% 20%;transform:rotate(-24deg)}
.sw[aria-checked=true]::after{transform:rotate(24deg)}
@media(prefers-reduced-motion:no-preference){.sw::after{transition:transform .12s}}
```
Meter: render N `<i data-on>` cells in a grid (React sets `data-on` per cell), colour by position.
```css
.meter{display:grid;grid-template-rows:repeat(12,1fr);gap:2px}
.meter i{background:var(--cell-off)}
.meter i[data-on]{background:var(--amber);box-shadow:0 0 4px var(--amber)}
.meter i[data-on]:nth-last-child(-n+8){background:var(--green)}
```
Note `:nth-child(-n + var(--n))` does not work (custom properties are not allowed inside an+b), so use per-cell attributes. Peak-hold decay is driven from JS (rAF), not CSS animation; under reduced motion show the instantaneous value with no hold or fade. Always show the number as text too (colour or level alone fails WCAG 1.4.1).

## 5. Patch cables / jacks

Technique: a full-window absolutely-positioned `<svg>` with `pointer-events:none`; measure jack centres with `getBoundingClientRect()` and re-measure on `ResizeObserver` and layout changes; draw a cubic bezier whose sag scales with distance.
```tsx
const d = (a:P, b:P) => { const s = Math.min(120, Math.hypot(b.x-a.x, b.y-a.y)*.4);
  return `M${a.x},${a.y} C${a.x},${a.y+s} ${b.x},${b.y+s} ${b.x},${b.y}`; };
<path d={d(a,b)} stroke="#0008" strokeWidth="7" fill="none" strokeLinecap="round"/>
<path d={d(a,b)} stroke="var(--cable)" strokeWidth="5" fill="none" strokeLinecap="round"/>
```
VCV Rack's cables use the same bezier-with-droop idea but are native rendering ([Rack manual](https://vcvrack.com/manual/GettingStarted)); I found no reusable web component ([VcvPatchDiagram](https://github.com/neilbgr/VcvPatchDiagram) is a diagram tool). cables.gl and patch-bay demos were not reached (unverified).
Worth it for source-to-receiver selection? Only as decoration. A segmented selector or two selects is faster, fully keyboard-operable and survives forced-colors. If kept, the cable is `aria-hidden` and the real control is a list; drag-to-connect needs a non-drag alternative (WCAG 2.2 SC 2.5.7, unverified here); cable and jack must hold 3:1 against the panel.

## 6. Accessibility and legibility

- 1.4.3: text 4.5:1 (3:1 large). Lit LCD digits are fine; unlit ghost segments are decorative. Engraved low-contrast panel labels are the usual failure: measure against the lightest point of the brushed gradient.
- 1.4.11: UI component boundaries and states need 3:1 against adjacent colours; inactive controls are exempt ([Understanding 1.4.11](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)). Knob pointer or value arc, switch lever and focus ring must each reach 3:1 against the panel; a dark-on-dark cap edge defined only by a soft shadow fails. Use `:focus-visible{outline:2px solid #fff;outline-offset:2px}`, not a glow.
- 2.5.8: targets at least 24x24 CSS px, or the 24px-circle spacing exception ([Understanding 2.5.8](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)). Small toggles and jacks get an enlarged hit area via `::before{content:"";position:absolute;inset:-6px}`.
- Forced colors ([MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/forced-colors)): `box-shadow` and `text-shadow` become none, `background-image` becomes none (except `url()` images), colours, borders, outlines and SVG fill/stroke take system colours; `forced-color-adjust:none` opts out. So glow LEDs, gradient caps and hairline metal vanish, and the DSEG ghost layer would render as full-strength `8`s: hide it. MDN advises small tweaks, not a separate design.
```css
@media (forced-colors:active){
  .lcd::before{display:none}
  .knob{border:2px solid ButtonText} .knob .val,.knob .ptr{stroke:Highlight}
  .led{border:2px solid ButtonText} .led[data-on]{background:Highlight}
  .sw{border:2px solid ButtonText} .sw[aria-checked=true]::after{background:Highlight}
  .meter i{border:1px solid ButtonText} .meter i[data-on]{background:Highlight}
}
```
Rule: every state must be carried by something other than shadow or gradient (border, fill, text, shape). Test with DevTools Rendering emulation for `forced-colors: active` and `prefers-reduced-motion`; keep all motion inside `@media (prefers-reduced-motion: no-preference)` so the default is still.

## Not reached
rc-knob and @nickyrc/react-knob pages, webaudio-controls release date, NexusUI render backend, cables.gl, and any measured feTurbulence cost in WebView2.
