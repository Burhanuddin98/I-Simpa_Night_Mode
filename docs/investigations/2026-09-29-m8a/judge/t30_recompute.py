"""Independent T30 recomputation for the M8a bed (judge, T30 lens).

Written from ISO 3382-1 alone, without importing or copying the project's params code:
  - read each run's raw SPPS output, `solve/Punctual receivers/R00x/<receiversp_filename>`
    (a GABE v2 table: a short-string row-label column, then one float column per band, one row
    per time step; each value is proportional to the energy crossing the receiver in that step);
  - Schroeder backward integration: S(t_k) = sum_{j>=k} E_j, with t_k = k*dt the start edge of
    step k (the integral of a step-constant series is exact at the step edges);
  - L(t_k) = 10 lg(S(t_k)/S(0)); linear least-squares fit of L against t over every edge with
    -35 <= L <= -5 dB; T30 = -60/slope.
Two discretisation variants are computed as a sensitivity check only (step centres with the
half-step correction; and the fit over the curve linearly interpolated at the exact -5 and -35 dB
crossings, i.e. the continuous least-squares integral of the piecewise-linear curve).

Then checks A, B and C of SPEC section 5.1 are recomputed from these T30s, using report.json's
references (kuttruff_s, transport_t, transport_se) so that only the T30 differs; and Kuttruff is
recomputed independently as a side check.

Single process. Reads C:/tmp/nm-m8a-bed/20260929T093134Z (read-only). Writes one JSON beside
this script.

Usage: python t30_recompute.py [bed_stamp_dir]
"""

import hashlib
import json
import math
import os
import struct
import sys
import xml.etree.ElementTree as ET

import numpy as np
from scipy import stats

BED = sys.argv[1] if len(sys.argv) > 1 else r"C:/tmp/nm-m8a-bed/20260929T093134Z"
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "t30_recompute.json")

LIMIT_A, LIMIT_B, LIMIT_C = 0.05, 0.02, 0.005


# ----------------------------------------------------------------------------------------------
# GABE v2 reader (layout: 20-byte file header = 4 x int32 + readOnly byte + 3 pad; per column a
# 280-byte header = uint16 type, 2 pad, int32 rows, int64 size, int64 header size, 255-byte label,
# 1 pad; then float: int32 digits + rows x f32; int: 1 pad + rows x i32; short string: 1 pad +
# rows x 50 bytes). Checked below by requiring the parse to consume the file exactly.
# ----------------------------------------------------------------------------------------------
def read_gabe(path):
    b = open(path, "rb").read()
    ver, _, _, ncol = struct.unpack_from("<iiii", b, 0)
    if ver != 2:
        raise ValueError(f"{path}: gabe version {ver}")
    p = 20
    cols = []
    for _ in range(ncol):
        typ, = struct.unpack_from("<H", b, p)
        rows, = struct.unpack_from("<i", b, p + 4)
        label = b[p + 24:p + 24 + 255].split(b"\0", 1)[0]
        p += 280
        if typ == 50:
            p += 4
            vals = np.frombuffer(b, dtype="<f4", count=rows, offset=p).astype(np.float64)
            p += 4 * rows
        elif typ == 51:
            p += 1
            vals = np.frombuffer(b, dtype="<i4", count=rows, offset=p)
            p += 4 * rows
        elif typ == 52:
            p += 1
            vals = [b[p + 50 * i:p + 50 * (i + 1)].split(b"\0", 1)[0] for i in range(rows)]
            p += 50 * rows
        else:
            raise ValueError(f"{path}: column type {typ}")
        name = label.split(b"\r", 1)[0].decode("latin-1")
        cols.append((name, vals))
    if p != len(b):
        raise ValueError(f"{path}: parsed {p} bytes of {len(b)}")
    return cols


