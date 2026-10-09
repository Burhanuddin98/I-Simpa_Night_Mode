"""Dry material for the Listen window (decision 84): clips made by this repository's own code, so
anechoic by construction and GPL-3.0 like the app.

Writes 48 kHz mono 16-bit WAVs into app/src-tauri/examples/clips/, each with a provenance file beside
it that says how it was made and what was measured on the file as written:

  clap-pattern   hand-clap-like noise bursts and rim clicks, with silences of 0.5 to 1.5 s between
                 them so the room's tail is heard on its own;
  pluck-melody   a short tune on a modal plucked string, staccato and held notes, every note damped
                 at its release;
  drum-groove    a synthesised kick, snare and closed hi-hat, two bars, a break, and a last hit.

Every event has a release: from it, the event is damped exponentially (60 dB in RELEASE_DECAY_S) and
faded to exact zero by RELEASE_S, so the file is digital silence until the next event. The check,
made on the 16-bit file read back: for each event (events sharing an onset are one), the time from
its release until every 1 ms frame up to the next event is 60 dB or more below the mean square of
the millisecond before the release. The script fails when any event takes longer than TAIL_LIMIT_S.

Deterministic: one seed (SEED) drives every random choice through numpy's PCG64. `--check`
regenerates the clips in memory and compares them with the files on disk byte for byte, writing
nothing.

    python tools/clips/make_dry_clips.py            # write the WAVs and provenance files
    python tools/clips/make_dry_clips.py --check    # regenerate and compare, write nothing
"""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import sys
import wave
from dataclasses import dataclass

import numpy as np

RATE = 48_000
SEED = 0x4E4D_4452_5931  # "NMDRY1"
PEAK_DBFS = -1.0
RELEASE_DECAY_S = 0.035  # 60 dB of exponential damping from an event's release
RELEASE_S = 0.040  # then a raised-cosine fade to exact zero by this time after the release
TAIL_LIMIT_S = 0.050  # the criterion: 60 dB down within 50 ms of each event's end
FRAME_S = 0.001

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "app", "src-tauri", "examples", "clips")
SCRIPT = "tools/clips/make_dry_clips.py"


@dataclass
class Event:
    """One sounding event: its onset and its release (the event's end), seconds into the clip."""

    onset: float
    release: float
    what: str


# ---- building blocks ----------------------------------------------------------------------------


def release_envelope(n_body: int) -> np.ndarray:
    """1 over the body, then 60 dB of exponential decay in RELEASE_DECAY_S and a raised-cosine
    fade to exactly 0 at RELEASE_S."""
    n_dec = int(round(RELEASE_DECAY_S * RATE))
    n_rel = int(round(RELEASE_S * RATE))
    t = np.arange(n_rel) / RATE
    rel = 10.0 ** (-3.0 * t / RELEASE_DECAY_S)
    fade = np.ones(n_rel)
    k = np.arange(n_rel - n_dec)
    fade[n_dec:] = 0.5 * (1.0 + np.cos(np.pi * (k + 1) / (n_rel - n_dec)))
    return np.concatenate([np.ones(n_body), rel * fade])


def shaped_noise(rng: np.random.Generator, n: int, lo: float, hi: float, order: int = 2) -> np.ndarray:
    """`n` samples of Gaussian noise through a zero-phase band-pass (Butterworth magnitudes of
    `order` at `lo` and `hi` Hz; `lo` 0 for none), applied by FFT, unit RMS."""
    m = 1 << int(np.ceil(np.log2(2 * n + 1)))
    x = np.zeros(m)
    x[:n] = rng.standard_normal(n)
    f = np.fft.rfftfreq(m, 1.0 / RATE)
    f[0] = 1e-9
    h = 1.0 / np.sqrt(1.0 + (f / hi) ** (2 * order))
    if lo > 0:
        h /= np.sqrt(1.0 + (lo / f) ** (2 * order))
    y = np.fft.irfft(np.fft.rfft(x) * h, m)[:n]
    return y / (np.sqrt(np.mean(y**2)) + 1e-30)


def place(buf: np.ndarray, at_s: float, x: np.ndarray, gain_db: float) -> None:
    i = int(round(at_s * RATE))
    buf[i : i + len(x)] += x * 10.0 ** (gain_db / 20.0)


# ---- (a) claps and rim clicks ---------------------------------------------------------------------


