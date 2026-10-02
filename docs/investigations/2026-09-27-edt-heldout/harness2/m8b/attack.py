"""The attack kit, round 2 (HARNESS-PLAN-2.md section 5; round 1's HARNESS-PLAN.md P25, P26; PREREG-2.md H6): the sandbox, the
class schema, the draws, the judges' prompt and the panel rule.

Round 2 differs from round 1 in these places only: the sandbox holds frozen2/method.py (checked against
corpus.FROZEN2_SHA256, VoidRun on a mismatch) in C:\\tmp\\m8b-edt\\attack2; build_sandbox refuses until the sentinel seat's
SENTINEL.md exists (driver.Refused 'attack_before_sentinel'); INTERFACE.md lists the statuses and every refusal reason the
file can give, read from the file, and says a refusal is never wrong-silent; and the draws are seeded by the class AND the
attack seed 2026100203, so they are not round 1's draws of the same class (section 9 m5). physics_md, judge_prompt,
validate_class, panel and generators_py are round 1's text, and B1 hashes this file.

Built in HARNESS-PLAN.md section 6, step 6 (the attack-kit step): validate_class, class_sha256,
draws, physics_md, build_sandbox and judge_prompt. parse_vote and panel were built earlier, with
the scorer, because T20 calls them and score.h6 rests on them; they are unchanged here. Tests T17
and T20 (the panel, through score.h6) hold the contract below.

Contract:
- A class is JSON (P26): {"generator": "synth" | "ism", "params": {name: [lo, hi], ...},
  "noise": {"kind": "none"} or {"kind": "compound_poisson", "particles_per_source": [lo, hi]},
  "step_ms": x, "run_over_t60": [lo, hi], "R_m": [lo, hi]}. Synth parameter names are
  synth_fresh's spec names ('t60_s', 'drr_db', 'd_m', 'gap_ms', 'delay_ms', 'ratio',
  'late_share_db').
- validate_class(cls) -> (True, None), or (False, reason) when it is malformed or out of bounds;
  the reason names the offending key. Bounds are PHYSICS.md's hard limits (LIMITS: T60 0.1-10 s,
  DRR -40 to +30 dB) and what the product accepts.
- class_sha256(cls) -> str: the sha256 of the class's canonical JSON (keys sorted), so the same
  class written in another key order has the same hash.
- draws(cls, n=DRAWS) -> [instance]: n instances seeded by class_sha256(cls) and ATTACK_SEED, each inside
  the class's box: a dict with a value for every name in cls['params'], and 'R_m', 'run_over_t60'
  and 'step_ms'. The harness computes their truth itself (P26).
- physics_md() -> str: PHYSICS.md (SPPS histogram physics and the hard limits).
- build_sandbox(dest, *, sentinel=None) -> [path]: refuses (driver.Refused 'attack_before_sentinel') while the sentinel
  file (default round2.SENTINEL) does not exist; writes exactly SANDBOX_FILES into dest, no folder: method.py
  (frozen2/method.py, byte for byte), INTERFACE.md, PHYSICS.md and generators.py (the synth, ISM
  and compound-Poisson generators with the product's source delay, importing nothing from the
  harness or target/).
- judge_prompt(class_files) -> str: for {file name: JSON text}, the judge's instructions, every
  class file verbatim and PHYSICS.md verbatim, and no result field; ValueError when a class file
  holds one.
- parse_vote(raw) -> 'plausible' | 'implausible' | None: a judge's answer for one class is JSON
  text {"verdict": "plausible" | "implausible", "reasons": "..."}; anything else, or None, is None.
- panel(raw_votes) -> 'implausible' only when there are exactly three votes and all three parse as
  'implausible'; 'plausible' otherwise (a split, a missing vote or one that does not parse).

Where parse_vote's contract is silent, it reads "anything else" strictly, since a vote that does not
parse counts as plausible and so can only fail H6, never pass it (P26): the text must be one JSON
object with exactly the keys 'verdict' and 'reasons', no key twice; the verdict exactly 'plausible'
or 'implausible'; the reasons a string that is not blank ("with reasons", P26). A raw vote that is
not a str, bytes included, is None. panel takes a list or tuple of raw votes; anything else, None
included, is 'plausible'. Three copies of one text count as three votes: the panel cannot tell
judges apart, and the harness gives it one vote per judge.
"""
import hashlib
import json
import re
from pathlib import Path