# ----------------------------------------------------------------------------------------------
# ISO 3382-1 T30
# ----------------------------------------------------------------------------------------------
def t30_edges(e, dt):
    """Primary: Schroeder at step edges, LS fit over -5..-35 dB."""
    e = np.asarray(e, dtype=np.float64)
    s = np.cumsum(e[::-1])[::-1]  # s[k] = sum_{j>=k} e_j
    tot = s[0]
    if not tot > 0:
        return None, {"why": "no energy"}
    with np.errstate(divide="ignore"):
        L = 10.0 * np.log10(s / tot)
    t = np.arange(len(e)) * dt
    m = (L <= -5.0) & (L >= -35.0)
    info = {"n_points": int(m.sum()), "min_level_db": float(np.min(L[np.isfinite(L)]))}
    if m.sum() < 3:
        return None, dict(info, why="range not reached")
    # the fit range must be contiguous: the Schroeder curve is monotone, so it is
    idx = np.nonzero(m)[0]
    if idx[-1] - idx[0] + 1 != len(idx):
        return None, dict(info, why="non-contiguous")
    if L[-1] > -35.0:
        return None, dict(info, why="range not reached")
    slope, icpt = np.polyfit(t[m], L[m], 1)
    return -60.0 / slope, info


def t30_centres(e, dt):
    """Variant: Schroeder at step centres, s[k] - e[k]/2."""
    e = np.asarray(e, dtype=np.float64)
    s = np.cumsum(e[::-1])[::-1] - 0.5 * e
    tot = e.sum()
    with np.errstate(divide="ignore"):
        L = 10.0 * np.log10(s / tot)
    t = (np.arange(len(e)) + 0.5) * dt
    m = (L <= -5.0) & (L >= -35.0)
    if m.sum() < 3 or L[-1] > -35.0:
        return None
    slope, _ = np.polyfit(t[m], L[m], 1)
    return -60.0 / slope


def t30_continuous(e, dt):
    """Variant: continuous LS fit of the piecewise-linear edge curve between the exact -5 and -35
    dB crossings (integrals of the linear pieces evaluated analytically)."""
    e = np.asarray(e, dtype=np.float64)
    s = np.concatenate([np.cumsum(e[::-1])[::-1], [0.0]])
    tot = s[0]
    with np.errstate(divide="ignore"):
        L = 10.0 * np.log10(s / tot)
    t = np.arange(len(s)) * dt
    if not (L[-2] < -35.0):
        return None

    def cross(level):
        k = np.nonzero(L <= level)[0][0]
        if k == 0:
            return 0.0
        l0, l1 = L[k - 1], L[k]
        return t[k - 1] + (level - l0) / (l1 - l0) * dt

    ta, tb = cross(-5.0), cross(-35.0)

    def lin(tt):
        return np.interp(tt, t[np.isfinite(L)], L[np.isfinite(L)])

    # piecewise-linear pieces within [ta, tb]
    knots = np.concatenate([[ta], t[(t > ta) & (t < tb)], [tb]])
    y = lin(knots)
    # integrals over each piece of 1, t, t^2, y, t*y with y linear
    S0 = S1 = S2 = Sy = Sty = 0.0
    for i in range(len(knots) - 1):
        a, b_ = knots[i], knots[i + 1]
        ya, yb = y[i], y[i + 1]
        h = b_ - a
        if h <= 0:
            continue
        k = (yb - ya) / h
        # y(t) = ya + k (t - a)
        S0 += h
        S1 += (b_ ** 2 - a ** 2) / 2
        S2 += (b_ ** 3 - a ** 3) / 3
        Sy += ya * h + k * h * h / 2
        # int t*(ya + k(t-a)) dt = (ya - k a) (b^2-a^2)/2 + k (b^3-a^3)/3
        Sty += (ya - k * a) * (b_ ** 2 - a ** 2) / 2 + k * (b_ ** 3 - a ** 3) / 3
    slope = (S0 * Sty - S1 * Sy) / (S0 * S2 - S1 * S1)
    return -60.0 / slope


def t30_tail(e, dt, t_est):
    """Variant: ISO 3382-1 truncation compensation. Add to every Schroeder value the energy an
    exponential tail would carry past the end of the series, from the mean step energy of the
    last 10 steps and the energy time constant T/(6 ln 10) of the primary estimate."""
    e = np.asarray(e, dtype=np.float64)
    tau = t_est / (6 * math.log(10))
    tail = e[-10:].mean() / dt * tau
    s = np.cumsum(e[::-1])[::-1] + tail
    L = 10.0 * np.log10(s / s[0])
    t = np.arange(len(e)) * dt
    m = (L <= -5.0) & (L >= -35.0)
    if m.sum() < 3:
        return None, tail / s[0]
    slope, _ = np.polyfit(t[m], L[m], 1)
    return -60.0 / slope, tail / s[0]


