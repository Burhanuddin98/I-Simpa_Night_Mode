You are judging whether each configuration below is physically plausible: one a real acoustic measurement, consistent with PHYSICS.md's limits, could produce. You are not told what any method does with it, and you are not asked to guess. Judge the configuration on its own terms, against PHYSICS.md alone.

For each configuration, answer with one line of JSON: {"verdict": "plausible" or "implausible", "reasons": "..."}. Give a verdict for every configuration listed below, each on its own merits.

## c01.json
```json
{"generator": "synth", "params": {"t60_s": [0.5, 2], "drr_db": [3, 5.5], "d_m": [1, 6], "delay_ms": [2, 6]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.35, 0.5]}
```

## c02.json
```json
{"generator": "synth", "params": {"t60_s": [1, 3], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.3, 0.5]}
```

## c03.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [2.5, 5], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.15, 0.3]}
```

## c04.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "compound_poisson", "particles_per_source": [3000000.0, 30000000.0]}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.3, 0.5]}
```

## c05.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "compound_poisson", "particles_per_source": [30000000.0, 300000000.0]}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.3, 0.5]}
```

## c06.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.3, 0.5]}
```

## c07.json
```json
{"generator": "synth", "params": {"t60_s": [0.5, 1.5], "drr_db": [3, 5], "d_m": [1, 6], "delay_ms": [2, 8]}, "noise": {"kind": "none"}, "step_ms": 0.2, "run_over_t60": [1.5, 3], "R_m": [0.3, 0.5]}
```

## c08.json
```json
{"generator": "synth", "params": {"t60_s": [0.5, 1.5], "drr_db": [2, 5], "d_m": [1, 6], "delay_ms": [2, 10]}, "noise": {"kind": "none"}, "step_ms": 0.25, "run_over_t60": [1.5, 3], "R_m": [0.35, 0.5]}
```

## c09.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [0.5, 3]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [1.5, 3], "R_m": [0.3, 0.5]}
```

## c10.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [1.2, 1.6], "R_m": [0.3, 0.5]}
```

## c11.json
```json
{"generator": "synth", "params": {"t60_s": [0.4, 1.5], "drr_db": [3, 6], "d_m": [1, 6], "delay_ms": [1, 8]}, "noise": {"kind": "none"}, "step_ms": 0.1, "run_over_t60": [2.5, 5], "R_m": [0.3, 0.5]}
```

## PHYSICS.md
# PHYSICS.md

SPPS histogram physics, for a class of inputs to the EDT method under test.

## What a bin holds

A point or ball receiver's output for one octave band is a sequence of energy bins, bin k
covering [k dt, (k+1) dt) seconds, dt the run's time step. In Random mode a bin's recorded
value is a compound-Poisson count: particles of fixed energy w arrive at the receiver's ball
(radius R, centred R_m/c seconds past emission along the straight path) independently per
step, so a bin's variance scales with its own mean, not with the series as a whole. In
Energetic mode the same expected energy is recorded with no counting noise. Direct sound is
spread over the ball's path-length window around the straight-path arrival time; a reflected
or reverberant contribution arrives everywhere the decaying field reaches the ball. A source
may start after the run begins (its delay, seconds), which shifts every arrival later by the
same amount, rounded up to the next whole time step.

## Hard limits: what the product accepts

- T60 (the reverberant decay's own time constant): 0.1 to 10 seconds.
- DRR (direct-to-reverberant energy ratio, dB): -40 to +30 dB.

A configuration outside either range is not one the product accepts, whatever its other
parameters: it describes no measurement the solver can produce.

## Diffuse-field consistency

A diffuse sound field's critical distance, r_c = 0.057 sqrt(V / T60) (m, V the room volume
in m^3, T60 in seconds, an omnidirectional source), is the distance at which direct and
reverberant energy are equal; its direct-to-reverberant ratio at distance d is
DRR(d) = 20 log10(r_c / d) dB. Read in reverse, a class's own (T60, DRR, d) implies the room
volume a diffuse field at that T60 would need to produce that DRR at that distance:
V = T60 (d / 0.057)^2 10^(DRR / 10). Real rooms are not always diffuse (coupled spaces, long
rooms, specular geometry), so a class's implied volume carries a +/-10 dB allowance before it
is judged. A class is physically implausible on this ground only when every one of its
instances fails in the same direction: its implied volume exceeds 1e8 m^3 (about eight times
the largest enclosed building on Earth) even at the -10 dB end of the allowance, or its
implied volume is too small to hold its own source-receiver distance -- a room cannot be
smaller than the straight-line path inside it, so V < d^3 -- even at the +10 dB end of the
allowance, the end most generous to the class.
