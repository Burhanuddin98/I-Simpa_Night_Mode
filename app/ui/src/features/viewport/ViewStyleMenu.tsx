// The 3D view's style menu (Burhan 2026-10-06: "any drop down option that cleanly allows for the 3D
// view style to be changed as per the users needs"): surfaces in colour, grey, see-through with the
// near walls' opacity, or the old wireframe; edges at every triangle or only the features. The
// choice is `viewStyle` in the engine, remembered per viewer.
import { useEffect, useRef, useState } from 'react';
import { useStore } from '../../store';
import { viewStyle, type EdgeStyle, type SurfaceStyle } from './engine';

const SURFACES: { key: SurfaceStyle; label: string; hint: string }[] = [
  { key: 'colour', label: 'Colour', hint: 'Each surface in its material colour, shaded by its angle' },
  { key: 'grey', label: 'Grey', hint: 'Shaded by angle, without the material colours' },
  { key: 'glass', label: 'See-through', hint: 'The near walls as glass, so the whole room shows from outside' },
  { key: 'wire', label: 'Wireframe', hint: 'Dark faces under the edges, the earlier look' },
];
const EDGES: { key: EdgeStyle; label: string; hint: string }[] = [
  { key: 'all', label: 'Every triangle', hint: 'Every edge of the mesh' },
  { key: 'feature', label: 'Features only', hint: 'Only where surfaces meet at 20 degrees or more' },
];

export function ViewStyleMenu() {
  const style = useStore(viewStyle);
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    window.addEventListener('mousedown', away);
    window.addEventListener('keydown', esc);
    return () => {
      window.removeEventListener('mousedown', away);
      window.removeEventListener('keydown', esc);
    };
  }, [open]);

  const set = (patch: Partial<typeof style>) => viewStyle.set({ ...viewStyle.get(), ...patch });

  return (
    <div className="view-style" ref={root}>
      <button
        className="view-style-button float-panel"
        data-part="view-style"
        aria-haspopup="true"
        aria-expanded={open}
        title="How the room is drawn"
        onClick={() => setOpen((o) => !o)}
      >
        Style <span aria-hidden>▾</span>
      </button>
      {open && (
        <div className="view-style-menu float-panel" role="menu" data-part="view-style-menu">
          <div className="view-style-head">Surfaces</div>
          {SURFACES.map((s) => (
            <button
              key={s.key}
              role="menuitemradio"
              aria-checked={style.surfaces === s.key}
              data-surfaces={s.key}
              title={s.hint}
              onClick={() => set({ surfaces: s.key })}
            >
              {s.label}
            </button>
          ))}
          {style.surfaces === 'glass' && (
            <label className="view-style-glass">
              Near walls
              <input
                type="range"
                min={0}
                max={60}
                value={style.glass}
                aria-label="Near walls' opacity, percent"
                onChange={(e) => set({ glass: Number(e.target.value) })}
              />
              <span>{style.glass} %</span>
            </label>
          )}
          <div className="view-style-head">Edges</div>
          {EDGES.map((e) => (
            <button
              key={e.key}
              role="menuitemradio"
              aria-checked={style.edges === e.key}
              data-edges={e.key}
              title={e.hint}
              onClick={() => set({ edges: e.key })}
            >
              {e.label}
            </button>
          ))}
          <div className="view-style-head">Shading</div>
          <button
            role="menuitemcheckbox"
            aria-checked={style.corners}
            data-corners=""
            title="Corners, the floor under balconies and stair steps drawn darker, so the room reads in depth"
            onClick={() => set({ corners: !style.corners })}
          >
            Darker corners
          </button>
        </div>
      )}
    </div>
  );
}