def clap(rng: np.random.Generator) -> tuple[np.ndarray, float]:
    """A hand-clap-like burst: three short noise flicks 2-4 ms apart, then the main burst decaying
    with a time constant of 6-8 ms; band 600 Hz to 6 kHz with a random tilt. Returns the samples
    and the release time (s after the onset)."""
    body_s = 0.032
    n = int(round((body_s + RELEASE_S) * RATE))
    t = np.arange(n) / RATE
    noise = shaped_noise(rng, n, rng.uniform(550, 750), rng.uniform(4500, 6500))
    env = np.zeros(n)
    at = 0.0
    for _ in range(3):
        env += np.where(t >= at, np.exp(-(t - at) / 0.0012), 0.0) * rng.uniform(0.5, 0.8)
        at += rng.uniform(0.002, 0.004)
    env += np.where(t >= at, np.exp(-(t - at) / rng.uniform(0.006, 0.008)), 0.0)
    return noise * env * release_envelope(int(round(body_s * RATE)))[:n], body_s


def rim(rng: np.random.Generator) -> tuple[np.ndarray, float]:
    """A rim click: four damped modes (about 0.75, 1.65, 2.9 and 4.4 kHz, time constants 2-6 ms)
    over a 1 ms noise transient."""
    body_s = 0.018
    n = int(round((body_s + RELEASE_S) * RATE))
    t = np.arange(n) / RATE
    x = np.zeros(n)
    for f, tau, a in ((750, 0.006, 1.0), (1650, 0.004, 0.7), (2900, 0.003, 0.5), (4400, 0.002, 0.35)):
        x += a * np.sin(2 * np.pi * f * rng.uniform(0.97, 1.03) * t + rng.uniform(0, 2 * np.pi)) * np.exp(-t / tau)
    tr = shaped_noise(rng, n, 1500, 9000) * np.exp(-t / 0.0006) * 0.8
    x = (x + tr) * release_envelope(int(round(body_s * RATE)))[:n]
    return x, body_s


def clap_pattern(rng: np.random.Generator) -> tuple[np.ndarray, list[Event], str]:
    """12 s: claps (0 to -4 dB) and rim clicks (-6 dB), silences of 0.5 to 1.5 s between them."""
    seq = ["clap", "clap", "rim", "clap", "rim", "rim", "clap", "clap", "rim", "clap", "clap", "rim", "clap"]
    gaps = [0.55, 1.10, 0.70, 1.45, 0.50, 0.85, 1.30, 0.60, 0.95, 0.50, 1.20, 0.75]
    total_s = 12.0
    buf = np.zeros(int(total_s * RATE))
    events: list[Event] = []
    at = 0.25
    for i, kind in enumerate(seq):
        x, body = clap(rng) if kind == "clap" else rim(rng)
        gain = rng.uniform(-4.0, 0.0) if kind == "clap" else rng.uniform(-7.0, -5.0)
        place(buf, at, x, gain)
        events.append(Event(at, at + body, kind))
        if i < len(gaps):
            at += body + RELEASE_S + gaps[i]
    desc = (
        f"{sum(1 for k in seq if k == 'clap')} hand-clap-like bursts and {sum(1 for k in seq if k == 'rim')} rim clicks, "
        f"silences of {min(gaps):.2f} to {max(gaps):.2f} s between one event's last sample and the next onset. "
        "A clap: three noise flicks 2-4 ms apart and a main burst decaying with a 6-8 ms time constant, noise "
        "band-passed about 600 Hz to 6 kHz (zero-phase, by FFT), 32 ms long before its release. A rim click: four "
        "damped modes near 0.75, 1.65, 2.9 and 4.4 kHz (time constants 6, 4, 3, 2 ms) over a 1 ms noise transient, "
        "18 ms before its release. Levels: claps 0 to -4 dB, rim clicks -5 to -7 dB, random per event."
    )
    return buf, events, desc


# ---- (b) a plucked-string melody ------------------------------------------------------------------