# ----------------------------------------------------------------------------------------------
# Independent Kuttruff reference (side check)
# ----------------------------------------------------------------------------------------------
def iso9613_m(f, T_c=20.0, hr=50.0, pa=101325.0):
    """ISO 9613-1 eqs (3)-(5) and annex B, energy attenuation m in 1/m at frequency f."""
    T = 273.15 + T_c
    T0, T01, pr = 293.15, 273.16, 101325.0
    C = -6.8346 * (T01 / T) ** 1.261 + 4.6151
    h = hr * (10 ** C) / (pa / pr)
    frO = (pa / pr) * (24 + 4.04e4 * h * (0.02 + h) / (0.391 + h))
    frN = (pa / pr) * (T / T0) ** -0.5 * (9 + 280 * h * math.exp(-4.170 * ((T / T0) ** (-1 / 3) - 1)))
    a = 8.686 * f * f * (1.84e-11 * (pa / pr) ** -1 * (T / T0) ** 0.5
                         + (T / T0) ** -2.5 * (0.01275 * math.exp(-2239.1 / T) / (frO + f * f / frO)
                                               + 0.1068 * math.exp(-3352.0 / T) / (frN + f * f / frN)))
    return a / (10 * math.log10(math.e))


def kuttruff(V, S, alpha, g2, m, c=343.2):
    K = 24 * math.log(10) / c
    ln = math.log(1 - alpha)
    A = -S * ln * (1 + 0.5 * g2 * ln)
    return K * V / (4 * m * V + A)


ROOM_DIMS = {"6x10x3": (6.0, 10.0, 3.0), "5x4x3": (5.0, 4.0, 3.0), "20x8x4": (20.0, 8.0, 4.0)}
EXACT_G2 = {"6x10x3": 0.388874, "5x4x3": 0.352401}

# SPEC.md section 2.1 and 2.3, transcribed from the SPEC (not from beds/m8a.json)
SPEC_ROOMS = {
    "6x10x3": ((3.0, 5.0, 1.8), [(1.0, 1.0, 1.8), (3.0, 7.0, 1.8), (5.0, 8.5, 1.2)]),
    "5x4x3": ((2.52, 1.97, 1.53), [(1.0, 1.0, 1.0), (4.0, 3.0, 2.0), (1.0, 3.0, 1.9)]),
    "20x8x4": ((4.02, 3.97, 1.53), [(2.0, 2.0, 1.2), (10.0, 6.0, 1.8), (18.0, 3.0, 2.5)]),
}
SPEC_RANDOM = {  # (room, alpha): (particles, duration)
    ("6x10x3", 0.05): (8_400_000, 4.0), ("6x10x3", 0.1): (18_000_000, 3.0),
    ("6x10x3", 0.2): (33_000_000, 2.0), ("6x10x3", 0.4): (110_000_000, 1.5),
    ("5x4x3", 0.05): (3_500_000, 3.0), ("5x4x3", 0.1): (7_900_000, 2.0),
    ("5x4x3", 0.2): (14_000_000, 1.5), ("5x4x3", 0.4): (31_000_000, 1.0),
    ("20x8x4", 0.05): (1_500_000, 5.7), ("20x8x4", 0.1): (1_500_000, 4.3),
    ("20x8x4", 0.2): (1_500_000, 2.9), ("20x8x4", 0.4): (1_500_000, 2.2),
}
SPEC_ENERGETIC = {  # (room, alpha): (particles, duration, trans_epsilon)
    ("6x10x3", 0.05): (1_500_000, 4.0, 7), ("6x10x3", 0.1): (1_500_000, 2.0, 7),
    ("6x10x3", 0.2): (1_500_000, 1.0, 9), ("6x10x3", 0.4): (1_500_000, 0.5, 9),
    ("5x4x3", 0.05): (1_500_000, 3.0, 7), ("5x4x3", 0.1): (1_500_000, 2.0, 7),
    ("5x4x3", 0.2): (1_500_000, 0.8, 9), ("5x4x3", 0.4): (1_500_000, 0.4, 9),
    ("20x8x4", 0.05): (1_500_000, 5.7, 7), ("20x8x4", 0.1): (1_500_000, 2.9, 7),
    ("20x8x4", 0.2): (1_500_000, 1.5, 9), ("20x8x4", 0.4): (1_500_000, 0.8, 9),
}
BANDS_OFF = [125, 250, 500, 1000, 2000, 4000]
BANDS_ON = BANDS_OFF + [8000]