import numpy as np

from . import driver, round2
from .corpus import FROZEN2, FROZEN2_SHA256, VoidRun, sha256_bytes

SANDBOX = round2.ATTACK_SANDBOX
ATTACK_SEED = driver.ATTACK_SEED
SANDBOX_FILES = ('INTERFACE.md', 'PHYSICS.md', 'generators.py', 'method.py')
LIMITS = {'t60_s': (0.1, 10.0), 'drr_db': (-40.0, 30.0)}     # P25, PHYSICS.md's hard limits
MAX_CLASSES = 30
DRAWS = 20
REPRODUCIBLE = 5                    # P26: 5 or more of 20 draws wrong-silent
VERDICTS = ('plausible', 'implausible')
PANEL_SIZE = 3                      # P26: three judges, implausible only 3 of 3

GENERATORS = ('synth', 'ism')
NOISE_KINDS = ('none', 'compound_poisson')
# P26's result vocabulary: a class file holding any of these is a leaked outcome, not a configuration.
RESULT_KEYS = {'status', 'edt', 'edt_lo', 'edt_hi', 'reason', 'truth', 'truth_edt', 'truth_lo', 'truth_hi',
               'wrong_silent', 'n_wrong', 'cls', 'err', 'weak_spots', 'truth_kind', 'truth_status', 'truth_share',
               'result', 'results', 'verdict', 'score'}


# ================================================================================================
# The class schema and its bound checks (P26).
# ================================================================================================
def _box_ok(box):
    return (isinstance(box, (list, tuple)) and len(box) == 2
            and all(isinstance(x, (int, float)) and not isinstance(x, bool) for x in box))


def validate_class(cls):
    if not isinstance(cls, dict):
        return False, 'class is not a dict'
    gen = cls.get('generator')
    if gen not in GENERATORS:
        return False, 'generator: %r is not one of %s' % (gen, GENERATORS)
    params = cls.get('params')
    if not isinstance(params, dict) or not params:
        return False, 'params: must be a non-empty dict of name -> [lo, hi]'
    for name, box in params.items():
        if not _box_ok(box):
            return False, '%s: not a [lo, hi] box' % name
        lo, hi = box
        if lo > hi:
            return False, '%s: lo %r is above hi %r' % (name, lo, hi)
        if name in LIMITS:
            llo, lhi = LIMITS[name]
            if lo < llo or hi > lhi:
                return False, "%s: [%r, %r] is outside PHYSICS.md's limit [%r, %r]" % (name, lo, hi, llo, lhi)
    for key in ('R_m', 'run_over_t60'):
        box = cls.get(key)
        if not _box_ok(box):
            return False, '%s: not a [lo, hi] box' % key
        lo, hi = box
        if lo > hi:
            return False, '%s: lo %r is above hi %r' % (key, lo, hi)
    step = cls.get('step_ms')
    if not isinstance(step, (int, float)) or isinstance(step, bool) or step <= 0:
        return False, 'step_ms: must be a positive number'
    noise = cls.get('noise')
    if not isinstance(noise, dict) or noise.get('kind') not in NOISE_KINDS:
        return False, 'noise: kind must be one of %s' % (NOISE_KINDS,)
    if noise.get('kind') == 'compound_poisson':
        box = noise.get('particles_per_source')
        if not _box_ok(box) or box[0] > box[1] or box[0] <= 0:
            return False, 'noise.particles_per_source: not a positive [lo, hi] box'
    return True, None