def pluck(rng: np.random.Generator, f0: float, sound_s: float) -> np.ndarray:
    """A modal plucked string: partials k f0 sqrt(1 + B k^2) (B 1e-4) up to 12 kHz, amplitudes
    |sin(k pi p)| / k^1.3 for a pluck at p (0.14-0.22 of the length), each decaying with a T60 of
    4 s (220/f)^0.75 (at most 5 s); a 0.4 ms attack and a 3 ms pluck noise; damped at `sound_s`."""
    n_body = int(round(sound_s * RATE))
    n = n_body + int(round(RELEASE_S * RATE))
    t = np.arange(n) / RATE
    p = rng.uniform(0.14, 0.22)
    x = np.zeros(n)
    k = 1
    while True:
        fk = k * f0 * np.sqrt(1.0 + 1e-4 * k * k)
        if fk > 12_000:
            break
        t60 = min(5.0, 4.0 * (220.0 / fk) ** 0.75)
        a = abs(np.sin(k * np.pi * p)) / k**1.3
        x += a * np.sin(2 * np.pi * fk * t) * 10.0 ** (-3.0 * t / t60)
        k += 1
    x /= np.max(np.abs(x[: int(0.05 * RATE)])) + 1e-30
    att = np.minimum(1.0, t / 0.0004)
    noise = shaped_noise(rng, n, 800, 7000) * np.exp(-t / 0.001) * 0.08
    return (x * att + noise) * release_envelope(n_body)[:n]


def midi_hz(m: int) -> float:
    return 440.0 * 2.0 ** ((m - 69) / 12.0)


def pluck_melody(rng: np.random.Generator) -> tuple[np.ndarray, list[Event], str]:
    """An original tune in A minor at 96 beats a minute: staccato eighths (sounding 0.14 s) and held
    notes (sounding their length less 0.12 s), rests between phrases."""
    beat = 60.0 / 96.0
    # (midi note or None for a rest, length in beats, 's' staccato or 'h' held)
    score = [
        (57, 0.5, "s"), (60, 0.5, "s"), (64, 0.5, "s"), (69, 1.5, "h"), (67, 0.5, "s"), (64, 0.5, "s"), (62, 2.0, "h"),
        (None, 1.0, ""),
        (64, 0.5, "s"), (67, 0.5, "s"), (69, 0.5, "s"), (72, 1.5, "h"), (71, 0.5, "s"), (67, 0.5, "s"), (69, 2.5, "h"),
        (None, 1.5, ""),
        (57, 0.5, "s"), (52, 0.5, "s"), (57, 0.5, "s"), (60, 1.0, "h"), (59, 0.5, "s"), (55, 0.5, "s"), (52, 1.0, "h"),
        (None, 0.5, ""),
        (64, 0.5, "s"), (62, 0.5, "s"), (60, 0.5, "s"), (59, 0.5, "s"), (45, 3.0, "h"),
    ]
    lead = 0.25
    length = lead + sum(b for _, b, _ in score) * beat + 1.0
    buf = np.zeros(int(round(length * RATE)))
    events: list[Event] = []
    at = lead
    notes = held = 0
    for m, beats, art in score:
        if m is not None:
            sound = 0.14 if art == "s" else beats * beat - 0.12
            x = pluck(rng, midi_hz(m), sound)
            place(buf, at, x, rng.uniform(-1.5, 0.0) - (0.0 if art == "h" else 2.0))
            events.append(Event(at, at + sound, f"note {m} {art}"))
            notes += 1
            held += art == "h"
        at += beats * beat
    desc = (
        f"{notes} notes ({notes - held} staccato, {held} held), MIDI 45 to 72 (A2 to C5), an original tune in A minor "
        "at 96 beats a minute with rests between its four phrases. Each note is a modal plucked string: partials at "
        "k f0 sqrt(1 + 1e-4 k^2) up to 12 kHz, amplitudes |sin(k pi p)|/k^1.3 for a pluck position p drawn in "
        "0.14-0.22, each partial decaying with a T60 of 4 s (220 Hz/f)^0.75 (at most 5 s), a 0.4 ms attack and a "
        "3 ms pluck noise. A staccato note sounds 0.14 s, a held note its length less 0.12 s; then the string is "
        "damped (the release). Levels random per note within 1.5 dB, staccato 2 dB below held."
    )
    return buf, events, desc


# ---- (c) a drum groove ------------------------------------------------------------------------------


def kick(rng: np.random.Generator) -> tuple[np.ndarray, float]:
    body_s = 0.180
    n = int(round((body_s + RELEASE_S) * RATE))
    t = np.arange(n) / RATE
    f = 50.0 + 90.0 * np.exp(-t / 0.030)
    phase = 2 * np.pi * np.cumsum(f) / RATE
    x = np.sin(phase) * np.exp(-t / 0.120)
    x += shaped_noise(rng, n, 1000, 6000) * np.exp(-t / 0.0015) * 0.3
    return x * release_envelope(int(round(body_s * RATE)))[:n], body_s


