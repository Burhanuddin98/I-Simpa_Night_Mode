"""PREREG-5 arms S, H, Hp: a copy of the SPPS run folder whose receiver echograms carry the ISM early part,
evaluated by Night Mode's own parameter code (`simpa results --json`).

  python splice.py <ROOM> <results json> <arm: S|H|Hp>

S  = the copy untouched (gate 1: its results must equal the original's).
H  = bins before t_tr replaced by the ISM's specular arrivals, prod (1-alpha)(1-s) per reflection.
Hp = the same with prod (1-alpha) only.
Per bin: energy_pa2 = rho c W / (4 pi L^2) exp(-m L) prod(...), rho c = 413.25 (level.rs, and backed out of the
run's own G). Writes runs/<ROOM>_<arm>.json and, for H/Hp, gate3_<ROOM>_<arm>.txt.
"""
import json, os, shutil, struct, subprocess, sys
from pathlib import Path

import numpy as np

ROOM, RES, ARM = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
HERE = Path(__file__).parent
REPO = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD')
SIMPA = REPO / 'target' / 'release' / 'simpa.exe'
RHO_C = 413.25
d = json.loads(RES.read_text())
s = d['spps']
src_run = Path(d['run_folder'].rstrip('\\'))
dst_root = HERE / 'runs' / f'{ROOM}_{ARM}'
dst = dst_root / src_run.name
if dst_root.exists():
    shutil.rmtree(dst_root)          # our own scratch copy from a previous call of this script


def place(a, b):
    """Particle files (hundreds of MB, never written here) are hard-linked; everything else is a real copy, because
    the receiver files are rewritten in place and a link would rewrite the original run."""
    if 'Particles' in Path(a).parts:
        os.link(a, b)
    else:
        shutil.copy2(a, b)


shutil.copytree(src_run, dst, copy_function=place)


def gabe_columns(b, raw=False):
    """[(label, float32 offset, rows)] of the float columns of a .gabe/.recp file (layout as
    recp_totals.read_gabe); raw=True keeps every column in file order, None for a non-float one."""
    cols, = struct.unpack_from('<i', b, 12)
    o, out = 20, []
    for _ in range(cols):
        typ, = struct.unpack_from('<H', b, o)
        rows, = struct.unpack_from('<i', b, o + 4)
        label = b[o + 24:o + 279].split(b'\0')[0].decode('latin-1')
        o += 280
        if typ == 50:
            out.append((label, o + 4, rows)); o += 4 + 4 * rows
        elif typ == 51:
            out.append(None); o += 1 + 4 * rows
        else:
            out.append(None); o += 1 + 50 * rows
    return out if raw else [x for x in out if x is not None]


