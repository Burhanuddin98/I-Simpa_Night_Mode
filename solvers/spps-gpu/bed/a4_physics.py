"""A4 bed, arm (c): the physics of the transmission and one-sided rules, from each run's point-receiver
echograms (`Punctual receivers/<rx>/[<source>/]Sound level.recp`, one float per time step and band,
read with recp_totals.read_gabe).

beam, beam-onesided (specular, unidirectional, no air absorption: every particle on one path, no RNG)
  Each pass of a particle of energy E through a receiver sphere adds E x chord (the same chord for every
  receiver: all sit on the beam axis). With a = 1 - alpha_panel, b = 1 - alpha_wall, tau = 10^(-TL/10):
  - beam: first arrival at B2a (room 2) over first arrival at B1b (room 1) = tau exactly (the first
    child carries E0 tau, CalculationCore.cpp energetic branch); second arrival at B1b over its first
    = a (the panel's reflection); B2a's second over its first = b (the end wall).
    Totals (infinite series, the family's FIFO children summed): room 1 parent passes B1b
    (1 + a)/(1 - ab), B1a a(1 + b)/(1 - ab); every unit entering room 2 (or room 1 from the panel)
    passes each receiver of that room (1 + b)/(1 - ab) times and hits the panel b/(1 - ab), so with
    g = tau b/(1 - ab): J2 = tau/((1 - ab)(1 - g^2)) enters room 2, J1 = g J2 re-enters room 1.
  - beam-onesided: Srefl (room 1, aimed +y at the panel's reflecting side) never reaches room 2: B2a
    and B2b exactly 0; B1b total over its first arrival (1 + a)/(1 - ab). Spass (room 2, aimed -y at the
    passing side) crosses once without loss: B2a one pass (E0), B2b 0, and room 1 traps it:
    B1b and B1a each (1 + b)/(1 - ab) E0.
trans, trans-a1, trans-random (diffuse, omni, air absorption on)
  Level difference across the panel, L1 - L2 = 10 lg(mean E over R1a-d / mean E over R2a-d), against the
  two-room steady-state energy balance (the relation behind ISO 10140-2 and ISO 16283-1,
  R = L1 - L2 + 10 lg(S/A2); Kuttruff, Room Acoustics, sound transmission between rooms):
  E2/E1 = tau S / A2, A2 = sum(alpha_i S_i) over room 2's walls + alpha_panel S + 4 m V2. An
  approximation: it assumes diffuse incidence on every surface of both rooms and ignores the direct
  sound (in R1's receivers, and onto the panel).
onesided (diffuse, omni)
  Srefl's energy at R2a-d must be exactly 0 (a particle meets the panel from room 1 with dir . n > 0, so
  it never passes; no transmission); Spass reaches room 1.

usage: python a4_physics.py <out.json> <out.md> <bed-log.jsonl> [...]
"""
import json, math, os, struct, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from recp_totals import read_gabe  # noqa: E402

S_PANEL = 18.0
ROOM_WALLS = 108.0           # room 2 (or room 1) without the panel: floor 30 + ceiling 30 + sides 30 + end 18
V_ROOM = 90.0
ALPHA_W = 0.2


def recp(run, rx, src=None):
    p = os.path.join(run, "solve", "Punctual receivers", rx, *( [src] if src else []), "Sound level.recp")
    return {label: v for label, v in read_gabe(p)}


def clusters(v):
    """Groups of consecutive non-zero steps: (first step, last step, sum)."""
    out, cur = [], None
    for i, x in enumerate(v):
        if x != 0:
            if cur is None:
                cur = [i, i, 0.0]
            cur[1] = i
            cur[2] += x
        elif cur is not None:
            out.append(tuple(cur))
            cur = None
    if cur is not None:
        out.append(tuple(cur))
    return out


def iso9613_m(f, T=293.15, rh=50.0, p=101325.0):
    """ISO 9613-1 attenuation (dB/m) converted to the energy coefficient m (1/m)."""
    pr, T0, T01 = 101325.0, 293.15, 273.16
    C = -6.8346 * (T01 / T) ** 1.261 + 4.6151
    h = rh * 10 ** C * (pr / p)
    frO = (p / pr) * (24 + 4.04e4 * h * (0.02 + h) / (0.391 + h))
    frN = (p / pr) * (T / T0) ** -0.5 * (9 + 280 * h * math.exp(-4.170 * ((T / T0) ** (-1 / 3) - 1)))
    a = 8.686 * f * f * ((1.84e-11 * (pr / p) * (T / T0) ** 0.5) + (T / T0) ** -2.5 * (
        0.01275 * math.exp(-2239.1 / T) / (frO + f * f / frO) + 0.1068 * math.exp(-3352.0 / T) / (frN + f * f / frN)))
    return a / (10 * math.log10(math.e))


