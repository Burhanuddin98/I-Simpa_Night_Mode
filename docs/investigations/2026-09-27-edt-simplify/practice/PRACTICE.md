# How the field actually computes EDT — and what that implies for SPPS histograms

Research pass for the `edt-simplify` line of work. Scope per the task: what do ISO
3382-1/2, ISO 18233, Lundeby et al. 1995, Chu, Hirata and six simulation tools
(ODEON, CATT-Acoustic, RAVEN, EASE/AURA, Treble, Pachyderm) plus upstream I-Simpa's
own code and docs actually do about issues I1–I7, and does any of it look like the
gate-1/gate-2/x1000-safety-factor/run_too_short apparatus this project built.

**Bottom line, stated plainly:** no standard and no surveyed tool computes a
model-free guaranteed bound on EDT. All of them do one of two much cheaper things —
(a) fit a straight line to a fixed dB window of the Schroeder curve and report one
number, sometimes gated by a regression-quality check, or (b) for the noise/tail
problem specifically, assume the tail is a single exponential and correct for it in
closed form (Lundeby/Chu/Hirata). None report an uncertainty *range* on the number
they hand back. The 5 % JND that this project (and ISO 3382-1) uses as its precision
target is a borrowed value, never validated for EDT specifically — the one study
that actually measured it found EDT's real perceptual JND is closer to **18 %**, more
than 3× looser. Upstream I-Simpa's own code does not implement any of this; it fits
from the emission instant with no noise/quality check at all, which is exactly what
decision #4 in this repo's decision log already correctly identified as the bar to
clear. Clearing that bar does not require the machinery this project built to clear
it — see §6.

---

## 1. Standards

### 1.1 ISO 3382-1:2009 — Acoustics: Measurement of room acoustic parameters, Part 1: Performance spaces