# ----------------------------------------------------------------------------------------------
def tint(x):
    x = np.asarray(x, dtype=np.float64)
    n = len(x)
    mean = x.mean()
    sd = x.std(ddof=1)
    se = sd / math.sqrt(n)
    t = stats.t.ppf(0.975, n - 1)
    return dict(n=n, mean=mean, sd=sd, se=se, t=t, lo=mean - t * se, hi=mean + t * se)


def read_run(folder):
    solve = os.path.join(folder, "solve")
    cfg = ET.parse(os.path.join(solve, "config.xml")).getroot()
    sim = cfg.find("simulation")
    dt = float(sim.get("pasdetemps"))
    dur = float(sim.get("duree_simulation"))
    info = dict(
        dt=dt, duration=dur, particles=int(sim.get("nbparticules")),
        seed=int(sim.get("random_seed")), method=int(sim.get("computation_method")),
        air=int(sim.get("abs_atmo_calc")), trans_epsilon=float(sim.get("trans_epsilon")),
        radius=float(sim.get("rayon_recepteurp")),
        bands=[int(b.get("freq")) for b in sim.find("freq_enum") if b.get("docalc") == "1"],
        recp_name=sim.get("receiversp_filename"), recp_dir=sim.get("receiversp_directory"),
        receivers={r.get("lbl"): (float(r.get("x")), float(r.get("y")), float(r.get("z")))
                   for r in cfg.find("recepteursp")},
        sources=[(float(r.get("x")), float(r.get("y")), float(r.get("z")))
                 for r in cfg.find("sources")],
        absorb=sorted({float(b.get("absorb")) for b in cfg.iter("bfreq") if b.get("absorb")}),
        diffusion=sorted({float(b.get("diffusion")) for b in cfg.iter("bfreq") if b.get("diffusion")}),
        loi=sorted({b.get("loi") for b in cfg.iter("bfreq") if b.get("loi")}),
    )
    series = {}
    h = hashlib.sha256()
    info["row_label_first"] = []
    info["row_label_last"] = []
    info["gap_dt"] = []
    for lbl in sorted(info["receivers"]):
        path = os.path.join(solve, info["recp_dir"], lbl, info["recp_name"])
        h.update(open(path, "rb").read())
        cols = read_gabe(path)
        rows = len(cols[0][1])
        info["row_label_first"].append(cols[0][1][0].decode())
        info["row_label_last"].append(cols[0][1][-1].decode())
        gap = read_gabe(os.path.join(solve, info["recp_dir"], lbl, sim.get("receiversp_filename_adv")))
        info["gap_dt"].append(float(gap[1][1][0]))
        bands = {}
        for name, vals in cols[1:]:
            f = int(name.replace("Hz", "").strip())
            bands[f] = vals
        series[lbl] = (rows, bands)
    info["recp_sha256"] = h.hexdigest()
    return info, series


