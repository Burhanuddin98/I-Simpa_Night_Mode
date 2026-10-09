// The materials table in a window of its own (GUI audit 2026-10-09, A1; Burhan 02:32: "THE LITERAL WINDOW IN WHICH
// THEY EXIST IS SO SMALL THAT YOU HAVE TO SCROLL RIGHTWARDS IN LITTLE INCREMENTS AND FILL UP LITTLE BOXES THAT CAN
// BARELY BE SEEN"; his pick 02:40, "Wide sheet"). In the properties panel the table had 318 px at any window size;
// here it opens as wide as its bands need, up to the window, with larger cells. The grid itself is unchanged: the
// same component, keys, paste, copy and hooks. Moved by its title bar and resized by its corner, as the response
// window is; Esc or × closes it (an Esc the grid uses, to leave a selection or an edit, stays the grid's).
import { createPortal } from 'react-dom';
import { useFrame } from '../acoustics/ResponseWindow';
import { sceneStore, useStore } from '../../store';
import { MaterialsGrid } from './MaterialsGrid';

/** Column widths in the sheet (materials.css `.msheet`): name, one band, law; plus the window's padding and scrollbar. */
const NAME_W = 180;
const BAND_W = 48;
const LAW_W = 130;
const CHROME_W = 28 + 2 + 12;

export function MaterialsSheet({ onClose }: { onClose: () => void }) {
  const frame = useFrame();
  const bands = useStore(sceneStore)?.view.bands.frequencies_hz.length ?? 0;
  const width = Math.max(640, Math.min(window.innerWidth - 48, NAME_W + bands * BAND_W + LAW_W + CHROME_W));
  return createPortal(
    <div
      ref={frame.ref}
      className={`rw glass msheet${frame.pos ? ' rw-moved' : ''}`}
      style={frame.pos ? { width, left: frame.pos.x, top: frame.pos.y } : { width }}
      role="dialog"
      aria-label="Materials"
      data-materials-sheet=""
      onKeyDown={(e) => {
        if (e.key !== 'Escape') return;
        e.stopPropagation();
        onClose();
      }}
    >
      <div className="rw-head" onPointerDown={frame.onHeadDown} onPointerMove={frame.onHeadMove} onPointerUp={frame.onHeadUp} onPointerCancel={frame.onHeadUp}>
        <span className="rw-title">Materials</span>
        <span className="rw-sub">per band · {bands} bands</span>
        <button type="button" className="rw-close" data-action="close-materials" aria-label="Close" title="Close (Esc)" onClick={onClose}>
          ×
        </button>
      </div>
      <MaterialsGrid />
    </div>,
    document.body,
  );
}