def class_sha256(cls):
    """The sha256 of the class's canonical JSON, keys sorted at every level (json.dumps' own rule),
    so the same class written in another key order hashes the same."""
    canon = json.dumps(cls, sort_keys=True, separators=(',', ':'), ensure_ascii=True)
    return hashlib.sha256(canon.encode('utf-8')).hexdigest()


def _uniform(rng, box):
    lo, hi = box
    return float(lo + (hi - lo) * rng.random())


def draws(cls, n=DRAWS):
    """n instances seeded by class_sha256(cls) and ATTACK_SEED (round 2): a fixed draw order (params by sorted
    name, then R_m, run_over_t60 and, for compound-Poisson noise, particles_per_source) so a class written in
    another key order, which hashes the same, draws the same instances, and not round 1's instances of it."""
    rng = np.random.default_rng(np.random.SeedSequence([int(class_sha256(cls), 16), ATTACK_SEED]))
    params = cls['params']
    names = sorted(params)
    noise = cls.get('noise') or {}
    compound = noise.get('kind') == 'compound_poisson'
    out = []
    for _ in range(n):
        inst = {name: _uniform(rng, params[name]) for name in names}
        inst['R_m'] = _uniform(rng, cls['R_m'])
        inst['run_over_t60'] = _uniform(rng, cls['run_over_t60'])
        inst['step_ms'] = float(cls['step_ms'])
        if compound:
            inst['particles_per_source'] = _uniform(rng, noise['particles_per_source'])
        out.append(inst)
    return out


def implied_volume_m3(t60_s, drr_db, d_m):
    """PHYSICS.md's diffuse-field consistency check, read in reverse: the room volume a diffuse
    field at t60_s would need, to produce drr_db at distance d_m, from r_c = 0.057 sqrt(V / T60)
    and DRR(d) = 20 log10(r_c / d). Pure arithmetic; it does not judge a class, only computes the
    number PHYSICS.md's prose and the judge panel both reason about."""
    return t60_s * (d_m / 0.057) ** 2 * 10 ** (drr_db / 10.0)


# ================================================================================================
# The sandbox (P25): PHYSICS.md, INTERFACE.md, generators.py and the frozen method, byte for byte.
# ================================================================================================
def physics_md():
    return (
        "# PHYSICS.md\n\n"
        "SPPS histogram physics, for a class of inputs to the EDT method under test.\n\n"
        "## What a bin holds\n\n"
        "A point or ball receiver's output for one octave band is a sequence of energy bins, bin k\n"
        "covering [k dt, (k+1) dt) seconds, dt the run's time step. In Random mode a bin's recorded\n"
        "value is a compound-Poisson count: particles of fixed energy w arrive at the receiver's ball\n"
        "(radius R, centred R_m/c seconds past emission along the straight path) independently per\n"
        "step, so a bin's variance scales with its own mean, not with the series as a whole. In\n"
        "Energetic mode the same expected energy is recorded with no counting noise. Direct sound is\n"
        "spread over the ball's path-length window around the straight-path arrival time; a reflected\n"
        "or reverberant contribution arrives everywhere the decaying field reaches the ball. A source\n"
        "may start after the run begins (its delay, seconds), which shifts every arrival later by the\n"
        "same amount, rounded up to the next whole time step.\n\n"
        "## Hard limits: what the product accepts\n\n"
        "- T60 (the reverberant decay's own time constant): 0.1 to 10 seconds.\n"
        "- DRR (direct-to-reverberant energy ratio, dB): -40 to +30 dB.\n\n"
        "A configuration outside either range is not one the product accepts, whatever its other\n"
        "parameters: it describes no measurement the solver can produce.\n\n"
        "## Diffuse-field consistency\n\n"
        "A diffuse sound field's critical distance, r_c = 0.057 sqrt(V / T60) (m, V the room volume\n"
        "in m^3, T60 in seconds, an omnidirectional source), is the distance at which direct and\n"
        "reverberant energy are equal; its direct-to-reverberant ratio at distance d is\n"
        "DRR(d) = 20 log10(r_c / d) dB. Read in reverse, a class's own (T60, DRR, d) implies the room\n"
        "volume a diffuse field at that T60 would need to produce that DRR at that distance:\n"
        "V = T60 (d / 0.057)^2 10^(DRR / 10). Real rooms are not always diffuse (coupled spaces, long\n"
        "rooms, specular geometry), so a class's implied volume carries a +/-10 dB allowance before it\n"
        "is judged. A class is physically implausible on this ground only when every one of its\n"
        "instances fails in the same direction: its implied volume exceeds 1e8 m^3 (about eight times\n"
        "the largest enclosed building on Earth) even at the -10 dB end of the allowance, or its\n"
        "implied volume is too small to hold its own source-receiver distance -- a room cannot be\n"
        "smaller than the straight-line path inside it, so V < d^3 -- even at the +10 dB end of the\n"
        "allowance, the end most generous to the class.\n"
    )


