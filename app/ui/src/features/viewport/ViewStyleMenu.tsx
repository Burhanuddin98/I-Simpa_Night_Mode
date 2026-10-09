// The 3D view's style menu (Burhan 2026-10-06: "any drop down option that cleanly allows for the 3D
// view style to be changed as per the users needs"): surfaces in colour, grey, see-through with the
// near walls' opacity, or the old wireframe; edges at every triangle or only the features. The
// choice is `viewStyle` in the engine, remembered per viewer.
//
// Parity G43: Faces, upstream's View > Faces (faces.ts): Inside (the near walls drop away, the
// default), Outside (every face, the room as a closed shell) or None (the edges only). See-through
// is glass over the near walls the inside view removes, so it is offered with Inside only.
//
// Parity G44: Edges > None, upstream's View > Lines > None: no edge of the model is drawn.
//
// Item 7: Hide > Roof off (R): the faces that close the room from above left out of the view (hide.ts),
// a view state; the chip over the view says how many and which groups.
// Item 8: Hide > Isolate selection (I): only the picked faces or surface groups drawn until shown again.
import { useEffect, useRef, useState } from 'react';
import { useStore } from '../../store';
import { closingProps, usePresence } from '../../chrome/usePresence';
import { selectionStore } from '../../store';
import { hideStore, isolateWhyNot, setRoofOff, toggleIsolate, viewStyle, type EdgeStyle, type SurfaceStyle } from './engine';
import { FACE_SHOWS } from './faces';

const SURFACES: { key: SurfaceStyle; label: string; hint: string }[] = [
  { key: 'colour', label: 'Colour', hint: 'Each surface in its material colour, shaded by its angle' },
  { key: 'grey', label: 'Grey', hint: 'Shaded by angle, without the material colours' },
  { key: 'glass', label: 'See-through', hint: 'The near walls as glass, so the whole room shows from outside' },
  { key: 'wire', label: 'Wireframe', hint: 'Dark faces under the edges, the earlier look' },
];
const EDGES: { key: EdgeStyle; label: string; hint: string }[] = [
  { key: 'all', label: 'Every triangle', hint: 'Every edge of the mesh' },
  { key: 'feature', label: 'Features only', hint: 'Only where surfaces meet at 20 degrees or more' },
  { key: 'none', label: 'None', hint: "No edges: the surfaces alone, as upstream's View > Lines > None" },
];

export function ViewStyleMenu() {
  const style = useStore(viewStyle);
  const hidden = useStore(hideStore);
  useStore(selectionStore);
  const isolateWhy = hidden.isolate ? null : isolateWhyNot();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const listed = usePresence(open);

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
      {listed.shown && (
        <div className="view-style-menu float-panel" role="menu" data-part="view-style-menu" {...closingProps(listed.closing)}>
          <div className="view-style-head">Surfaces</div>
          {SURFACES.map((s) => {
            const off = s.key === 'glass' && style.faces !== 'inside';
            return (
              <button
                key={s.key}
                role="menuitemradio"
                aria-checked={style.surfaces === s.key}
                data-surfaces={s.key}
                disabled={off}
                title={off ? 'See-through puts glass over the near walls the Inside view removes: choose Faces, Inside first' : s.hint}
                onClick={() => set({ surfaces: s.key })}
              >
                {s.label}
              </button>
            );
          })}
          {style.surfaces === 'glass' && style.faces === 'inside' && (
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
          <div className="view-style-head">Faces</div>
          {FACE_SHOWS.map((f) => (
            <button
              key={f.key}
              role="menuitemradio"
              aria-checked={style.faces === f.key}
              data-faces={f.key}
              title={f.hint}
              onClick={() => set({ faces: f.key })}
            >
              {f.label}
            </button>
          ))}
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
          <button
            role="menuitemcheckbox"
            aria-checked={style.fade}
            data-fade=""
            title="The far side of the room sinks toward the background, so near and far separate"
            onClick={() => set({ fade: !style.fade })}
          >
            Distance fade
          </button>
          <button
            role="menuitemcheckbox"
            aria-checked={style.ground}
            data-ground=""
            title="A grey grid and a soft shadow under the room, so it sits on something"
            onClick={() => set({ ground: !style.ground })}
          >
            Floor grid
          </button>
          <div className="view-style-head">Show</div>
          <button
            role="menuitemcheckbox"
            aria-checked={style.dims}
            data-dims=""
            title="Length, width and height beside the room, as the Room model panel gives them"
            onClick={() => set({ dims: !style.dims })}
          >
            Dimensions
          </button>
          <div className="view-style-head">Hide</div>
          <button
            role="menuitemcheckbox"
            aria-checked={hidden.roof}
            data-roof-off=""
            title="Leave out the faces that close the room from above, so you can look in from above; the model is not changed"
            onClick={() => setRoofOff(!hidden.roof)}
          >
            Roof off<span className="view-style-key">R</span>
          </button>
          <button
            role="menuitemcheckbox"
            aria-checked={hidden.isolate !== null}
            data-isolate=""
            disabled={isolateWhy !== null}
            title={isolateWhy ?? (hidden.isolate ? 'Show everything again' : 'Show only the picked faces or surface groups, like isolate in a modelling tool; the model is not changed')}
            onClick={() => toggleIsolate()}
          >
            Isolate selection<span className="view-style-key">I</span>
          </button>
        </div>
      )}
    </div>
  );
}