def beam_expect(alpha_p, alpha_w, tl):
    a, b, tau = 1 - alpha_p, 1 - alpha_w, 10 ** (-tl / 10)
    q = 1 - a * b
    g = tau * b / q
    J2 = tau / (q * (1 - g * g))
    J1 = g * J2
    return {"tau": tau, "a": a, "b": b,
            "B1b": (1 + a) / q + J1 * (1 + b) / q, "B1a": a * (1 + b) / q + J1 * (1 + b) / q,
            "B2a": J2 * (1 + b) / q, "B2b": J2 * (1 + b) / q}


def beam_model(alpha_p, alpha_w, tl, starts, two_sided=True, transmit=True, eps_exp=7.0):
    """The beam's exact truncated reference: every particle of one family followed along the axis with
    SPPS's energetic rules (CalculationCore.cpp: wall E *= 1 - alpha; panel with 0 < alpha < 1: a
    child E tau when E tau > eps, then the parent E *= 1 - alpha and reflects; panel with alpha == 1:
    the parent itself passes with E *= tau; death when E <= eps = E0 10^-trans_epsilon; a single-sided
    panel is passed when the particle moves -y, against its +y normal), with alpha and tau rounded to
    float as SPPS stores them. Returns each receiver's passes as (path length from the source, E)
    (E0 = 1, chord 1) and the family's particle count (SPPS's statistics count every child)."""
    import numpy as np
    f32 = np.float32
    a = float(f32(1) - f32(alpha_p))
    b = float(f32(1) - f32(alpha_w))
    tau = float(f32(10.0 ** (-tl / 10.0))) if transmit else 0.0
    eps = 10.0 ** (-eps_exp)
    rx = {"B1a": 0.6, "B1b": 3.3, "B2a": 6.7, "B2b": 8.6}
    passes = {k: [] for k in rx}
    queue, n = [(y, d, E, 0.0) for y, d, E in starts], 0
    while queue:
        y, d, E, s = queue.pop(0)
        n += 1
        while True:
            if y == 5.0:
                end = 0.0 if d < 0 else 10.0
            else:
                end = (0.0 if y < 5 else 5.0) if d < 0 else (5.0 if y < 5 else 10.0)
            for k, ry in rx.items():
                if min(y, end) < ry < max(y, end):
                    passes[k].append((s + abs(ry - y), E))
            s += abs(end - y)
            y = end
            if y in (0.0, 10.0):
                E *= b
                if E <= eps:
                    break
                d = -d
                continue
            if two_sided or d > 0:
                if a == 0.0:
                    if not transmit:
                        break
                    E *= tau
                    if E <= eps:
                        break
                    continue           # passes, same direction
                if transmit and tau != 0 and E * tau > eps:
                    queue.append((5.0, d, E * tau, s))
                E *= a
                if E <= eps:
                    break
                d = -d
    return passes, n