REASON_MEANING = {
    'no_energy': 'the bins hold no energy (or fewer than 4 bins, or a non-positive step)',
    'no_energy_after_arrival': 'no energy is recorded at or after the arrival',
    'run_too_short': 'the run ends before the decay has fallen 10 dB, or the unrecorded tail is over 2 % of the energy at -10 dB',
    'direct_only': 'the decay reaches -10 dB inside the direct sound',
    'step_too_coarse': 'fewer than 8 steps between the ball\'s back and -10 dB',
    'not_decaying': 'the fitted slope does not fall',
    'too_few_particles': 'nothing is recorded after -10 dB',
    'not_decaying_at_run_end': 'the energy is not falling at the end of the run, so the unrecorded tail is unbounded',
    'receiver_too_large': 'the ball\'s half-crossing time h = R / c is above 0.01 of the fitted EDT, so the fit cannot see '
                          'the first h of the decay',
}


def refusal_reasons(method_bytes):
    """Every reason frozen2/method.py can refuse with, in the order the file first names them: the quoted
    words on the lines that call _refuse( (the definition line excluded)."""
    out = []
    for line in method_bytes.decode('utf-8').splitlines():
        if '_refuse(' in line and not line.lstrip().startswith('def '):
            for w in re.findall(r"'([a-z_]+)'", line):
                if w not in out:
                    out.append(w)
    return out


def interface_md(reasons=None):
    reasons = refusal_reasons(FROZEN2.read_bytes()) if reasons is None else list(reasons)
    lines = [
        "# INTERFACE.md\n\n"
        "`method.py` exposes one entry point:\n\n"
        "    analyse(bins, dt, t_arrival, meta=None)\n\n"
        "- `bins`: a 1-D array of per-step energy (see PHYSICS.md), one receiver and one band.\n"
        "- `dt`: the time step, seconds.\n"
        "- `t_arrival`: the straight-path arrival time, seconds, or None when it is not known.\n"
        "- `meta`: an optional dict; its only key read is `half_width`, the ball radius R over the\n"
        "  speed of sound c, seconds (default 0.31 / 343.2 m / (m/s)).\n\n"
        "It returns a dict with `edt`, `edt_lo`, `edt_hi`, `status` and `reason`. Nothing else in\n"
        "`method.py` is a public interface: read it for what it does with the arguments above, not for\n"
        "a second entry point.\n\n"
        "## Statuses\n\n"
        "- status 'ok': an EDT with a shown range inside the 5 % just-noticeable difference.\n"
        "- status 'wide': an EDT whose shown range is wider than that; it is usable, not confident.\n"
        "- status 'refused': `edt`, `edt_lo` and `edt_hi` are None and `reason` is one of the following.\n\n"
        "## Refusal reasons\n"]
    for r in reasons:
        lines.append('- `%s`: %s' % (r, REASON_MEANING.get(r, 'see method.py')))
    lines.append(
        "\nA refusal is never wrong-silent: the method said it would not answer, and says why. Only a status 'ok' row\n"
        "that is more than 5 % from the true EDT is wrong-silent.\n")
    return '\n'.join(lines)


