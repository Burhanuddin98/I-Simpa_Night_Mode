# FDTD probe, phase 2 pre-registration (2026-10-07 21:08, written before any phase-2 run)

Burhan, 2026-10-07 21:06: "nice okay, i believe that we should test more and find the RIGHT way to integrate it"

Phase 1 (`RESULT.md`): on CR2, PFFDTD with its default boundary (octave alpha read as random-incidence) decays
too slowly, T30 +25 % at 125 Hz and +2 % at 250 Hz on the 10-pair means. The question now is whether that is a
property of the boundary mapping everywhere, or of the modal regime of a small room, and which rule for turning
Night Mode's material data into an FDTD boundary transfers to rooms it was not chosen on.

## Rooms and runs

Night Mode's shipped examples, measured BRAS dodecahedron RIRs, the same driver (`probe.py`, `PROBE_ROOM`):

| room | V (m3) | f_Schroeder ~ 2000 sqrt(T/V) | pairs | grid |
|---|---|---|---|---|
| CR1 (coupled, door angle 3) | 225 | ~ 170 Hz | 4 (LS1-2 x MP3-4) | fmax 400 Hz, 10 PPW, 3.0 s |
| CR2 (phase 1 runs reused) | 147 | ~ 195 Hz | 10 | fmax 1000 Hz, 10 PPW, 2.5 s |
| CR3 (chamber music hall) | 3335 | ~ 50 Hz | 10 | fmax 400 Hz, 10 PPW, 3.0 s |
| CR4 (auditorium) | 8735 | ~ 26 Hz | 10 | fmax 400 Hz, 10 PPW, 3.0 s |

Judged bands: 125 and 250 Hz octaves (63 Hz reported only). Same metrics, filters and targets as `PREREG.md`
(T30, EDT <= 5 %; C80 <= 1 dB; mean over pairs). Surfaces with air on both sides are absorbing on both.

## Hypotheses, stated before the runs

- **H-sys**: the default boundary decays too slowly in every room: FDTD T30 mean > measured mean at 125 and
  250 Hz in CR1, CR3 and CR4.
- **H-modal**: the over-long decay is a below-Schroeder effect: in CR3 and CR4 (f_S << 125 Hz) the default
  boundary meets the T30 target at 125 and 250 Hz, while CR1 (f_S ~ 170 Hz) shows CR2's bias at 125 Hz.

These are mutually exclusive on CR3/CR4. Either outcome sets the next test:

- H-sys holds: derive one per-band boundary factor on CR2 alone, apply it unchanged to CR1/CR3/CR4
  (a forecast with transfer history "fitted on CR2"), judged against the same targets and against Eyring.
- H-modal holds: the default boundary is the integration rule above the Schroeder frequency; below it the gap
  is physics the boundary data does not carry (extended reaction, GA-tuned alpha), and integration reports it.
- Neither: the bias is room-specific; no single mapping rule; integration ships FDTD for spectra and IRs only.

## Addendum 21:39, before any CR4 output exists (CR4 GPU run started 21:38)

Forecast for CR4 under the default boundary, transfer history CR2 + CR3 (Paris arm, FDTD/measured T30 ratio
1.25 / 1.02 on CR2 and 1.25 / 1.25 on CR3 at 125 / 250 Hz; CR1 left out as a coupled room):
**FDTD T30 / measured T30 = 1.15 to 1.35 at 125 Hz and at 250 Hz.** Outside that range in either band, the
over-long decay is not one systematic factor and the transfer test below starts from that fact.
