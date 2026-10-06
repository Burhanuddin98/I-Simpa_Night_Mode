# Auralization in v1: plan (2026-10-06 16:55, draft for Burhan's word)

Burhan, 16:51, verbatim: "can you run it with the maxium density of octave bands and show me the resulting IR,
also can we have a super detailed spectrogram using the methods we have without all the issues, i mean
auralization is another feature we need in version 1 itself?"

My reading: auralization is v1 scope from now (his filter, CLAUDE.md: v1 takes a feature he asked for). This
file is the plan; nothing is built yet. The 27-band run and the IR and spectrogram pictures are
`docs/investigations/2026-10-06-third-octave-bug/IR-27.md` when they land.

## What SPPS gives, and what it does not

SPPS counts energy. A point receiver's `.recp` series is Pa² per time step (1 ms) in each band: an **energy
echogram**, 27 bands wide at the densest preset (third octaves, 50 Hz to 20 kHz). It has no phase and no
waveform, and the response window says so (`response.ts`: "an energy echogram, not a pressure impulse
response"). The spectral detail is the band set: 27 values across frequency per millisecond. A finer frequency
axis means more bands, and the cost grows linearly in time and in the maps' memory (the data bound in
`runSize.ts` is per band); 1/12 octaves would be about four times the 27-band run.

## The method (the one every energy-based room simulator uses)

A pressure impulse response is synthesised from the energy echogram, per band, then summed:

1. For each band, a white-noise sequence at the output rate (44.1 or 48 kHz) is band-pass filtered to the
   band (a zero-phase or linear-phase filter at the nominal edges).
2. The filtered noise is multiplied by the square root of the band's energy envelope, interpolated from the
   1 ms steps to the sample rate (the envelope is energy; the waveform's amplitude is its root).
3. The bands are summed into one waveform. Direct sound and early reflections keep their timing from the
   echogram's steps; the noise carries the band's spectrum and a random fine structure, as a real room's late
   field does.
4. The result is normalised and written as a WAV (the IR itself), and convolved with an anechoic recording
   for the auralization (another WAV, playable in the app through Web Audio).

This is what ODEON, CATT and I-Simpa's own auralization do with the same kind of echogram; it is honest about
what it is (a statistically equivalent waveform, not the room's actual pressure response), and the app must
say so where it plays it, as the response window already does for the picture.

## Where it goes

- **Core (`crates/simpa-core`):** `auralize` module: band filters, envelope interpolation, synthesis, WAV
  writer, convolution (FFT, the IR is 10 s at 48 kHz = 480,000 samples; a 30 s anechoic sample is a few
  million). Pure functions, tested against: a single-band single-step echogram gives one filtered click;
  the synthesised IR's band energies per step equal the echogram's within the filter's leakage; Schroeder
  decay of the synthesised IR gives the echogram's T30 within the JND.
- **CLI:** `simpa auralize <run> --receiver <MP> [--source <S>] [--anechoic <wav>] --out <wav>`.
- **App:** on the Results step, beside the response window: "Impulse response (synthesised)" with Play and
  Save, and "Auralize" taking an anechoic WAV from disk. Words on the button: what it is and is not.
- **Michael's engine:** Showerb (ElAntyr) auralizes from ROM's mode arrays through `dsp_core`; the ecosystem
  flow (CLAUDE.md) runs ROM -> Showerb. For this app's v1 the synthesis above lives in the core so the app
  ships alone; exporting the echogram to Showerb's input is a v1.1 row if Michael wants it.

## Order of work, after Burhan's word

1. Core synthesis and WAV writer with the three tests above (half a day of build, a bed on CR4's receivers).
2. CLI command, used to produce the first IR WAVs from the 27-band run for him to hear.
3. App: Play and Save on the Results step; then Auralize with an anechoic file.
4. Decision row and scope ledger entry; backlog rows for what is deferred (binaural, moving sources, the
   Showerb export).

## Open for him

- Sample rate (48 kHz proposed) and whether the IR is written per source or summed (both, proposed).
- Whether the band filters are the nominal third-octave edges or the solver's own band definitions (nominal
  proposed; the same as the response window draws).
- Anechoic material: he supplies, or a few public-domain clips ship with the examples.