def _synth_generators_source():
    return (
        '"""Synth generator: independent of the method under test, numpy only.\n\n'
        'A response is a direct-sound pulse at t_arrival plus a sum of exponential decays\n'
        'A_i exp(-k_i (t - t_r)), t_r the reflection start. histogram() integrates it into bins;\n'
        'noisy() turns a noise-free histogram into Random mode\'s compound-Poisson counts.\n'
        '"""\n'
        'import math\n'
        'import numpy as np\n\n'
        'C = 343.2\n\n\n'
        'def ball_atoms(t_arrival, h, Ed, n=400):\n'
        '    """The direct sound\'s energy, spread over the ball\'s path-length window."""\n'
        '    if h <= 0:\n'
        '        return np.array([t_arrival]), np.array([Ed])\n'
        '    tau = t_arrival - h + (np.arange(n) + 0.5) * (2 * h / n)\n'
        '    D, R = C * t_arrival, C * h\n'
        '    rho = C * tau\n'
        '    w = np.maximum(R * R - (rho - D) ** 2, 0.0) / np.maximum(rho, 1e-9)\n'
        '    return tau, Ed * w / w.sum()\n\n\n'
        'def rev_integral(t0, t1, t_r, A, k):\n'
        '    """integral of sum A_i exp(-k_i (t - t_r)) over [t0, t1], clipped to t >= t_r."""\n'
        '    lo = np.maximum(t0, t_r)\n'
        '    hi = np.maximum(t1, lo)\n'
        '    out = np.zeros_like(lo, dtype=float)\n'
        '    for a, kk in zip(A, k):\n'
        '        out += a / kk * (np.exp(-kk * (lo - t_r)) - np.exp(-kk * (hi - t_r)))\n'
        '    return out\n\n\n'
        'def histogram(dt, run_s, t_arrival, h, Ed, gap, A, k):\n'
        '    """Noise-free bins, bin n = exact integral over [n dt, (n+1) dt)."""\n'
        '    n_bins = int(round(run_s / dt))\n'
        '    n = np.arange(n_bins)\n'
        '    b = rev_integral(n * dt, (n + 1) * dt, t_arrival + gap, A, k)\n'
        '    tau, e = ball_atoms(t_arrival, h, Ed)\n'
        '    idx = np.floor(tau / dt).astype(int)\n'
        '    ok = (idx >= 0) & (idx < n_bins)\n'
        '    np.add.at(b, idx[ok], e[ok])\n'
        '    return b\n\n\n'
        'def truth_edt(t_arrival, Ed, gap, A, k, fine=1e-5, bottom_db=-10.0):\n'
        '    """The point-receiver EDT, direct sound as a vertical drop at t_arrival (Definition A)."""\n'
        '    S0 = Ed + sum(a / kk for a, kk in zip(A, k))\n\n'
        '    def s_after(u):\n'
        '        t = t_arrival + u\n'
        '        return sum(a / kk * math.exp(-kk * max(0.0, t - (t_arrival + gap))) for a, kk in zip(A, k))\n'
        '    target = S0 * 10 ** (bottom_db / 10)\n'
        '    lo, hi = 0.0, 1.0\n'
        '    while s_after(hi) > target:\n'
        '        hi *= 2\n'
        '        if hi > 1e3:\n'
        '            return float("nan")\n'
        '    for _ in range(200):\n'
        '        mid = 0.5 * (lo + hi)\n'
        '        if s_after(mid) > target:\n'
        '            lo = mid\n'
        '        else:\n'
        '            hi = mid\n'
        '    u = np.arange(1, int(hi / fine) + 1) * fine\n'
        '    if len(u) < 3:\n'
        '        return float("nan")\n'
        '    t = t_arrival + u\n'
        '    s = np.zeros_like(u)\n'
        '    for a, kk in zip(A, k):\n'
        '        s += a / kk * np.exp(-kk * np.maximum(0.0, t - (t_arrival + gap)))\n'
        '    y = 10 * np.log10(s / S0)\n'
        '    a_, _b = np.polyfit(u, y, 1)\n'
        '    return -60.0 / a_ if a_ < 0 else float("nan")\n\n\n'
        'def noisy(bins, w, rng):\n'
        '    """Random mode\'s compound-Poisson counts: every hit carries fixed energy w."""\n'
        '    lam = np.maximum(bins, 0.0) / w\n'
        '    return w * rng.poisson(lam).astype(float)\n\n\n'
        'def emission_steps(delay_s, dt):\n'
        '    """A delayed source (seconds) emits from the next whole step, in float32 arithmetic, as\n'
        '    the solver rounds it."""\n'
        '    return int(np.ceil(np.float32(delay_s) / np.float32(dt)))\n'
    )