def check_run(cell, s, info, series, problems):
    """dt, seed, counts, duration, bands, receivers, walls, rows: against SPEC, not the bed file."""
    cid, room, a = cell["id"], cell["room"], cell["alpha"]
    want_bands = BANDS_ON if cell["air"] else BANDS_OFF
    if cell["method"] == "random":
        n, dur = SPEC_RANDOM[(room, a)]
        eps, meth = 7.0, 0
    else:
        n, dur, eps = SPEC_ENERGETIC[(room, a)]
        meth = 1
    src, rcv = SPEC_ROOMS[room]
    labels = sorted(series)

    def bad(what):
        problems.append(f"{cid} s{s}: {what}")

    if info["dt"] != 0.001:
        bad(f"pasdetemps {info['dt']}")
    if any(abs(g - 0.001) > 1e-9 for g in info["gap_dt"]):
        bad(f".gap time step {info['gap_dt']}")
    if any(lab != "1.0 ms" for lab in info["row_label_first"]):
        bad(f"first row label {info['row_label_first']}")
    if info["seed"] != s:
        bad(f"random_seed {info['seed']} in the folder of seed {s}")
    if info["particles"] != n:
        bad(f"nbparticules {info['particles']}, SPEC {n}")
    if abs(info["duration"] - dur) > 1e-9:
        bad(f"duration {info['duration']}, SPEC {dur}")
    if info["trans_epsilon"] != eps:
        bad(f"trans_epsilon {info['trans_epsilon']}, SPEC {eps}")
    if info["method"] != meth:
        bad(f"computation_method {info['method']}, want {meth}")
    if info["air"] != int(cell["air"]):
        bad(f"abs_atmo_calc {info['air']}")
    if info["bands"] != want_bands:
        bad(f"bands {info['bands']}")
    if labels != ["R000", "R001", "R002"]:
        bad(f"receivers {labels}")
    for i, lab in enumerate(labels):
        if max(abs(x - y) for x, y in zip(info["receivers"][lab], rcv[i])) > 1e-9:
            bad(f"{lab} at {info['receivers'][lab]}, SPEC {rcv[i]}")
        rows, bd = series[lab]
        if rows != round(dur / 0.001):
            bad(f"{lab}: {rows} rows for {dur} s")
        if sorted(bd) != want_bands:
            bad(f"{lab}: band columns {sorted(bd)}")
    if len(info["sources"]) != 1 or max(abs(x - y) for x, y in zip(info["sources"][0], src)) > 1e-9:
        bad(f"sources {info['sources']}")
    if info["absorb"] != [a]:
        bad(f"absorb {info['absorb']}")
    if info["diffusion"] != [1.0] or info["loi"] != ["2"]:
        bad(f"diffusion {info['diffusion']} loi {info['loi']}")


def npz_check(cell, runs_by_seed):
    """The bed's derived decays/<cell>.npz against the raw Schroeder curve (levels, u_s > 0) and
    the fit end points against report.json's T30."""
    path = os.path.join(BED, "decays", cell["id"] + ".npz")
    if not os.path.exists(path):
        return {"missing": True}
    z = np.load(path)
    src, rec, sd, fq, u, L = (z["source"], z["receiver"], z["seed"], z["freq_hz"], z["u_s"],
                              z["level_db"])
    max_db, max_fit = 0.0, 0.0
    where_db = where_fit = None
    room_src = SPEC_ROOMS[cell["room"]][0]
    for s in cell["seeds"]:
        info, series = runs_by_seed[s["seed"]]
        for ri, lab in enumerate(sorted(series)):
            arr = math.dist(room_src, info["receivers"][lab]) / 343.2
            for bi, f in enumerate(cell["bands_hz"]):
                e = series[lab][1][f]
                sc = np.cumsum(e[::-1])[::-1]
                with np.errstate(divide="ignore"):
                    Lm = 10 * np.log10(sc / sc[0])
                t = np.arange(len(e)) * info["dt"]
                m = (src == "spps") & (sd == s["seed"]) & (rec == ri) & (fq == f)
                uu, ll = u[m], L[m]
                with np.errstate(invalid="ignore"):
                    mm = np.interp(uu + arr, t, Lm)
                ok = np.isfinite(ll) & np.isfinite(mm) & (uu > 0)
                if ok.any():
                    d = float(np.max(np.abs(mm[ok] - ll[ok])))
                    if d > max_db:
                        max_db, where_db = d, (s["seed"], lab, f)
                mf = (src == "spps_fit") & (sd == s["seed"]) & (rec == ri) & (fq == f)
                rep = s["t30"][ri][bi]["t"]
                if mf.sum() == 2 and rep:
                    ends = u[mf][np.argsort(L[mf])[::-1]]  # -5 then -35
                    tf = 2.0 * (ends[1] - ends[0])
                    d = abs(tf / rep - 1)
                    if d > max_fit:
                        max_fit, where_fit = d, (s["seed"], lab, f)
    return {"max_level_db_diff": max_db, "where_level": where_db,
            "max_fit_t30_rel_diff": max_fit, "where_fit": where_fit}


