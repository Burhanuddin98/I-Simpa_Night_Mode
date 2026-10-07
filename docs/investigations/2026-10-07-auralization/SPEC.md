# C5: auralization in v1 (spec, 2026-10-07 22:40)

The plan is `docs/investigations/2026-10-06-auralization/PLAN.md` (the method, where it goes, the order of work):
read it first; this spec adds Burhan's answers and the done-when. Auralization is v1 on his word, 10-06 16:51
(decision 67): "auralization is another feature we need in version 1 itself".

**Burhan's answers, 2026-10-07 22:39 (AskUserQuestion, verbatim choices):**
- Sample rate: **"48 kHz (Recommended)"**.
- Per source or summed: **"Both (Recommended)"**: one IR per source-receiver pair (what ISO 3382 defines) and one
  summed over the enabled sources.
- Band edges: **"The solver's own bands"**: exactly the bands the project ran (octaves if it ran octaves, thirds
  if thirds), edges at the geometric midpoints between neighbouring centre frequencies, the outer edges a half
  band out (half an octave or a sixth of one, matching the set). NOT the nominal IEC edges.
- Anechoic material: **"Ship a few clips (Recommended)"**: a handful of public-domain or CC0 anechoic clips
  (speech, a solo instrument) bundled with the examples, plus "open your own WAV".
Log these as decision 75 in `docs/decision-log.md`, with the words above.

## Done when

1. **Core** (`crates/simpa-core`, an `auralize` module): band filters on the run's own bands (above), envelope
   interpolation from the echogram's time step to 48 kHz, synthesis (filtered noise times the square root of the
   band's energy envelope, summed over bands), a WAV writer (32-bit float and 24-bit PCM, mono), FFT convolution.
   Deterministic: the noise seed is fixed and recorded, so the same run gives the same WAV bit for bit. Tests,
   each with its numbers in the report:
   - a single-band, single-step echogram gives one band-limited click at the right time;
   - the synthesised IR's band energies per time step equal the echogram's within the filters' leakage (state
     the leakage you measured and the bound you test);
   - the Schroeder decay of the synthesised IR gives the echogram's EDT and T30 within the JND (5 %) per band,
     on CR4's receivers (a real run, not a synthetic one), octave AND third-octave band sets;
   - filters on the solver's bands sum flat across the band set (the reconstruction ripple, measured, stated).
2. **CLI:** `simpa auralize <run> --receiver <name> [--source <name>|--summed] [--anechoic <wav>] --out <wav>`
   (or the CLI's idiom). An anechoic file at another rate is resampled to 48 kHz (say how, test it).
3. **App, Results step, beside the response window:** "Impulse response (synthesised)" with Play and Save for the
   chosen receiver, per source or summed; "Auralize" with the bundled clips in a list and "Open your own WAV…";
   Play/Stop through Web Audio and Save as WAV. The words on screen say what this is: "synthesised from the energy
   echogram: the room's decay and spectrum per band, with a random fine structure; not a measured or wave-based
   impulse response" (shorter is fine; the response window's wording is the precedent). Peak-normalised playback
   with a level meter; never clip silently.
4. **The clips:** 2 to 4 short anechoic clips (speech and an instrument at least), each with a provenance file
   beside it: source URL, author, licence (public domain or CC0 only; CC-BY only if the attribution is shown in
   the app's About and in THIRD_PARTY_NOTICES), the date fetched and its sha256. A clip whose licence page you
   cannot read and quote does not ship. If none can be verified, ship none, say so in the report, and keep
   "Open your own WAV…" as the path. Keep the total under 5 MB (mono, 48 kHz, short).
5. **e2e:** a new spec (`m13.aural.e2e.ts` or the specs' numbering) on CR4 results: the IR panel shows, Save
   writes a WAV whose header says 48 kHz and whose length matches the run's duration, Play starts and stops,
   Auralize with a bundled clip produces audio, the wording is on screen. Run it with
   `tools/gates/m11.ps1 -Only e2e -Spec m13.aural -TargetDir C:\tmp\nm-target-aural`, and once at the end
   together with the m12 specs and m13.blank (the m12 builder's list) so nothing on the Results step broke.
6. Scope ledger row and backlog rows for what is deferred (binaural, moving sources, the Showerb export).

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\aural` (branch `aural`, from `gpu` 2e6345a). Cargo
  target `C:\tmp\nm-target-aural`. Solvers `C:\tmp\nm-solvers-gpu`; `SIMPA_TETGEN160` =
  `B:\repos\I-Simpa_Night_Mode\target\solvers\build\src\tetgen\Release\tetgen.exe`; `SIMPA_UPSTREAM` =
  `B:\repos\I-Simpa-upstream`. A CR4 project to bed on: copy `B:\repos\I-Simpa_Night_Mode\.out\a5\cr4-1k\` and
  re-run it with all its octave bands (and a third-octave copy via `simpa reband`), into `.out\aural\`.
- **Disk rule (Burhan 15:27/15:34): regenerable output is deleted, not archived.** Delete your run staging and
  test builds when you finish; keep the WAVs the report cites (the IRs Burhan will listen to) and the receipts.
  Check C: free space before every `cargo test` (60 GB at 22:40); stop under 20 GB.
- `cargo fmt --all` and `cargo clippy -D warnings` clean before you finish (today's gates failed on exactly this).
- It is after 22:00: no deletions outside your own outputs; never delete a repo, branch or anything you did not
  create.
- Your app's DevTools port 9301; check that the pid you drive is the one you started.
- Commit on `aural` as you go, messages on the why, no Claude attribution, no Co-Authored-By. Do not push.
  `npm run typecheck` and `npm test` pass at the end.

## Report

`docs/investigations/2026-10-07-auralization/REPORT.md` on `aural` (if the harness refuses your write, put the full
report in your final message). Lead with the done-when per item; the test numbers (leakage, ripple, EDT/T30 against
the echogram per band and set); the WAVs to listen to (paths); the clips with their licences; what is deferred.