def _ism_generators_source():
    return (
        '"""ISM generator: a specular box room\'s image-source echogram, numpy only.\n\n'
        'For an image source at distance D emitting energy W isotropically, the expected energy the\n'
        'receiver\'s ball of radius R records in path-length bins follows the solid-angle subtended by\n'
        'the ball from each point on the path; this integrates that per fine bin with 3-point\n'
        'Gauss-Legendre.\n'
        '"""\n'
        'import math\n'
        'import numpy as np\n\n'
        'GL_X = np.array([-math.sqrt(3 / 5), 0.0, math.sqrt(3 / 5)])\n'
        'GL_W = np.array([5 / 9, 8 / 9, 5 / 9])\n\n\n'
        'def iso9613_db_per_m(F, H=50.0, P=101325.0, T_c=20.0):\n'
        '    K = T_c + 273.15\n'
        '    K01, Pref, Kref = 273.16, 101325.0, 293.15\n'
        '    C = -6.8346 * (K01 / K) ** 1.261 + 4.6151\n'
        '    Ps = Pref * 10 ** C\n'
        '    hmol = H * Ps / Pref\n'
        '    cson = 343.2 * math.sqrt(K / Kref)\n'
        '    Acr = (Pref / P) * 1.60e-10 * math.sqrt(K / Kref) * F ** 2\n'
        '    FmolO, KvibO, FmolN, KvibN = 0.209, 2239.1, 0.781, 3352.0\n'
        '    Fr = (P / Pref) * (24. + 4.04e4 * hmol * (0.02 + hmol) / (0.391 + hmol))\n'
        '    Am = 1.559 * FmolO * math.exp(-KvibO / K) * (KvibO / K) ** 2\n'
        '    AvibO = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)\n'
        '    Fr = (P / Pref) * math.sqrt(Kref / K) * (9. + 280. * hmol * math.exp(-4.170 * ((K / Kref) ** (-1. / 3.) - 1)))\n'
        '    Am = 1.559 * FmolN * math.exp(-KvibN / K) * (KvibN / K) ** 2\n'
        '    AvibN = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)\n'
        '    return Acr + AvibO + AvibN\n\n\n'
        'def m_energy(F):\n'
        '    return iso9613_db_per_m(F) * math.log(10) / 10\n\n\n'
        'def images(L, s, alpha, lmax):\n'
        '    """(positions (N,3), weights (N,), orders (N,)) of every image within lmax of the room.\n'
        '    alpha: [(x_lo, x_hi), (y_lo, y_hi), (z_lo, z_hi)]."""\n'
        '    per_axis = []\n'
        '    for ax in range(3):\n'
        '        nmax = int(math.ceil(lmax / (2 * L[ax]))) + 1\n'
        '        cs, ws, os_ = [], [], []\n'
        '        for n in range(-nmax, nmax + 1):\n'
        '            for p in (0, 1):\n'
        '                cs.append(2 * n * L[ax] + (-1) ** p * s[ax])\n'
        '                lo, hi = abs(n - p), abs(n)\n'
        '                ws.append((1 - alpha[ax][0]) ** lo * (1 - alpha[ax][1]) ** hi)\n'
        '                os_.append(lo + hi)\n'
        '        per_axis.append((np.array(cs), np.array(ws), np.array(os_)))\n'
        '    X, Y, Z = np.meshgrid(per_axis[0][0], per_axis[1][0], per_axis[2][0], indexing="ij")\n'
        '    W = (per_axis[0][1][:, None, None] * per_axis[1][1][None, :, None] * per_axis[2][1][None, None, :])\n'
        '    O = (per_axis[0][2][:, None, None] + per_axis[1][2][None, :, None] + per_axis[2][2][None, None, :])\n'
        '    P = np.stack([X.ravel(), Y.ravel(), Z.ravel()], axis=1)\n'
        '    return P, W.ravel(), O.ravel()\n\n\n'
        'def echogram(L, s, r, R, alpha, lmax, dl, chunk=3000):\n'
        '    """Expected recorded energy per path-length bin of width dl, no air absorption, split into\n'
        '    (direct, reflected). Bin i covers [i dl, (i+1) dl)."""\n'
        '    P, W, O = images(L, s, alpha, lmax + R)\n'
        '    D = np.linalg.norm(P - np.asarray(r, float)[None, :], axis=1)\n'
        '    keep = (D - R < lmax) & (W > 0)\n'
        '    P, W, O, D = P[keep], W[keep], O[keep], D[keep]\n'
        '    nb = int(math.ceil(lmax / dl))\n'
        '    direct = np.zeros(nb)\n'
        '    refl = np.zeros(nb)\n'
        '    nspan = int(math.ceil(2 * R / dl)) + 2\n'
        '    for a in range(0, len(D), chunk):\n'
        '        Dc, Wc, Oc = D[a:a + chunk], W[a:a + chunk], O[a:a + chunk]\n'
        '        assert np.all(Dc > R), "receiver ball contains an image"\n'
        '        i0 = np.floor((Dc - R) / dl).astype(np.int64)\n'
        '        idx = i0[:, None] + np.arange(nspan)[None, :]\n'
        '        lo = np.maximum(idx * dl, (Dc - R)[:, None])\n'
        '        hi = np.minimum((idx + 1) * dl, (Dc + R)[:, None])\n'
        '        wid = np.clip(hi - lo, 0, None)\n'
        '        mid = 0.5 * (lo + hi)\n'
        '        acc = np.zeros_like(wid)\n'
        '        for x, w in zip(GL_X, GL_W):\n'
        '            rho = mid + 0.5 * wid * x\n'
        '            u = rho - Dc[:, None]\n'
        '            acc += w * (R * R - u * u) / rho\n'
        '        val = (Wc / (4 * Dc))[:, None] * acc * 0.5 * wid\n'
        '        val[wid <= 0] = 0\n'
        '        ok = (idx < nb)\n'
        '        isdir = (Oc == 0)\n'
        '        for tgt, sel in ((direct, isdir), (refl, ~isdir)):\n'
        '            if sel.any():\n'
        '                ii = idx[sel][ok[sel]]\n'
        '                vv = val[sel][ok[sel]]\n'
        '                tgt += np.bincount(ii, weights=vv, minlength=nb)[:nb]\n'
        '    return direct, refl\n\n\n'
        'def air_factor(nb, dl, m, c, dt=None):\n'
        '    """Per fine bin (width dl in path length): exp(-m rho_mid) if dt is None, else the\n'
        '    step-wise p ** (n+1) with n the step (of length c*dt) holding the fine bin."""\n'
        '    rho_mid = (np.arange(nb) + 0.5) * dl\n'
        '    if dt is None:\n'
        '        return np.exp(-m * rho_mid)\n'
        '    step = c * dt\n'
        '    n = np.floor((np.arange(nb) * dl) / step + 1e-9)\n'
        '    return np.exp(-m * step * (n + 1))\n'
    )