def main():
    report = json.load(open(os.path.join(BED, "report.json")))
    out = {"bed": BED, "cells": []}
    problems = []
    for cell in report["cells"]:
        cid = cell["id"]
        bands = cell["bands_hz"]
        ref = {b["freq_hz"]: b for b in cell["reference"]["bands"]}
        nb = len(bands)
        seeds = [s["seed"] for s in cell["seeds"]]
        if seeds != list(range(1, 11)):
            problems.append(f"{cid}: seeds {seeds}")
        mine, var_c, var_i, var_t = {}, {}, {}, {}
        runinfo, per_rb_diff, refusals = [], [], []
        runs_by_seed = {}
        tail_max = 0.0
        for s in cell["seeds"]:
            folder = s["run"]["folder"]
            info, series = read_run(folder)
            runs_by_seed[s["seed"]] = (info, series)
            check_run(cell, s["seed"], info, series, problems)
            labels = sorted(series)
            runinfo.append(dict(seed=s["seed"], folder=folder, dt=info["dt"], gap_dt=info["gap_dt"],
                                duration=info["duration"], particles=info["particles"],
                                cfg_seed=info["seed"], method=info["method"], air=info["air"],
                                trans_epsilon=info["trans_epsilon"], bands=info["bands"],
                                receivers=labels, rows=[series[lab][0] for lab in labels],
                                recp_sha256=info["recp_sha256"]))
            T = np.full((len(labels), nb), np.nan)
            Tc, Ti, Tt = T.copy(), T.copy(), T.copy()
            for ri, lab in enumerate(labels):
                rows, bd = series[lab]
                for bi, f in enumerate(bands):
                    v, inf = t30_edges(bd[f], info["dt"])
                    if v is None:
                        refusals.append(dict(seed=s["seed"], receiver=lab, freq_hz=f, **inf))
                    else:
                        T[ri, bi] = v
                        Tc[ri, bi] = t30_centres(bd[f], info["dt"]) or np.nan
                        Ti[ri, bi] = t30_continuous(bd[f], info["dt"]) or np.nan
                        tv, tail = t30_tail(bd[f], info["dt"], v)
                        Tt[ri, bi] = tv if tv else np.nan
                        tail_max = max(tail_max, tail)
                    rep = s["t30"][ri][bi] if s.get("t30") else None
                    rv = rep.get("t") if isinstance(rep, dict) else None
                    per_rb_diff.append(dict(
                        seed=s["seed"], receiver=lab, freq_hz=f, mine=None if v is None else v,
                        centres=None if v is None else Tc[ri, bi],
                        continuous=None if v is None else Ti[ri, bi],
                        tail=None if v is None else Tt[ri, bi],
                        report=rv, report_source=rep.get("source") if isinstance(rep, dict) else rep,
                        rel=None if (v is None or rv is None) else v / rv - 1,
                        n_points=inf.get("n_points")))
            mine[s["seed"]], var_c[s["seed"]], var_i[s["seed"]], var_t[s["seed"]] = T, Tc, Ti, Tt
        hashes = [r["recp_sha256"] for r in runinfo]
        if len(set(hashes)) != len(hashes):
            problems.append(f"{cid}: two seeds wrote identical .recp files")

        # ---- checks
        TK = np.array([ref[f]["kuttruff_s"] for f in bands])
        Ttr = np.array([ref[f]["transport_t"] if ref[f]["transport_t"] is not None else np.nan
                        for f in bands])
        setr = np.array([(ref[f]["transport_se"] / ref[f]["transport_t"])
                         if ref[f]["transport_t"] else np.nan for f in bands])
        # independent Kuttruff (box V, S; exact gamma^2 for the two gated boxes; ISO 9613-1 m)
        dims = ROOM_DIMS[cell["room"]]
        V = dims[0] * dims[1] * dims[2]
        S = 2 * (dims[0] * dims[1] + dims[0] * dims[2] + dims[1] * dims[2])
        g2 = cell["reference"]["gamma2"]
        kk = []
        for f in bands:
            m = iso9613_m(f) if cell["air"] else 0.0
            tk = kuttruff(V, S, cell["alpha"], g2, m)
            tk_exact = kuttruff(V, S, cell["alpha"], EXACT_G2.get(cell["room"], g2), m)
            kk.append(dict(freq_hz=f, mine=tk, mine_exact_g2=tk_exact, report=ref[f]["kuttruff_s"],
                           rel=tk / ref[f]["kuttruff_s"] - 1,
                           rel_exact_g2=tk_exact / ref[f]["kuttruff_s"] - 1,
                           m_mine=m, m_report=ref[f]["air_m_per_metre"]))
        TK_own = np.array([k["mine_exact_g2"] for k in kk])

        allT = np.stack([mine[s] for s in seeds])  # seed, r, b
        judged = not np.isnan(allT).any()
        res = dict(id=cid, gated=cell["gated"], method=cell["method"], air=cell["air"],
                   alpha=cell["alpha"], room=cell["room"], seeds=seeds, n_seeds=len(seeds),
                   n_receivers=allT.shape[1], n_bands=allT.shape[2],
                   runs=runinfo, refusals_mine=refusals, per_rb=per_rb_diff, judged_mine=judged,
                   tail_share_max=tail_max)

        def checks(allT, TK):
            d = {}
            x = allT.mean(axis=1) / TK - 1  # seed, band
            d["a_bands"] = [dict(freq_hz=f, **tint(x[:, bi])) for bi, f in enumerate(bands)]
            d["a_worst_edge"] = max(max(abs(a["lo"]), abs(a["hi"])) for a in d["a_bands"])
            d["a_pass"] = d["a_worst_edge"] <= LIMIT_A
            ms = (allT / TK).mean(axis=(1, 2))
            d["b_cell_means"] = ms.tolist()
            d["b_spread"] = float((ms.max() - ms.min()) / ms.mean())
            d["b_pass"] = d["b_spread"] <= LIMIT_B
            y = (allT / Ttr).mean(axis=(1, 2)) - 1
            d["c_per_seed"] = y.tolist()
            ci = tint(y)
            se_tr = float(np.nanmean(setr))
            SE = math.sqrt(ci["sd"] ** 2 / len(y) + se_tr ** 2)
            d["c_d"] = ci["mean"]
            d["c_se"] = SE
            d["c_se_tr"] = se_tr
            d["c_stat"] = abs(ci["mean"]) + ci["t"] * SE
            d["c_lo"] = ci["mean"] - ci["t"] * SE
            d["c_hi"] = ci["mean"] + ci["t"] * SE
            d["c_pass"] = d["c_stat"] <= LIMIT_C
            return d

        if judged:
            res["mine"] = checks(allT, TK)
            res["mine_own_kuttruff"] = checks(allT, TK_own)
            res["centres"] = checks(np.stack([var_c[s] for s in seeds]), TK)
            res["continuous"] = checks(np.stack([var_i[s] for s in seeds]), TK)
            vt = np.stack([var_t[s] for s in seeds])
            if not np.isnan(vt).any():
                res["tail"] = checks(vt, TK)

        rep = {}
        if cell.get("a"):
            rep["a_bands"] = cell["a"]["bands"]
            rep["a_verdict"] = cell["a"]["verdict"]
        if cell.get("b"):
            rep["b_cell_means"] = cell["b"]["cell_means"]
            rep["b_spread"] = cell["b"]["spread"]
            rep["b_verdict"] = cell["b"]["verdict"]
        if cell.get("c"):
            rep["c_per_seed"] = cell["c"]["per_seed"]
            rep["c_interval"] = cell["c"]["interval"]
            rep["c_se_transport"] = cell["c"]["se_transport"]
            rep["c_verdict"] = cell["c"]["verdict"]
        res["report"] = rep
        res["verdict_report"] = cell.get("verdict")
        res["kuttruff_check"] = dict(V=V, S=S, report_V=cell["reference"]["volume_m3"],
                                     report_S=cell["reference"]["area_m2"], bands=kk)
        res["npz"] = npz_check(cell, runs_by_seed)
        out["cells"].append(res)
        print(cid, "judged" if judged else f"NOT judged ({len(refusals)} refusals)", flush=True)

    out["problems"] = problems
    out["summary"] = summarise(out)
    with open(OUT, "w") as fh:
        json.dump(out, fh, indent=0, default=float)
    print("problems:", len(problems))
    for p in problems[:50]:
        print("  ", p)
    print(json.dumps(out["summary"]["maxima"], indent=1, default=float))
    print("wrote", OUT)