def snare(rng: np.random.Generator) -> tuple[np.ndarray, float]:
    body_s = 0.150
    n = int(round((body_s + RELEASE_S) * RATE))
    t = np.arange(n) / RATE
    tone = (np.sin(2 * np.pi * 185 * t) + 0.6 * np.sin(2 * np.pi * 330 * t)) * np.exp(-t / 0.030)
    noise = shaped_noise(rng, n, 1000, 8000) * np.exp(-t / 0.050)
    return (0.6 * tone + 0.7 * noise) * release_envelope(int(round(body_s * RATE)))[:n], body_s


def hat(rng: np.random.Generator) -> tuple[np.ndarray, float]:
    body_s = 0.050
    n = int(round((body_s + RELEASE_S) * RATE))
    t = np.arange(n) / RATE
    x = shaped_noise(rng, n, 6000, 16000, order=3) * np.exp(-t / 0.012)
    return x * release_envelope(int(round(body_s * RATE)))[:n], body_s


def drum_groove(rng: np.random.Generator) -> tuple[np.ndarray, list[Event], str]:
    """100 beats a minute: two bars of kick, snare and eighth-note hats, a 1.2 s break, then one bar
    and a last kick and snare together, and 1.5 s of silence."""
    e = 60.0 / 100.0 / 2.0  # an eighth note
    bar = ["kh", "h", "sh", "h", "kh", "kh", "sh", "h"]
    bar2 = ["kh", "h", "sh", "kh", "h", "kh", "sh", "sh"]
    hits: list[tuple[float, str]] = []
    at = 0.25
    for pattern in (bar, bar2):
        for slot in pattern:
            hits.append((at, slot))
            at += e
    at += 1.2
    for slot in bar:
        hits.append((at, slot))
        at += e
    hits.append((at, "ks"))
    length = at + 1.5
    buf = np.zeros(int(round(length * RATE)))
    events: list[Event] = []
    make = {"k": (kick, -1.0), "s": (snare, -3.0), "h": (hat, -11.0)}
    for t0, slot in hits:
        end = 0.0
        for c in slot:
            fn, g = make[c]
            x, body = fn(rng)
            place(buf, t0, x, g + rng.uniform(-1.0, 0.0))
            end = max(end, body)
        events.append(Event(t0, t0 + end, slot))
    desc = (
        f"{len(hits)} hits at 100 beats a minute on an eighth-note grid: two bars, a 1.2 s break, a third bar and a "
        "last kick and snare together, then 1.5 s of silence. Kick: a sine falling from 140 to 50 Hz (time constant "
        "30 ms) decaying with 120 ms, and a 1.5 ms click, 180 ms before its release. Snare: 185 and 330 Hz tones "
        "(30 ms) and noise band-passed 1-8 kHz (50 ms), 150 ms. Closed hi-hat: noise band-passed 6-16 kHz (12 ms), "
        "50 ms, 10 dB below the kick. Hits sharing an onset are one event, released when its longest part is."
    )
    return buf, events, desc


# ---- writing and measuring ---------------------------------------------------------------------------


def to_pcm16(x: np.ndarray) -> tuple[bytes, float]:
    g = 10.0 ** (PEAK_DBFS / 20.0) / np.max(np.abs(x))
    q = np.clip(np.round(x * g * 32767.0), -32768, 32767).astype("<i2")
    out = io.BytesIO()
    with wave.open(out, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(q.tobytes())
    return out.getvalue(), g


def read_pcm16(b: bytes) -> np.ndarray:
    with wave.open(io.BytesIO(b), "rb") as w:
        assert (w.getnchannels(), w.getsampwidth(), w.getframerate()) == (1, 2, RATE)
        return np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float64) / 32768.0


def measure(x: np.ndarray, events: list[Event]) -> dict:
    """Each event's tail on the file as read back (module docs), and the silences between events
    (from the last non-zero sample after a release to the next onset)."""
    fl = int(round(FRAME_S * RATE))
    tails = []
    silences = []
    for i, ev in enumerate(events):
        r = int(round(ev.release * RATE))
        nxt = int(round(events[i + 1].onset * RATE)) if i + 1 < len(events) else len(x)
        ref = np.mean(x[r - fl : r] ** 2)
        seg = x[r:nxt]
        nf = len(seg) // fl
        ms = np.mean(seg[: nf * fl].reshape(nf, fl) ** 2, axis=1)
        loud = np.nonzero(ms > ref * 1e-6)[0]
        tails.append((loud[-1] + 1) * FRAME_S if len(loud) else 0.0)
        nz = np.nonzero(seg)[0]
        last = r + (nz[-1] + 1 if len(nz) else 0)
        if i + 1 < len(events):
            silences.append((nxt - last) / RATE)
        ref_peak = 20 * np.log10(np.max(np.abs(x[int(round(ev.onset * RATE)) : r])) + 1e-30)
        assert ref > 0, f"event {i} ({ev.what}) is silent before its release"
        assert ref_peak > -60, f"event {i} ({ev.what}) peaks at {ref_peak:.1f} dBFS"
    return {"tails": np.array(tails), "silences": np.array(silences)}