def generators_py():
    return (
        '"""generators.py: the synth and ISM generators, and the compound-Poisson noise model, for\n'
        'building a class\'s instances into inputs the method under test accepts. Self-contained: no\n'
        'import of anything outside this file.\n'
        '"""\n'
        '# ==== synth: a closed-form double-slope response plus its noise model =========================\n'
        + _synth_generators_source()
        + '\n\n# ==== ism: a specular box room\'s image-source echogram =========================================\n'
        + _ism_generators_source()
    )


def build_sandbox(dest, *, sentinel=None):
    sentinel = Path(round2.SENTINEL if sentinel is None else sentinel)
    if not sentinel.is_file():
        raise driver.Refused('attack_before_sentinel', '%s does not exist: the attack waits for the sentinel seat\'s recount '
                                                       '(HARNESS-PLAN-2.md section 7)' % sentinel)
    data = Path(FROZEN2).read_bytes()
    if sha256_bytes(data) != FROZEN2_SHA256:
        raise VoidRun('frozen2/method.py has sha256 %s, not %s: nothing was written to the sandbox'
                      % (sha256_bytes(data), FROZEN2_SHA256))
    dest = Path(dest)
    dest.mkdir(parents=True, exist_ok=True)
    for name in dest.iterdir():     # the sandbox holds exactly SANDBOX_FILES, never a leftover
        if name.name not in SANDBOX_FILES:
            if name.is_dir():
                raise VoidRun('%s is not a sandbox file' % name)
            name.unlink()
    (dest / 'method.py').write_bytes(data)
    (dest / 'INTERFACE.md').write_text(interface_md(refusal_reasons(data)), encoding='utf-8')
    (dest / 'PHYSICS.md').write_text(physics_md(), encoding='utf-8')
    (dest / 'generators.py').write_text(generators_py(), encoding='utf-8')
    return [dest / name for name in SANDBOX_FILES]