if ARM != 'S':
    p = json.loads((REPO / 'app/src-tauri/examples' / f'bras_{ROOM.lower()}.simpa').read_text(encoding='utf-8'))
    mat_of_group = {g['id']: g['material'] for g in p['surface_groups']}
    mats = {m['id']: m for m in p['materials']}          # base materials: a variant is never read here
    bands = p['bands']['frequencies_hz']
    z = np.load(HERE / f'arrivals_{ROOM}.npz')
    pg = z['plane_group']; t_tr = float(z['t_tr'])
    c, dt = s['speed_of_sound_m_s'], s['time_step_s']
    n_tr = int(np.floor(t_tr / dt + 1e-9))
    air = {int(b['freq_hz']): b['air_m_per_metre'] for b in s['reference']['bands']}
    W = {x['name']: x['band_power_w'] for x in s['sources']}
    dist = {(x['name'], r['label']): float(np.linalg.norm(np.array(x['position_m']) - np.array(r['position_m'])))
            for x in s['sources'] for r in s['point_receivers']}

    def factor(planes, bi):
        f = 1.0
        for q in planes:
            m = mats[mat_of_group[pg[q]]]
            a = m['absorption'][bi]; sc = m['scattering'][bi]
            f *= (1 - a) * ((1 - sc) if ARM == 'H' else 1.0)
        return f

    log = [f'{ROOM} {ARM}: t_tr {1e3 * t_tr:.1f} ms ({n_tr} bins), orders exhaustive <= {int(z["order_exh"])}, '
           f'ray-guided <= {int(z["order_ray"])} ({int(z["n_rays"])} rays)',
           'pair band | direct ISM/SPPS dB | early (t<t_tr) ISM/SPPS dB | SPPS early mc dB']
    agg, totals = {}, {}
    for r in s['point_receivers']:
        for ps in r['per_source']:
            sel = (z['src'] == ps['source']) & (z['rcv'] == r['label'])
            L = z['length'][sel]; order = z['order'][sel]
            planes = [json.loads(x) for x in z['planes'][sel]]
            f = dst / 'solve' / ps['file']
            b = bytearray(f.read_bytes())
            cols = gabe_columns(b)
            for bi, fc in enumerate(bands):
                lab, off, rows = cols[bi]
                assert lab.startswith(str(fc)), (lab, fc)
                e = np.array(struct.unpack_from(f'<{rows}f', b, off), float)
                ism = np.zeros(rows)
                Wb = W[ps['source']][bi]
                for Li, pl in zip(L, planes):
                    k = int(np.floor(Li / c / dt))
                    if k < rows:
                        ism[k] += RHO_C * Wb / (4 * np.pi * Li ** 2) * np.exp(-air[fc] * Li) * factor(pl, bi)
                # window: from emission to t_tr after the direct arrival (PREREG-5 amendment 22:55)
                k0 = int(np.floor(dist[ps['source'], r['label']] / c / dt))
                n_hi = k0 + n_tr
                new = e.copy(); new[:n_hi] = ism[:n_hi]
                new32 = new.astype(np.float32)
                struct.pack_into(f'<{rows}f', b, off, *new32)
                acc = agg.setdefault((r['label'], bi), [np.zeros(rows), np.zeros(rows, bool)])
                acc[0] += new32.astype(float); acc[1][:n_hi] = True
                totals[(r['label'], ps['source'], bi)] = float(new32.astype(float).sum())
                # gate 3 numbers: direct (a 5-bin window round the direct bin: SPPS's receiver sphere spreads it)
                w0, w1 = max(0, k0 - 2), k0 + 3
                band = next(x for x in ps['bands'] if int(x['freq_hz']) == fc)
                mc = (band['parameters'].get('c80_db') or {}).get('mc_sd') or 0.0
                log.append(f'{ps["source"]}-{r["label"]} {fc} | {10 * np.log10(max(ism[w0:w1].sum(), 1e-300) / e[w0:w1].sum()):+.2f} '
                           f'| {10 * np.log10(ism[:n_hi].sum() / e[:n_hi].sum()):+.2f} | {mc:.3f}'
                           f' | direct {"open" if (order == 0).any() else "BLOCKED"}')
            f.write_bytes(bytes(b))
    # the receiver's all-source echogram (.recp) and its .gap twin move by the same per-source changes; the .gap's
    # energy column must equal the .recp's bit for bit, its two lateral columns (E cos^2, E |cos|) scale with it
    for rl in {k[0] for k in agg}:
        fr = dst / 'solve' / 'Punctual receivers' / rl / 'Sound level.recp'
        fg = fr.with_name('Advanced sound level.gap')
        br = bytearray(fr.read_bytes()); cr = gabe_columns(br)
        bg = bytearray(fg.read_bytes()) if fg.exists() else None; cg = gabe_columns(bg, raw=True) if bg is not None else None
        for bi in range(len(bands)):
            lab, off, rows = cr[bi]
            e = np.array(struct.unpack_from(f'<{rows}f', br, off), np.float32)
            # a bin inside any source's window is the sum of the sources' new values; the rest stay bit-identical
            sum_new, touched = agg[(rl, bi)][0][:rows], agg[(rl, bi)][1][:rows]
            new32 = e.copy(); new32[touched] = sum_new[touched].astype(np.float32)
            struct.pack_into(f'<{rows}f', br, off, *new32)
            if bg is not None:
                _, ge, gr = cg[5 + 3 * bi]
                ge_old = np.array(struct.unpack_from(f'<{gr}f', bg, ge), np.float32)
                assert np.array_equal(ge_old, e), f'{rl} band {bi}: .gap energy column is not the .recp column'
                struct.pack_into(f'<{gr}f', bg, ge, *new32)
                ratio = np.where(e > 0, new32.astype(float) / np.where(e > 0, e, 1), 0.0)
                for j in (6, 7):
                    _, go, _ = cg[j + 3 * bi]
                    lat = np.array(struct.unpack_from(f'<{gr}f', bg, go), float)
                    lat = np.where(np.isnan(lat), lat, lat * ratio)
                    struct.pack_into(f'<{gr}f', bg, go, *lat.astype(np.float32))
        fr.write_bytes(bytes(br))
        if bg is not None:
            fg.write_bytes(bytes(bg))
        # each source's band total (.recps): the sum of that source's new echogram
        fs = fr.with_name('Sound level per source.recps')
        if fs.exists():
            bs = bytearray(fs.read_bytes())
            n, = struct.unpack_from('<i', bs, 12); o = 20; names = []; fcols = []
            for _ in range(n):
                typ, = struct.unpack_from('<H', bs, o); rows, = struct.unpack_from('<i', bs, o + 4); o += 280
                if typ == 52:
                    names = [bs[o + 1 + 50 * k:o + 51 + 50 * k].split(bytes(1))[0].decode('latin-1') for k in range(rows)]
                    o += 1 + 50 * rows
                elif typ == 50:
                    fcols.append(o + 4); o += 4 + 4 * rows
                else:
                    o += 1 + 4 * rows
            for bi, off in enumerate(fcols):
                vals = [totals[(rl, nm, bi)] for nm in names]
                struct.pack_into(f'<{len(vals)}f', bs, off, *np.array(vals, np.float32))
            fs.write_bytes(bytes(bs))
    (HERE / f'gate3_{ROOM}_{ARM}.txt').write_text('\n'.join(log) + '\n')

out = HERE / 'runs' / f'{ROOM}_{ARM}.json'
r = subprocess.run([str(SIMPA), 'results', str(dst), '--json'], capture_output=True, text=True)
out.write_text(r.stdout)
print(f'{ROOM} {ARM}: simpa results exit {r.returncode}, {len(r.stdout)} bytes -> {out}')
if r.stderr.strip():
    print('stderr:', r.stderr.strip()[:2000])