CLIPS = [
    ("clap-pattern", "Claps and rim clicks with silences", clap_pattern),
    ("pluck-melody", "A plucked-string melody", pluck_melody),
    ("drum-groove", "A drum groove (kick, snare, hi-hat)", drum_groove),
]


def build() -> list[tuple[str, bytes, str]]:
    rng = np.random.Generator(np.random.PCG64(SEED))
    made = []
    for cid, title, fn in CLIPS:
        x, events, desc = fn(rng)
        wav, gain = to_pcm16(x)
        back = read_pcm16(wav)
        m = measure(back, events)
        worst = float(np.max(m["tails"]))
        if worst > TAIL_LIMIT_S:
            raise SystemExit(f"{cid}: an event's tail takes {worst * 1000:.0f} ms to fall 60 dB (limit {TAIL_LIMIT_S * 1000:.0f} ms)")
        sha = hashlib.sha256(wav).hexdigest()
        secs = len(back) / RATE
        sil = m["silences"]
        prov = "\n".join(
            [
                f"# {cid}.wav",
                f"- Title: {title} (generated by this repository's code, not recorded)",
                f"- Made by: {SCRIPT} (`python {SCRIPT}`; `--check` regenerates and compares byte for byte), seed {SEED:#x} "
                f"through numpy's PCG64, numpy {np.__version__}; the three clips are drawn from one generator in the order "
                "clap-pattern, pluck-melody, drum-groove",
                "- Licence: GPL-3.0-only, as the app (the script and what it writes are this repository's own work)",
                "- Source page: none; synthesised, nothing was recorded or fetched",
                f"- sha256: {sha}",
                f"- Format: {secs:.3f} s, 48 kHz, mono, 16-bit PCM, peak-normalised to {PEAK_DBFS:.1f} dBFS "
                f"(gain {gain:.6f} on the synthesised samples), no dither",
                f"- What it is: {desc}",
                f"- Each event's end (its release): damped by 60 dB in {RELEASE_DECAY_S * 1000:.0f} ms (exponentially) and "
                f"faded to exact zero by {RELEASE_S * 1000:.0f} ms (raised cosine).",
                "- Dryness: anechoic by construction (no room, no microphone, no reverberation in the synthesis). "
                f"Measured on this file as written (read back): for each of its {len(events)} events, the time from its "
                "end until every 1 ms frame up to the next event is 60 dB or more below the mean square of the "
                f"millisecond before the end: at most {np.max(m['tails']) * 1000:.0f} ms, median "
                f"{np.median(m['tails']) * 1000:.0f} ms (criterion: within {TAIL_LIMIT_S * 1000:.0f} ms). After every event the "
                f"file is digital zero until the next onset: those silences last {np.min(sil):.3f} to {np.max(sil):.3f} s.",
                "",
            ]
        )
        made.append((cid, wav, prov))
        print(f"{cid}: {secs:.3f} s, {len(events)} events, tail max {np.max(m['tails']) * 1000:.0f} ms median "
              f"{np.median(m['tails']) * 1000:.0f} ms, silences {np.min(sil):.3f}-{np.max(sil):.3f} s, sha256 {sha}")
    return made


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--check", action="store_true", help="regenerate and compare with the files on disk; write nothing")
    args = ap.parse_args()
    made = build()
    bad = 0
    for cid, wav, prov in made:
        wpath = os.path.join(OUT, f"{cid}.wav")
        ppath = os.path.join(OUT, f"{cid}.provenance.md")
        if args.check:
            for p, want, text in ((wpath, wav, False), (ppath, prov.encode("utf-8"), True)):
                have = open(p, "rb").read() if os.path.exists(p) else b""
                if text:
                    have = have.replace(b"\r\n", b"\n")
                if have != want:
                    print(f"DIFFERS: {p}")
                    bad += 1
        else:
            with open(wpath, "wb") as f:
                f.write(wav)
            with open(ppath, "w", encoding="utf-8", newline="\n") as f:
                f.write(prov)
    if args.check:
        print("check: " + ("every clip matches" if bad == 0 else f"{bad} file(s) differ"))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
