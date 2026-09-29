// The Geometry step's properties (design:280-310): the room model and its check as text rows
// (closed, self-intersections, flipped normals, open edges, units), the dimensions, volume and
// surface (geometry facts, inside `[data-geometry]`), and Import model…, which opens the same
// dialog as File › Open… on a mesh (unit and up axis confirmed, never guessed).
import * as actions from '../actions';
import { sceneStore, useStore } from '../store';
import { checkRows, fact, fileLabel, unitsText } from './sceneModel';

function ImportBlock() {
  return (
    <div className="props-section">
      <button className="wide-button" data-part="import-model" onClick={() => actions.fire(actions.openDialog())}>
        Import model…
      </button>
      <div className="formats">PLY · OBJ · STL</div>
    </div>
  );
}

export function GeometryPanel() {
  const scene = useStore(sceneStore);
  const check = scene?.check ?? null;
  if (!scene || !check) {
    return (
      <div data-part="geometry-panel">
        <div className="props-head">
          <div className="title">Room model</div>
          <div className="sub">{scene ? 'No model in this project' : 'No project open'}</div>
        </div>
        <div className="props-section empty">
          Import a room model, or open a project, to see its model check here.
        </div>
        <ImportBlock />
      </div>
    );
  }
  const info = scene.info;
  const ok = check.verdict === 'ok';
  const imported = !info.path;
  const [lx, ly, lz] = check.extents_m;
  return (
    <div data-part="geometry-panel">
      <div className="props-head">
        <div className="title">Room model</div>
        <div className="sub">
          {fileLabel(info)} · {imported ? 'imported, ' : ''}
          {ok ? 'checked' : 'refused'}
        </div>
      </div>

      <div className="props-section">
        <div className="section-head">
          <span className="label">Model check</span>
          <span className={`verdict ${ok ? 'ok' : 'fail'}`} data-part="check-verdict">
            {ok ? 'Passed' : 'FAIL · refused'}
          </span>
        </div>
        <div className="check-list" data-geometry>
          {checkRows(check, unitsText(info)).map((r) => (
            <div key={r.key} className="check-row" data-check={r.key}>
              <span className={`state ${r.state === 'FAIL' ? 'fail' : r.state === 'OK' ? 'ok' : 'none'}`}>
                {r.state ?? ''}
              </span>
              <span className="k">{r.label}</span>
              <span className="v">{r.value}</span>
            </div>
          ))}
        </div>
        {!ok && (
          <div className="hint">
            Run is refused until the model passes. The faces involved are highlighted in the view; the
            Console has each reason in full.
          </div>
        )}
      </div>

      <div className="props-section" data-geometry>
        <div className="label section-label">Dimensions</div>
        <div className="fact-grid">
          {(
            [
              ['Length', lx],
              ['Width', ly],
              ['Height', lz],
            ] as const
          ).map(([k, v]) => (
            <div key={k} className="fact-cell">
              <div className="k">{k}</div>
              <div className="v mono">{fact(v, 2)} m</div>
            </div>
          ))}
        </div>
        <div className="fact-line">
          <span>
            Volume <span className="mono v">{fact(check.enclosed_volume_m3, 1)} m³</span>
          </span>
          <span>
            Surface <span className="mono v">{fact(check.area_m2, 1)} m²</span>
          </span>
        </div>
        <div className="fact-line">
          <span>
            Faces <span className="mono v">{check.counts.faces}</span>
          </span>
          <span>
            Surface groups <span className="mono v">{info.surface_groups}</span>
          </span>
        </div>
      </div>

      <ImportBlock />
    </div>
  );
}