- EDT is obtained from the same backward-integrated (Schroeder) decay curve as T20/T30,
  but the linear regression is fit over **0 dB to −10 dB** instead of −5/−25 or −5/−35 dB,
  then the resulting slope is extrapolated to a 60 dB-equivalent time — i.e. the raw
  10 dB fit is multiplied by 6 to get a time in seconds
  ([SonaVyx summary of ISO 3382-1 clause 3.1](https://sonavyx.com/en/learn/articles/room-acoustics-measurement);
  matches [ODEON's own description](https://odeon.dk/learn/articles/room-acoustics/) and
  [Treble's](https://docs.treble.tech/acoustic-parameters/reverberation-time)).
- **The 0 dB reference is the arrival of the direct sound, not the excitation/emission
  instant.** ODEON's Christensen/Rindel ISRA-2013 paper, written specifically about
  how ISO 3382 parameters are computed for simulated vs. measured responses, states it
  directly: EDT is "conventionally measured from the arrival of direct sound down to
  10 dB below it," and separately that "the onset of the direct sound and of the whole
  impulse response should be at least 20 dB below the peak of the direct sound" — i.e.
  ISO 3382-1 explicitly wants everything *before* the direct-sound arrival excluded or
  negligible, not folded into the fit
  ([odeon.dk/pdf/ISRA2013_Paper...pdf](https://www.odeon.dk/pdf/ISRA2013_Paper_The%20ISO%203382%20parameters_Can%20we%20simulate%20them_Can%20we%20measure%20them_24July2013.pdf),
  corroborated by search-indexed text of the same paper). This is the standard's answer
  to **I1**: the fit origin is a physical point (direct-sound arrival), not t=0 of the
  recording/emission, and pre-arrival samples are not part of the curve being fit at all.
- For T20/T30, ISO 3382-1 also specifies a **minimum-headroom rule**: the evaluation range
  must sit at least 10 dB above the noise floor, and if a 30 dB range does not have that
  headroom the standard falls back to a 20 dB range (both still extrapolated to 60 dB) —
  paraphrased from search-indexed ISO 3382-1 text, not independently verified against
  the clause number (**label: memory-supported, moderate confidence**; the underlying PDF
  could not be parsed by the fetch tool in this session, see §7 caveats).
- **Annex A JND table:** EDT's just-noticeable difference is listed as **5 %**, the same
  figure used for T ([search-indexed summary of ISO 3382-1:2009](https://www.researchgate.net/publication/328248817_A_STUDY_OF_THE_JUST_NOTICEABLE_DIFFERENCE_OF_EARLY_DECAY_TIME_EDT)).
  Critically, the same source states plainly that **this number was never independently
  validated for EDT** — it was carried over from T's JND by assumption, not measured.
- ISO 3382-1 does not specify *how* to obtain a clean decay curve from a noisy or
  truncated impulse response (no Lundeby/Chu/Hirata is named in the clause text found).
  That is left to measurement/simulation software, which is why every tool below
  implements its own noise-handling and why they disagree with each other (§4.7).

### 1.2 ISO 3382-2:2008 — Reverberation time in ordinary rooms

Defines T20/T30 the same way (Schroeder backward integration, linear regression,
5/25 and 5/35 dB ranges) for **non-performance ("ordinary") rooms** — offices,
classrooms, etc. **It does not define EDT at all** — EDT is an ISO 3382-1
(performance-space) parameter, tied to subjective reverberance judgement in halls,
not to ordinary-room descriptors
([ISO catalogue entry](https://www.iso.org/obp/ui/#iso:std:iso:3382:-2:en)). It is
listed in the task brief "if relevant" — it is not directly relevant to EDT beyond
confirming that EDT's evaluation-range convention is specific to Part 1.

### 1.3 ISO 18233:2006 — Application of new measurement methods in building and room acoustics

Governs *how the impulse response itself is obtained* (MLS and swept-sine
excitation, linearity/time-invariance requirements, environmental control) — it is
upstream of the EDT calculation, not a description of the EDT calculation itself
([ISO catalogue entry](https://www.iso.org/standard/40408.html)). It is relevant to
this project only by analogy: SPPS's Monte-Carlo particle count plays the role that
ISO 18233's SNR/excitation-level requirements play for measured IRs — both exist to
keep the *input* to the Schroeder integration clean before any decay-curve fitting
starts. It says nothing about I1–I7 directly.

---

## 2. The noise / truncation literature

### 2.1 Schroeder 1965 — the actual foundation, and the real answer to I5

M. R. Schroeder, "New Method of Measuring Reverberation Time," *JASA* 37, 409 (1965)
([ADS abstract](https://ui.adsabs.harvard.edu/abs/1965ASAJ...37S1187S/abstract)).
The point of backward integration is not merely "smooth the curve" as an afterthought
— **it is a mathematically exact replacement for ensemble-averaging many repeated
decay measurements.** A single backward integral of a squared impulse response gives
the same expected decay curve that would be obtained by averaging an infinite number
of interrupted-noise decay measurements
([search-indexed summary](https://pubs.aip.org/asa/jasa/article/157/2/R3/3333468/Schroeder-integration-for-sound-energy-decay)).
This is the field's actual answer to particle/ray statistical noise (**I5**): you do
not need a separate noise model for the receiver's Monte-Carlo counting noise,
because the backward cumulative sum already performs the averaging. What is left
after that is only the *tail* problem (I4/I6) — genuine background noise or
simulation truncation that the backward integral cannot average away because it
never ends.

### 2.2 Chu 1978 — noise subtraction

Chu, "Comparison of reverberation measurements using Schroeder's integrated impulse
response method and decay-curve averaging method," *JASA* 63, 1444–1450 (1978).
Chu's method: estimate the steady background-noise power level from the tail of the
recording, **subtract that constant power from every sample of the squared impulse
response before backward-integrating.** This directly fixes the "reverberation time
substantially overestimated because the noise tail decays much slower than the real
decay" failure mode
([search-indexed summary, citing the JASA comparison of software implementations](https://rd.springer.com/article/10.1007/s40857-016-0055-6)).
It requires an actual noise floor to estimate — for a simulated histogram with no
measurement noise, the closest analogue is the truncation/kill-threshold energy
(§5), not an instrument noise floor.

### 2.3 Lundeby, Vigran, Bietz & Vorländer 1995 — truncation *and* noise compensation together

A. Lundeby, T. E. Vigran, H. Bietz, M. Vorländer, "Uncertainties of Measurements in
Room Acoustics," *Acta Acustica united with Acustica* 81(4), 344–355 (1995)
([RWTH Aachen ITA publication page](https://www.akustik.rwth-aachen.de/cms/institut-fuer-hoertechnik-und-akustik/forschung/publikationen/~dwoc/publikationen-einzelansicht/?file=192049&lidx=1)).
This is the algorithm nearly every modern measurement package (and, by extension,
most simulation tools that bothered to implement noise handling) actually runs.
Steps, reconstructed from converging search-indexed summaries of the paper and
downstream descriptions
([search-indexed step list](https://www.mdpi.com/2075-5309/12/5/697),
[pyrato docs, module list only — full docstring not retrievable this session](https://pyrato.readthedocs.io/en/latest/api_reference.html)):

1. Square the IR, average it in local blocks of roughly **10–50 ms** (coarse enough to
   suppress single-sample noise, fine enough not to blur the decay).
2. Estimate the background-noise power from the **tail** of the response (commonly the
   last ~10 % of the recorded signal).
3. Fit a preliminary decay slope from 0 dB down to the noise estimate.
4. **Re-derive the block-averaging interval from that slope** — Lundeby specifies
   3–10 averaging intervals per 10 dB of decay, so the interval shrinks as the true
   decay rate is refined.
5. Iterate steps 2–4 a few times until the estimated intersection time converges.
6. The **intersection time** — where the fitted decay line crosses the estimated noise
   floor — becomes the truncation point: backward integration for the final decay
   curve only runs from the end of the recording *up to* this point, not to the very
   end of the buffer, so integrated late-arriving noise never contaminates the curve.

This is the standard method's answer to **I4** and **I6** simultaneously: instead of
proving a rigorous bound on what's unrecorded, it *estimates where reliable signal
ends*, throws away everything after that point, and reports the parameter only if
enough clean decay range remains above that truncation point.

### 2.4 Hirata 1982 — closed-form tail correction

Y. Hirata published a method (cited across the literature as "a method of
eliminating noise in power responses," *J. Sound Vib.* 82, 593–595, 1982) that,
rather than truncating and accepting the missing tail energy as an unquantified
loss, **estimates the missing energy analytically and adds it back**: find the knee
where the main decay slope meets the noise floor, measure the local slope there, and
integrate that exponential slope out to infinity in closed form to get the "missing"
tail energy, which is then added to the (finite) backward integral computed up to
the truncation point
([search-indexed summary](https://www.researchgate.net/publication/289228157_A_modified_approach_to_the_backward_integration_leading_to_an_increase_in_decay_curve_dynamic_range)).
This is the direct precedent for a **cheap, standard-practice answer to I4**: rather
than a model-free bound with a fudge factor, assume the tail is exponential (same
assumption the room's own physics already rests on) and integrate it in closed form.
Later work (e.g. a 2016 JASA paper on "automated estimation of the truncation of
room impulse response by applying a nonlinear decay model," and a related
ScienceDirect paper on "correction of room impulse response truncation based on a
nonlinear decay model") extends this to non-single-exponential tails, but the
one-line single-exponential version is what nearly everyone actually ships.

### 2.5 Comparative software study

"Calculating Reverberation Time from Impulse Responses: A Comparison of Software
Implementations," *Acoustics Australia* (Springer, 2016) — directly on point:
different commercial/academic packages implement Lundeby/Chu/Hirata-family
algorithms slightly differently and get **different EDT/T30 numbers from the same
input IR**
([publisher page — full text behind a login wall this session](https://rd.springer.com/article/10.1007/s40857-016-0055-6)).
This matters for scope: even the "rigorous, standard" methods do not converge on a
single ground truth; there is no zero-disagreement reference implementation to chase.

---

## 3. Practice of simulation tools

### 3.1 ODEON

- EDT: best-fit linear regression **0 to −10 dB** on the backward-integrated curve,
  ×6 extrapolation to 60 dB — same convention as the standard
  ([odeon.dk/learn/articles/room-acoustics](https://odeon.dk/learn/articles/room-acoustics/)).
- ODEON **automatically locates a "truncation time"** on the simulated echogram —
  described as "the part of the response not affected by the background noise" — and
  presents both the raw and truncation-corrected decay curves
  ([same source](https://odeon.dk/learn/articles/room-acoustics/)). For a simulation
  (no real measurement noise), ODEON's "background noise" is effectively the
  statistical/ray-count floor of its own late-part ray-tracing stage — i.e. ODEON
  applies a Lundeby/Hirata-style truncation step even to its own synthetic output,
  it does not treat "simulated" as exempt from the truncation problem.
- ODEON is explicitly a **hybrid** method: image-source for early reflections up to
  a transition order (default 3), ray tracing for the late part — the late part is
  exactly where the Monte-Carlo noise problem (I5) and the truncation problem (I4)
  both live, same as SPPS's particle tracer.
- Christensen & Rindel's 2013 paper (cited in §1.1) is ODEON's own team explicitly
  asking "can we simulate [ISO 3382] parameters, can we measure them" — i.e. the
  authors of one of the most-used simulation tools in the field publicly treat
  simulated-vs-measured EDT as an open, non-trivial question, not a solved one.

### 3.2 CATT-Acoustic / TUCT

The most directly useful finding in this whole search, because CATT's release notes
say the quiet part out loud:

> "The EDT minimum regression coefficient has been lowered from 0.95 to 0.70,
> reflecting that the 0 to −10 dB decay is seldom linear, especially not close to a
> source or on axis of a directive source, which is a fundamental problem with the
> EDT measure."
> — [CATT-Acoustic v9.1 release notes](https://www.catt.se/CATT-Acoustic_v9.1_news.pdf)
> (search-indexed text; the raw PDF could not be parsed by the fetch tool this
> session, so this is quoted as surfaced by the search index, not independently
> re-verified against the PDF bytes)

Two other points from the same source:

- CATT's echogram/decay-curve feature does exactly the standard thing: full-length
  Schroeder integration with optional EDT/T15/T20/T30 and a **linear regression
  overlay**, exported with its data.
- **"In all EDT, T-15, T-20 and T-30 predictions, the length of the echogram
  corresponds to at least 10 dB below the end of the range."** This is CATT's
  answer to **I6**: a *minimum decay range requirement* (run/record long enough that
  you have ≥10 dB of curve beyond the bottom of whatever window you're fitting), not
  a computed bound on the error from not having it. If the requirement isn't met,
  the implication (standard practice everywhere, not CATT-specific) is: don't report
  the number, rather than report it with an inflated uncertainty band.

CATT's own admission that EDT is "seldom linear" and a "fundamental problem" is a
useful sanity check on scope: the field's own tools do not treat EDT as a quantity
that deserves a tight, provable bound — they treat it as a fundamentally noisy,
somewhat unreliable single-number descriptor, and manage that by loosening the
acceptance criterion (r² down to 0.70), not by tightening the uncertainty apparatus
around it.

### 3.3 RAVEN (Institute of Technical Acoustics, RWTH Aachen)

RAVEN combines deterministic image-source modelling for early reflections with a
stochastic (ray-tracing/particle-based) method for the late reverberant field, and
explicitly claims physically accurate auralization including stochastic late-field
effects
([virtualacoustics.org/RAVEN overview](https://www.virtualacoustics.org/RAVEN/),
[Schroeder et al. 2011 conference paper](http://www2.users.ak.tu-berlin.de/akgroup/ak_pub/seacen/2011/Schroeder_2011b_P2_RAVEN_A_Real_Time_Framework.pdf)).
This session could not locate RAVEN's own published description of its energy-
histogram noise-floor or EDT-extraction procedure specifically (no page found
documenting a Lundeby-style step for RAVEN's histogram) — **flagged as a genuine gap
in this pass**, not filled in from memory. RAVEN's architectural similarity to SPPS
(deterministic early part + stochastic late part, both eventually needing a decay
curve out of a possibly-noisy late-field estimate) makes it the closest published
analogue to this project's problem, so this gap is worth closing in a follow-up
search specifically against RWTH Aachen ITA's own publication list if this matters
later.

### 3.4 EASE / AURA (AFMG)

AURA's ray-tracing engine computes "accurate, full-length echograms" and explicitly
states it implements **"calculations according to ISO 3382 for Early Decay Time
... along with Reverberation Time"** and several other Part-1 parameters
([AFMG AURA 4 Module page](https://www.afmg.eu/en/aura-4-module)). No published
detail was found in this session on AURA's specific noise-compensation/truncation
algorithm for its own simulated echograms (i.e. whether it runs a Lundeby-style
step on its own ray-traced output before fitting EDT) — **flagged as a gap**, same
caveat as RAVEN.

### 3.5 Treble

- Treble is a genuinely hybrid **wave-based (low frequency) + geometrical-acoustics
  (high frequency)** solver, with a documented default transition frequency of 4×
  the Schroeder frequency
  ([Transition Frequency docs](https://docs.treble.tech/Transitionfrequency)).
- Its own acoustic-parameters page states EDT/T20/T30 with the **same dB ranges as
  the standard** (0/−10, −5/−25, −5/−35, all ×-extrapolated to 60 dB), computed "using
  the energy decay curve by Schroeder's backward integration and via a least-square
  linear fitting" — no mention of a bound, a range, or an uncertainty figure attached
  to the reported value
  ([docs.treble.tech/acoustic-parameters/reverberation-time](https://docs.treble.tech/acoustic-parameters/reverberation-time)).
  One earlier search snippet paraphrased Treble's EDT window as "−1 dB to −11 dB" —
  this conflicts with the 0/−10 dB wording found directly on the page above; **flagged
  as an unresolved discrepancy** between two summarizations of Treble's docs rather
  than a confirmed fact about Treble either way.
- Termination of a geometrical-acoustics run in Treble is controlled by either an
  **energy-decay-threshold** (stop once a receiver has dropped by N dB) or a fixed
  **signal-length**, and "Survey mode" picks an IR length from an initial
  reverberation-time estimate based on the assigned materials
  ([Simulation settings docs](https://docs.treble.tech/user-guide/simulations/simulation_settings)).
  This is Treble's answer to "how long do you run before truncating" (**I4**): a
  practical stopping rule tied to an estimated (not measured) decay, not a rigorous
  receiver-capture bound. No documented noise-floor or minimum-dynamic-range check
  was found on this page for the resulting histogram.

### 3.6 Pachyderm

Open-source (Grasshopper/Rhino) ray-tracing tool. Builds a logarithmic
Energy-Time-Curve (ETC) histogram per receiver and computes EDT "like reverberation
time T30 but in the level range from −0.1 dB to −10 dB"
([search-indexed description](https://www.researchgate.net/publication/272591807_Pachyderm_Acoustical_Simulation_Towards_Open-Source_Sound_Analysis)).
As with RAVEN/AURA, no published detail was found in this session on Pachyderm's
own noise/truncation handling for the synthesized ETC — its documentation footprint
in the general web index is thin compared to ODEON/CATT. **Flagged as a gap.**

### 3.7 Cross-tool disagreement is large and well documented

Bork's ISO-affiliated round-robin studies (2nd and 3rd Round Robin on Room
Acoustical Computer Simulation, plus the more recent "A round robin on room
acoustical simulation and auralization," *JASA* 145(4), 2019) had **multiple
commercial/academic simulation programs** (ODEON, CATT and others among them,
per the round-robin design) predict T30/EDT/C80/D50 for the *same* modelled rooms
against measurement, and found EDT specifically **overestimated at low frequencies
and underestimated at high frequencies** relative to measurement, with accuracy
"increasing from 1979 to 2020" but still frequency-dependent and tool-dependent
([search-indexed summaries of the round-robin literature](https://www.researchgate.net/publication/233569251_A_Comparison_of_Room_Simulation_Software_-_The_2nd_Round_Robin_on_Room_Acoustical_Computer_Simulation),
[JASA 2019 round robin](https://pubs.aip.org/asa/jasa/article/145/4/2746/848382/A-round-robin-on-room-acoustical-simulation-and)).
**Scope-setting fact:** even between fully-developed, decades-old commercial tools,
EDT predictions for the same room disagree with each other and with measurement by
amounts that dwarf any sub-percent engineering bound a single tool could compute
internally. A model-free bound on one tool's own numerical error is answering a much
smaller question than the one the field actually struggles with.

### 3.8 Upstream I-Simpa — read directly from source, not from memory

This project's own decision log (row 4) already states: *"Upstream's EDT fits from
emission and has no check (`projet_calculation.cpp:68-219`)."* Read directly from
`B:\repos\I-Simpa-upstream\src\isimpa\data_manager\projet_calculation.cpp` (upstream
checkout, tag `v1.4.0_snapshot_14_01_2026`, read-only) this session, confirmed line
by line:

- `MakeSchroederArray` builds the Schroeder curve by summing the raw per-bin energy
  array **backward from the last time bin to the first**, converting each partial
  sum to dB as it goes (`10*log10(sum*p0)`). No noise subtraction, no truncation
  step — the entire recorded histogram, end to end, is integrated every time.
- `GetTimeRange(fromdB=0, toDB, ...)` walks the resulting dB curve **forward from
  `timeTable[0]`** (the very first time bin, i.e. the emission instant, not the
  direct-sound arrival) looking for where the level has dropped by `fromdB` (0 for
  EDT) from `row_db[0]`. Because `fromdB == 0`, the very first sample already
  satisfies `|row_db[0] - row_db[0]| >= 0`, so **the fit start is unconditionally the
  first time bin of the whole simulation**, whatever silence or dead time sits before
  the direct sound arrives at that receiver. This is I1's failure mode, verified in
  code, not asserted from the decision log alone: any pre-arrival bins that happen
  to sit at the (flat) top-of-curve level get pulled into the least-squares window
  used for the regression, biasing the fitted slope shallow (over-estimating EDT)
  whenever the direct sound arrives meaningfully later than t=0.
- `ComputeLinearRegression` does an unweighted ordinary least-squares fit over
  whatever range `GetTimeRange` returned — **no r² or residual check of any kind is
  computed or reported**, so a degenerate window (I6: near-empty, non-monotonic, all
  noise) silently produces a number with no flag.
- `Compute_TR_Param` always extrapolates via `-60/regRes.a` regardless of whether the
  fit window was 10 dB (EDT) or something else, matching the ISO ×6/×3/×2 convention
  in spirit but with none of ISO's or CATT's guardrails around it.
- Upstream's own **user documentation**
  (`B:\repos\I-Simpa-upstream\Docs\room_acoustics_parameters.rst`) states the ISO
  3382-1 definition correctly in prose ("obtained... by calculating the slope of the
  background integration decay curve... between 0 and −10 dB") but says nothing
  about noise, truncation, direct-sound alignment, minimum decay range, or
  uncertainty — the documentation is descriptive of the ISO formula only, not
  prescriptive about any of I1–I7.

**Conclusion on "how would I-Simpa's actual code resolve these issues": it doesn't.**
There is no hidden sophistication to recover from upstream — decision #4's
characterization is accurate and now doubly confirmed by direct code reading. The
useful question isn't "what would upstream do," it's "what does the rest of the
field do that upstream skipped," which is answered in §2–§3 above: Lundeby-style
truncation-at-the-noise-floor, a direct-sound-anchored fit origin, and a
regression-quality/minimum-range gate — not a model-free bound.

---

## 4. What SPPS itself actually does (read from source this session, for I1–I5)

All read from `B:\repos\I-Simpa-upstream\src\spps\` (read-only upstream checkout;
not modified).

- **Temporal meshing is a fixed time step, not continuous time** — SPPS's own
  documentation states this as a deliberate design choice: *"the monitoring of sound
  particles is carried out in constant time steps, and not in continuous time"*
  (`Docs/code_SPPS_principle.rst`, "Temporal meshing" section). This is upstream's
  own framing of exactly **I3** — it is a named, acknowledged discretisation, not an
  oversight, and upstream does not claim to resolve where inside a bin energy
  actually arrived.
- **Receiver energy is binned by integer time-step index at the moment of
  collision**, not at the continuous crossing time: in
  `src/spps/input_output/reportmanager.cpp`, `ParticuleFreeTranslation` computes the
  exact sphere-crossing length (`Lintersect`, via `mu1`/`mu2` from `RaySphere`) but
  still deposits the resulting energy into `currentRecp->energy_sum[freq][pasCourant]`
  — a single integer bin index for the whole time step. This is the literal,
  in-code shape of **I3**: sub-bin timing information is computed internally for the
  crossing-length weighting, then discarded when writing to the histogram.
- **The particle kill threshold ("`trans_epsilon`") is a general energy floor, not
  a transmission-only parameter**, despite its XML attribute name. `sppsNantes.cpp:75`
  computes `energie_epsilon = energie * 10^(-trans_epsilon/10)` from the
  `trans_epsilon` config value (read in `core_configuration.cpp:65`), and
  `CalculationCore.cpp` checks `energie <= energie_epsilon` as a kill condition after
  **atmospheric absorption** (line ~58), **obstruction/clutter absorption** (line
  ~142), and **surface absorption** (line ~305) alike — not only at transmission
  events. This confirms the project's own description of **I4**'s second term ("particles
  SPPS kills below `-trans_epsilon` dB") is an accurate reading of the actual kill
  condition, and also clarifies its bound is trivial by construction: a particle is
  only ever killed once its own energy has already fallen `trans_epsilon` dB below
  its value at the last absorption event, so the energy lost to this specific
  mechanism is bounded above by that same particle's pre-kill energy — already a
  small quantity relative to the room's total injected energy on any run with enough
  steps for the room to have absorbed most of it through ordinary wall losses first.
- SPPS offers a **direct-field-only mode** ("Active calculation of direct field
  only," `Docs/code_configuration_run.rst`) specifically so the direct arrival can be
  computed and inspected in isolation from the reverberant field — this is upstream's
  own tool for exactly locating the direct-sound arrival deterministically (source
  position, receiver position, sound speed ⇒ arrival bin index is just
  `distance / c / dt`), which is the practical fix for I1 in this codebase: the
  direct-sound bin does not need to be *detected* the way a measurement has to detect
  it in a noisy recorded IR — it is already known exactly from geometry.

---

## 5. Issue-by-issue: what standard practice actually does

**I1 — fit origin.** Every standard and tool surveyed anchors 0 dB (and the start of
the fit) at the **direct-sound arrival**, not at emission/recording start (§1.1,
§3.8). For a simulated histogram this is *not* a detection problem the way it is for
a measured IR — the arrival bin is `round(distance / c / dt)`, computed once from
geometry already known to the solver. Pre-arrival bins are simply excluded from the
regression window; nothing else is needed. This is strictly simpler than a
model-free bound: it's an index computation, not an uncertainty term.

**I2 — direct-sound weighting.** No standard or tool documentation found gives the
direct-sound bin any special weight in the regression beyond being the first
(dominant) term summed into the backward integral. Causality already guarantees
nothing precedes it, so "0 dB" (total remaining energy) is automatically anchored at
that bin without a separate rule.

**I3 — time-step discretisation.** Universally accepted as a fixed, negligible-by-
choice discretisation once the bin width is small relative to the parameter of
interest (SPPS's own docs frame it exactly this way, §4). No tool or standard
computes a term for "where in the bin the energy really sat" — it is not treated as
a source of quantifiable error, only as a resolution setting to be chosen small
enough (which this project's own step-cost investigation, decision-log row 11,
already measured empirically: at 1 ms the discretisation is on the order of the
Monte-Carlo noise floor anyway).

**I4 — unrecorded late energy.** Standard practice (Lundeby/Hirata, §2.3–2.4) is:
find where the decay curve meets the noise floor, truncate there, and add back the
missing tail analytically **assuming single-exponential decay** — the same
assumption the room's Sabine/Eyring reverberation model already rests on, not a new
one. For SPPS specifically: (a) run-end truncation should get a Hirata-style
closed-form exponential-tail correction fit to the last clean segment of the curve,
not a model-free bound; (b) `trans_epsilon`-killed energy is already bounded trivially
by construction (§4) and does not need its own receiver-capture model.

**I5 — particle/receiver noise.** Standard practice does not compute a statistical
confidence interval from ray/particle count at all (§3.7 — no tool found does this).
It relies on backward (Schroeder) integration's built-in ensemble-averaging property
(§2.1) plus a documented minimum particle/ray count, and otherwise pushes "is this
curve too noisy" onto the regression-quality gate (I6). There is no field precedent
for propagating per-bin Monte-Carlo noise into an EDT uncertainty figure.

**I6 — degenerate curves.** Standard practice is a **binary accept/refuse gate**, not
a graduated bound: check the regression quality (CATT relaxed this to r² ≥ 0.70
specifically because raw 0/−10 dB fits are "seldom linear," §3.2) and the available
decay range (CATT: echogram must run ≥10 dB past the bottom of the fit window). Fail
either check ⇒ omit/flag the number. No tool computes a *width* for how wrong the
number might be in degenerate cases — it declines to report one at all.

**I7 — display.** No surveyed tool (ODEON, CATT, Treble, EASE/AURA, Pachyderm)
reports a range alongside EDT; all report one number per band/receiver, with
regression quality as the only attached confidence signal, and ISO 3382-1's own
Annex A JND (5 %, itself unvalidated for EDT, §1.1) as the nominal precision target.
The one dedicated perceptual study of EDT's actual JND found it closer to **18 %**
for broadband symphonic-hall conditions (Del Solar Dorrego & Vigeant, *JASA* 151(1),
80–94, 2022 —
[PubMed](https://pubmed.ncbi.nlm.nih.gov/35105034/),
[publisher abstract](https://pubs.aip.org/asa/jasa/article-abstract/151/1/80/2838316)).
Burhan's row-9 ruling ("show the range always") is a real, deliberate departure from
every surveyed tool's practice — that is his call to keep, not a gap in the research —
but the finding worth handing back is: field practice doesn't need a range because
(a) its noise-handling method (Lundeby/Hirata) produces a corrected point value, not
a bound, and (b) the actual perceptual JND is wide enough that sub-percent
engineering precision is inaudible. If a range is kept, the cheap, standards-grounded
way to build one is "value with vs. without the Hirata tail correction" (a natural,
physically motivated pair), not a bound scaled by an arbitrary safety factor.

---

## 6. What this implies about the current apparatus, stated once, plainly

The project's model-free band (decision rows 7, 12, 14, 15) set out to prove a
rigorous, assumption-free bound on EDT error. Nothing in ISO 3382-1/2, ISO 18233,
Lundeby, Chu, Hirata, or any of ODEON/CATT/RAVEN/EASE-AURA/Treble/Pachyderm attempts
that. All of them fit a straight line over a fixed dB window anchored at the
direct-sound arrival, apply an exponential-tail assumption for the part they can't
record, gate on regression quality and available decay range, and report one number.
The x1000 safety factor and the `run_too_short` refusal exist specifically because a
*model-free* bound on late-field energy is impossible without a diffuse-field
assumption (row 14's own text says as much) — which means the "model-free" premise
itself, not the arithmetic built on top of it, is what produced the 32.7 %-of-real-
EDTs refusal rate and the still-uncalibrated ×1000 guess. Standard practice sidesteps
that whole problem by making the same single-exponential assumption up front
(Hirata/Lundeby) that the room's own reverberation model already makes, and paying
for that assumption with a truncate-and-flag gate instead of a load-bearing safety
factor.

---

## 7. Caveats on this pass

- WebFetch could not extract text from any of the PDF sources cited above (ISO
  3382-1/2 preview PDFs, the CATT release notes, the arXiv lecture-notes PDF, the
  ODEON ISRA-2013 paper) — the tool returned "binary/encoded, cannot parse" for
  every PDF tried. Every claim sourced to a PDF above is therefore taken from
  **WebSearch's own indexed/snippeted text of that PDF**, not from this session
  directly reading the document. Where a claim is quoted verbatim in this file, that
  quote is the search index's rendering, flagged as such at first use (see §1.1,
  §3.2). Reading the actual clause numbers of ISO 3382-1/2 would need either a paid
  standards portal login or an OCR pass on the saved PDFs (saved locally by the fetch
  tool under the session's own tool-results cache, not part of this project).
- RAVEN, EASE/AURA and Pachyderm's own specific noise/truncation algorithms for their
  synthesized histograms were **not found** in this pass (§3.3, §3.4, §3.6) — flagged
  as gaps rather than papered over. ODEON and CATT are the two tools with genuinely
  strong documentation coverage found this session; the others are covered only for
  their EDT range convention and general architecture.
- The "Treble EDT window is −1 to −11 dB" claim (§3.5) is an unresolved discrepancy
  between two different summarizations of Treble's own docs pages, not a confirmed
  fact — flagged, not resolved.
- No item in this file is presented from the assistant's own unaided memory without
  a citation, except where explicitly marked "(memory)" or "search-indexed,
  unverified against source bytes" inline.

## References (direct links)

- ISO 3382-1:2009 preview PDF: https://cdn.standards.iteh.ai/samples/40979/b0e9f87f3d9f4df6b18f767e3439bfb6/ISO-3382-1-2009.pdf
- ISO 3382-2:2008 catalogue entry: https://www.iso.org/obp/ui/#iso:std:iso:3382:-2:en
- ISO 18233:2006 catalogue entry: https://www.iso.org/standard/40408.html
- ODEON, "The ISO 3382 parameters: Can we simulate them? Can we measure them?" (ISRA 2013): https://www.odeon.dk/pdf/ISRA2013_Paper_The%20ISO%203382%20parameters_Can%20we%20simulate%20them_Can%20we%20measure%20them_24July2013.pdf
- ODEON room acoustics article: https://odeon.dk/learn/articles/room-acoustics/
- CATT-Acoustic v9.1 release notes: https://www.catt.se/CATT-Acoustic_v9.1_news.pdf
- CATT TUCT overview: https://www.catt.se/TUCT/TUCToverview.html
- RAVEN overview (Virtual Acoustics / RWTH Aachen): https://www.virtualacoustics.org/RAVEN/
- RAVEN 2011 conference paper: http://www2.users.ak.tu-berlin.de/akgroup/ak_pub/seacen/2011/Schroeder_2011b_P2_RAVEN_A_Real_Time_Framework.pdf
- AFMG AURA 4 Module: https://www.afmg.eu/en/aura-4-module
- Treble reverberation-time docs: https://docs.treble.tech/acoustic-parameters/reverberation-time
- Treble simulation-settings docs: https://docs.treble.tech/user-guide/simulations/simulation_settings
- Treble transition-frequency docs: https://docs.treble.tech/Transitionfrequency
- Pachyderm description: https://www.researchgate.net/publication/272591807_Pachyderm_Acoustical_Simulation_Towards_Open-Source_Sound_Analysis
- Lundeby, Vigran, Bietz, Vorländer 1995 (RWTH Aachen ITA page): https://www.akustik.rwth-aachen.de/cms/institut-fuer-hoertechnik-und-akustik/forschung/publikationen/~dwoc/publikationen-einzelansicht/?file=192049&lidx=1
- Chu 1978 / software-comparison discussion: https://rd.springer.com/article/10.1007/s40857-016-0055-6
- Hirata correction, modern restatement: https://www.researchgate.net/publication/289228157_A_modified_approach_to_the_backward_integration_leading_to_an_increase_in_decay_curve_dynamic_range
- Schroeder 1965 (ADS abstract): https://ui.adsabs.harvard.edu/abs/1965ASAJ...37S1187S/abstract
- Schroeder-integration review: https://pubs.aip.org/asa/jasa/article/157/2/R3/3333468/Schroeder-integration-for-sound-energy-decay
- ISO 3382-1 EDT JND (5%, unvalidated): https://www.researchgate.net/publication/328248817_A_STUDY_OF_THE_JUST_NOTICEABLE_DIFFERENCE_OF_EARLY_DECAY_TIME_EDT
- Measured EDT JND ≈18% (Del Solar Dorrego & Vigeant 2022): https://pubmed.ncbi.nlm.nih.gov/35105034/ , https://pubs.aip.org/asa/jasa/article-abstract/151/1/80/2838316
- Bork 2nd Round Robin: https://www.researchgate.net/publication/233569251_A_Comparison_of_Room_Simulation_Software_-_The_2nd_Round_Robin_on_Room_Acoustical_Computer_Simulation
- JASA 2019 round robin on simulation & auralization: https://pubs.aip.org/asa/jasa/article/145/4/2746/848382/A-round-robin-on-room-acoustical-simulation-and
- I-Simpa upstream source read directly (read-only, this session):
  `B:\repos\I-Simpa-upstream\src\isimpa\data_manager\projet_calculation.cpp`,
  `B:\repos\I-Simpa-upstream\Docs\room_acoustics_parameters.rst`,
  `B:\repos\I-Simpa-upstream\Docs\code_SPPS_principle.rst`,
  `B:\repos\I-Simpa-upstream\src\spps\CalculationCore.cpp`,
  `B:\repos\I-Simpa-upstream\src\spps\sppsNantes.cpp`,
  `B:\repos\I-Simpa-upstream\src\spps\data_manager\core_configuration.cpp`,
  `B:\repos\I-Simpa-upstream\src\spps\input_output\reportmanager.cpp`,
  `B:\repos\I-Simpa-upstream\Docs\code_configuration_run.rst`
- This project's own decision log (context, not a source on EDT practice):
  `B:\repos\I-Simpa_Night_Mode\docs\decision-log.md`