def model_clusters(passes, radius=0.31, step=0.3432):
    """Passes grouped as the echogram groups them: a pass covers the steps of its chord; passes whose
    steps touch form one cluster. Returns [(first step, last step, E sum)] in time order."""
    spans = sorted((int((s - radius) // step), int((s + radius) // step), E) for s, E in passes)
    out = []
    for lo, hi, E in spans:
        if out and lo <= out[-1][1] + 1:
            out[-1] = (out[-1][0], max(out[-1][1], hi), out[-1][2] + E)
        else:
            out.append((lo, hi, E))
    return out


TL = {"500 Hz": 10.0, "1000 Hz": 20.0}


def analyse_beam(run, alpha_p):
    out = {}
    for band, tl in TL.items():
        passes, nfam = beam_model(alpha_p, ALPHA_W, tl, [(1.6, 1, 1.0)])
        rx = {r: recp(run, r)[band] for r in ("B1a", "B1b", "B2a", "B2b")}
        cl = {r: clusters(v) for r, v in rx.items()}
        mc = {r: model_clusters(p) for r, p in passes.items()}
        ref = cl["B1b"][0][2]     # first arrival at B1b = E0 x chord
        row = {"tau": 10 ** (-tl / 10), "model_particles_per_family": nfam}
        for r in rx:
            for i in range(3):
                if i < len(cl[r]) and i < len(mc[r]):
                    row[f"{r}_arrival{i + 1}_over_first_B1b"] = cl[r][i][2] / ref
                    row[f"{r}_arrival{i + 1}_model"] = mc[r][i][2]
                    row[f"{r}_arrival{i + 1}_steps_measured_model"] = f"{cl[r][i][0]}-{cl[r][i][1]} / {mc[r][i][0]}-{mc[r][i][1]}"
            m = sum(E for _, E in passes[r])
            row[f"total_{r}_over_first_B1b"] = sum(rx[r]) / ref
            row[f"model_{r}"] = m
            row[f"dev_model_{r}"] = sum(rx[r]) / ref / m - 1
            if alpha_p < 1:
                row[f"series_{r}"] = beam_expect(alpha_p, ALPHA_W, tl)[r]
        row["total_B2a_over_total_B1b"] = sum(rx["B2a"]) / sum(rx["B1b"])
        out[band] = row
    return out


def analyse_beam_onesided(run):
    out = {}
    a = b = 1 - ALPHA_W
    q = 1 - a * b
    for band in TL:
        row = {}
        s = {r: recp(run, r, "Srefl")[band] for r in ("B1a", "B1b", "B2a", "B2b")}
        p = {r: recp(run, r, "Spass")[band] for r in ("B1a", "B1b", "B2a", "B2b")}
        ref = clusters(s["B1b"])[0][2]
        row["Srefl_B2a_sum"], row["Srefl_B2b_sum"] = sum(s["B2a"]), sum(s["B2b"])
        row["Srefl_B2_nonzero_steps"] = sum(1 for x in s["B2a"] + s["B2b"] if x != 0)
        row["Srefl_total_B1b_over_first"], row["expect_(1+a)/(1-ab)"] = sum(s["B1b"]) / ref, (1 + a) / q
        row["Srefl_total_B1a_over_first_B1b"], row["expect_a(1+b)/(1-ab)"] = sum(s["B1a"]) / ref, a * (1 + b) / q
        pref = clusters(p["B2a"])[0][2]
        row["Spass_B2a_passes"] = len(clusters(p["B2a"]))
        row["Spass_B2a_total_over_first"] = sum(p["B2a"]) / pref
        row["Spass_B2b_sum"] = sum(p["B2b"])
        row["Spass_first_B1b_over_first_B2a"] = clusters(p["B1b"])[0][2] / pref
        row["Spass_total_B1b_over_first_B2a"] = sum(p["B1b"]) / pref
        row["Spass_total_B1a_over_first_B2a"] = sum(p["B1a"]) / pref
        row["expect_(1+b)/(1-ab)"] = (1 + b) / q
        ms, _ = beam_model(ALPHA_W, ALPHA_W, 0, [(1.6, 1, 1.0)], two_sided=False, transmit=False)
        mp, _ = beam_model(ALPHA_W, ALPHA_W, 0, [(8.0, -1, 1.0)], two_sided=False, transmit=False)
        row["model_Srefl"] = {r: sum(E for _, E in v) for r, v in ms.items()}
        row["model_Spass"] = {r: sum(E for _, E in v) for r, v in mp.items()}
        out[band] = row
    return out


def diffuse_expect(alpha_p, tl, f):
    tau = 10 ** (-tl / 10)
    m = iso9613_m(f)
    A2 = ROOM_WALLS * ALPHA_W + S_PANEL * alpha_p + 4 * m * V_ROOM
    return 10 * math.log10(A2 / (tau * S_PANEL)), A2, m


def analyse_trans(run, alpha_p):
    out = {}
    for band, tl in TL.items():
        f = float(band.split()[0])
        e1 = [sum(recp(run, r)[band]) for r in ("R1a", "R1b", "R1c", "R1d")]
        e2 = [sum(recp(run, r)[band]) for r in ("R2a", "R2b", "R2c", "R2d")]
        dl = 10 * math.log10((sum(e1) / 4) / (sum(e2) / 4))
        pred, A2, m = diffuse_expect(alpha_p, tl, f)
        out[band] = {"L1_minus_L2_dB": dl, "expect_dB": pred, "deviation_dB": dl - pred, "A2_m2": A2, "m_per_m": m,
                     "R1_energies": e1, "R2_energies": e2}
    return out


def analyse_onesided(run):
    out = {}
    for band in TL:
        row = {}
        for r in ("R2a", "R2b", "R2c", "R2d"):
            v = recp(run, r, "Srefl")[band]
            row[f"Srefl_{r}_sum"] = sum(v)
            row[f"Srefl_{r}_nonzero_steps"] = sum(1 for x in v if x != 0)
        for r in ("R1a", "R1b", "R1c", "R1d"):
            row[f"Spass_{r}_sum"] = sum(recp(run, r, "Spass")[band])
            row[f"Srefl_{r}_sum"] = sum(recp(run, r, "Srefl")[band])
        for r in ("R2a", "R2b", "R2c", "R2d"):
            row[f"Spass_{r}_sum"] = sum(recp(run, r, "Spass")[band])
        out[band] = row
    return out


def main(out_json, out_md, logs):
    runs = {}
    for log in logs:
        for line in open(log, encoding="utf-8"):
            r = json.loads(line)
            if r.get("run") and r.get("exit") == 0:
                runs[(r["case"], r["arm"], r["seed"])] = r
    res = {}
    for (case, arm, seed), r in sorted(runs.items()):
        run = r["run"]
        if case == "beam":
            a = analyse_beam(run, 0.3)
        elif case == "beam-a1":
            a = analyse_beam(run, 1.0)
        elif case == "beam-onesided":
            a = analyse_beam_onesided(run)
        elif case in ("trans", "trans-random"):
            a = analyse_trans(run, 0.3)
        elif case == "trans-a1":
            a = analyse_trans(run, 1.0)
        elif case in ("onesided", "onesided-random"):
            a = analyse_onesided(run)
        else:
            continue
        res[f"{case} {arm} s{seed}"] = {"run": run, "child_queue_overflow": r.get("child_queue_overflow"), "bands": a}
    # the beams draw no random number: SPPS and spps-gpu at the same seed, every .recp step compared bit for bit
    bits = {}
    for (case, arm, seed), r in sorted(runs.items()):
        if not case.startswith("beam") or arm != "gpu" or (case, "spps", seed) not in runs:
            continue
        g, s_ = r["run"], runs[(case, "spps", seed)]["run"]
        base = os.path.join(s_, "solve", "Punctual receivers")
        rows = []
        for rx in sorted(os.listdir(base)):
            for sub in [None] + sorted(x for x in os.listdir(os.path.join(base, rx)) if os.path.isdir(os.path.join(base, rx, x))):
                A, B = recp(s_, rx, sub), recp(g, rx, sub)
                for band in A:
                    a_, b_ = A[band], B[band]
                    same = sum(1 for x, y in zip(a_, b_) if struct.pack("<f", x) == struct.pack("<f", y))
                    first_diff = next((i for i, (x, y) in enumerate(zip(a_, b_)) if struct.pack("<f", x) != struct.pack("<f", y)), None)
                    rows.append({"receiver": rx + ("/" + sub if sub else ""), "band": band, "steps": len(a_), "bit_identical_steps": same,
                                 "first_differing_step": first_diff, "spps_total": sum(a_), "gpu_total": sum(b_),
                                 "gpu_over_spps_minus_1": (sum(b_) / sum(a_) - 1) if sum(a_) else None})
        bits[f"{case} s{seed}"] = rows
    res["_bits_spps_vs_gpu"] = bits
    with open(out_json, "w", encoding="utf-8") as fh:
        json.dump(res, fh, indent=1)
    md = []
    for k, v in res.items():
        if k.startswith("_"):
            for case, rows in v.items():
                md.append(f"### bits, spps vs gpu: {case}\n")
                for r in rows:
                    md.append("- " + ", ".join(f"{a}={b:.9g}" if isinstance(b, float) else f"{a}={b}" for a, b in r.items()))
                md.append("")
            continue
        md.append(f"### {k}\n\n`{v['run']}`, child_queue_overflow {v['child_queue_overflow']}\n")
        for band, row in v["bands"].items():
            md.append(f"- {band}: " + ", ".join(f"{a}={b:.9g}" if isinstance(b, float) else f"{a}={b}" for a, b in row.items() if not isinstance(b, (list, dict))))
        md.append("")
    with open(out_md, "w", encoding="utf-8") as fh:
        fh.write("\n".join(md) + "\n")
    print("\n".join(md))


if __name__ == "__main__":
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2], sys.argv[3:])