def summarise(out):
    """Per cell: my A worst edge, band means, B, C against report.json's; global maxima."""
    rows = []
    g = dict(max_rel_value=(0, None), max_abs_mean_rel_value=(0, None), max_dA=(0, None),
             max_dB=(0, None), max_dC_seed=(0, None), max_dCstat=(0, None),
             max_rel_value_cont=(0, None), verdict_flips=[])
    for c in out["cells"]:
        r = c["report"]
        rel = [(abs(p["rel"]), p) for p in c["per_rb"] if p["rel"] is not None]
        relc = [(abs(p["continuous"] / p["report"] - 1), p) for p in c["per_rb"]
                if p["rel"] is not None]
        mean_rel = float(np.mean([p["rel"] for _, p in rel])) if rel else None
        if c["gated"]:
            mx = max(rel, key=lambda x: x[0])
            if mx[0] > g["max_rel_value"][0]:
                g["max_rel_value"] = (mx[0], dict(cell=c["id"], seed=mx[1]["seed"],
                                                  receiver=mx[1]["receiver"],
                                                  freq_hz=mx[1]["freq_hz"], mine=mx[1]["mine"],
                                                  report=mx[1]["report"]))
            mc = max(relc, key=lambda x: x[0])
            if mc[0] > g["max_rel_value_cont"][0]:
                g["max_rel_value_cont"] = (mc[0], c["id"])
            if abs(mean_rel) > g["max_abs_mean_rel_value"][0]:
                g["max_abs_mean_rel_value"] = (abs(mean_rel), c["id"])
        row = dict(id=c["id"], gated=c["gated"], n=len(rel), mean_rel=mean_rel,
                   max_abs_rel=max(rel, key=lambda x: x[0])[0] if rel else None)
        if "mine" in c and "a_bands" in r:
            m = c["mine"]
            rep_edge = max(max(abs(b["interval"]["lo"]), abs(b["interval"]["hi"])) for b in r["a_bands"])
            dA = max(max(abs(a["lo"] - b["interval"]["lo"]), abs(a["hi"] - b["interval"]["hi"]),
                         abs(a["mean"] - b["interval"]["mean"]))
                     for a, b in zip(m["a_bands"], r["a_bands"]))
            dB = abs(m["b_spread"] - r["b_spread"])
            dCs = float(np.max(np.abs(np.array(m["c_per_seed"]) - np.array(r["c_per_seed"]))))
            ci = r["c_interval"]
            rep_cst = abs(ci["mean"]) + ci["t"] * ci["se"]
            dCst = abs(m["c_stat"] - rep_cst)
            row.update(a_band_means=[a["mean"] for a in m["a_bands"]],
                       a_band_means_report=[b["interval"]["mean"] for b in r["a_bands"]],
                       a_edge=m["a_worst_edge"], a_edge_report=rep_edge, dA=dA,
                       b=m["b_spread"], b_report=r["b_spread"], dB=dB,
                       c_d=m["c_d"], c_d_report=ci["mean"], c_lo=m["c_lo"], c_hi=m["c_hi"],
                       c_stat=m["c_stat"], c_stat_report=rep_cst, dC_seed=dCs, dC_stat=dCst,
                       a_own_kuttruff_edge=c["mine_own_kuttruff"]["a_worst_edge"],
                       b_own_kuttruff=c["mine_own_kuttruff"]["b_spread"],
                       a_edge_tail=c.get("tail", {}).get("a_worst_edge"),
                       c_stat_tail=c.get("tail", {}).get("c_stat"),
                       tail_share_max=c["tail_share_max"],
                       a_edge_cont=c["continuous"]["a_worst_edge"],
                       b_cont=c["continuous"]["b_spread"], c_stat_cont=c["continuous"]["c_stat"],
                       npz=c["npz"])
            if c["gated"]:
                for key, val in (("max_dA", dA), ("max_dB", dB), ("max_dC_seed", dCs),
                                 ("max_dCstat", dCst)):
                    if val > g[key][0]:
                        g[key] = (val, c["id"])
                for chk, mp in (("a", m["a_pass"]), ("b", m["b_pass"]), ("c", m["c_pass"])):
                    rv = r.get(chk + "_verdict")
                    if (rv == "pass") != mp:
                        g["verdict_flips"].append((c["id"], chk, rv, mp))
        elif "mine" in c:
            m = c["mine"]
            row.update(a_edge=m["a_worst_edge"], a_band_means=[a["mean"] for a in m["a_bands"]],
                       b=m["b_spread"], c_d=m["c_d"], c_lo=m["c_lo"], c_hi=m["c_hi"],
                       c_stat=m["c_stat"], refused_in_report=sum(
                           1 for p in c["per_rb"] if p["report"] is None), npz=c["npz"])
        rows.append(row)
    return dict(rows=rows, maxima=g)


if __name__ == "__main__":
    main()
