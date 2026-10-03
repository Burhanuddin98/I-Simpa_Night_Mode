# The "wow" list: what a client sees and thinks "they know what they are doing" (2026-10-03)

Burhan, 22:19: "MAKE A LIST OF ALL THE COOL FEATURES THAT A CLIENT LOOKS AT AND GOES WOW THEY KNOW WHAT THEY ARE DOING
BECAUSE I SEE ALL THESE COLORFUL DIAGRAMS IN THE SOFTWARE, THAT'S THE KIND OF THING YOU NEED TO BUILD FOR ME".

One rule holds for all of it (gate (b), decision 47): a colour that stands for an acoustic **parameter** (C80, STI,
T30...) is only drawn once that parameter has passed a check for that kind of picture. Colours of **energy or level**
come straight from the solver's own files and are checked bit-exact (gate (c)), so they can be drawn now. That is why
the parameter maps sit lower on this list: they are the most impressive and the least checked.

Ids are the parity audit's (`docs/investigations/2026-09-25-parity-audit/parity-matrix.md`).

## Already in the app (M12, today)

| What the client sees | Made from |
|---|---|
| The hall's surfaces coloured by sound energy, animated through the decay, per band, with a legend (R38, R39, R41, R50) | SPPS surface receivers (`.csbin`) |
| Before/after: the difference between two designs coloured on the surfaces (R52) | two runs' `.csbin` |
| Sound particles flying through the hall and dying out (R53, R55) | `.pbin` |
| Decay curves per receiver; reverberation per band against the DIN 18041 target band | echograms (`.recp`) |
| Every receiver's parameters in one table, each with its range | the report |

## Quick wins: the data already exists, only the picture is missing

| # | What the client sees | Why it lands | Size |
|---|---|---|---|
| W1 | **A coloured sound-level floor plan at ear height** (cutting-plane map, M41): the classic room-acoustics picture, animated | It is what every consultant's report shows; the model and solver files already support planes, the UI to place one is missing | M |
| W2 | **Sound filling the hall** (cumulative map animation, R40): colour builds up from the source, then fades | Shows the physics in one glance | S |
| W3 | **Particle trails** (R54): streaks instead of dots, so reflections paths show | The most "simulation-looking" image I-Simpa can make | S-M |
| W4 | **Area by level** (R69): "92 % of the audience area is above 75 dB", with a coloured histogram | A number a client can repeat to their boss | S |
| W5 | **Smooth colouring, iso-contour lines, fixed colour range, value probe on hover** (R46-R48, R51) | Makes every map look professional and lets the client point at a seat | S each |
| W6 | **Spectrum chart per receiver** (R9): level per band as coloured bars | Familiar to every client | S |
| W7 | **Echogram with direct sound and early reflections marked** (R8 extended) | Shows *why* a seat sounds as it does | S-M |
| W8 | **Side-by-side before/after views** of two designs, synchronised camera | Sells a treatment proposal | M |
| W9 | **Export: 3D view as an image, animations as video, charts as images** (R56, R63) | Goes straight into the client's report | S-M |

## Bigger builds

| # | What the client sees | What it needs first | Size |
|---|---|---|---|
| W10 | **Parameter maps on the audience area: clarity C80, definition D50, speech STI, T30** (R42, R44) | A per-surface check against references, as the point receivers had (backlog 76) | L |
| W11 | **Where the sound comes from**: arrows/hedgehog at receivers (R33) | the solver's intensity output read and checked | M |
| W12 | **A one-click report** with these pictures and tables (R68) | the pictures above | M-L |
| W13 | **Waterfall plot** of the decay per band at a seat | echograms per band (exist) | S-M |

## Not on the list, honestly

- **"Listen at R1" (auralisation).** SPPS computes energy, not a pressure impulse response, so there is nothing true to
  play. Faking one would be the opposite of "they know what they are doing" (decision 47, MQ5).
- **Measured-room comparison** (V2-9, BRAS) is not a picture but it is the strongest client argument of all: "our
  simulation of this hall matched its measurement within X". It is backlog V2-9.

## Recommendation (Jarvis)

Build first, in this order: **W1** (ear-height floor plan), **W2** (sound filling the hall), **W3** (particle trails),
**W5** (smooth colour, contours, hover probe), **W9** (export). Together they turn the Elmia demo into the pictures a
client expects, all from data that is already checked. W10 (parameter maps) after, with its own check.