# ================================================================================================
# The judges' prompt (P26): the class files and PHYSICS.md only, never a result.
# ================================================================================================
def _holds_result_field(obj):
    if isinstance(obj, dict):
        if RESULT_KEYS & set(obj):
            return True
        return any(_holds_result_field(v) for v in obj.values())
    if isinstance(obj, list):
        return any(_holds_result_field(v) for v in obj)
    return False


def judge_prompt(class_files):
    for name, text in class_files.items():
        if _holds_result_field(json.loads(text)):
            raise ValueError('%s holds a result field: it cannot reach a blind judge' % name)
    parts = [
        "You are judging whether each configuration below is physically plausible: one a real "
        "acoustic measurement, consistent with PHYSICS.md's limits, could produce. You are not told "
        "what any method does with it, and you are not asked to guess. Judge the configuration on "
        "its own terms, against PHYSICS.md alone.\n\n"
        "For each configuration, answer with one line of JSON: "
        '{"verdict": "plausible" or "implausible", "reasons": "..."}. Give a verdict for every '
        "configuration listed below, each on its own merits.",
    ]
    for name, text in sorted(class_files.items()):
        parts.append('## %s\n```json\n%s\n```' % (name, text))
    parts.append('## PHYSICS.md\n' + physics_md())
    return '\n\n'.join(parts)


def _object_once(pairs):
    keys = [k for k, _ in pairs]
    if len(set(keys)) != len(keys):
        raise ValueError('a key appears twice: %s' % keys)
    return dict(pairs)


def parse_vote(raw):
    if not isinstance(raw, str):
        return None
    try:
        obj = json.loads(raw, object_pairs_hook=_object_once)
    except (ValueError, RecursionError):        # not JSON, or a key twice
        return None
    if not isinstance(obj, dict) or set(obj) != {'verdict', 'reasons'}:
        return None
    verdict, reasons = obj['verdict'], obj['reasons']
    if not isinstance(verdict, str) or verdict not in VERDICTS:
        return None
    if not isinstance(reasons, str) or not reasons.strip():
        return None
    return verdict


def panel(raw_votes):
    if not isinstance(raw_votes, (list, tuple)) or len(raw_votes) != PANEL_SIZE:
        return 'plausible'
    if all(parse_vote(v) == 'implausible' for v in raw_votes):
        return 'implausible'
    return 'plausible'
