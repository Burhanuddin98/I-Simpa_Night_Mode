# Why EDT v2.1 failed H3 in energetic mode (VERDICT-2): a blocked receiver, not the mode

2026-10-02. Existing round-2 data only. Scripts and tables are in `B:\data\m8b-t20\edt-energetic\` (`tab.py`,
`curves.py`, `anchor.py`).

## The receiver

"G4 far" is G4 R007 at (12.5, 10.5, 1.2), 13.67 m from the source, in the L's upper arm. The straight line to the
source leaves the room, so the receiver has **no line of sight**. Its design T60 is 0.64 s and its truth EDT is about
1.0 s.

## The failing rows

All 9 failing rows are energetic, 1 and 2 ms, at 125 Hz to 4 kHz, with errors of -5.3 % to -8.9 % against the truth.
The ranges shown are tighter than the bias. Example: 500 Hz, seed 3101, 0.945 s, range 0.907-0.985, truth 1.011.

## Not an energetic bias

Mean EDT of the 9 tested runs per band:

| Band | Energetic | Random | Truth |
|---|---|---|---|
| 125 Hz | 0.973 | 0.973 | 1.006 |
| 250 Hz | 0.953 | 0.975 | 1.006 |
| 500 Hz | 0.951 | 0.998 | 1.011 |
| 1 kHz | 0.952 | 0.940 | 0.985 |
| 2 kHz | 0.936 | 0.937 | 0.968 |
| 4 kHz | 0.883 | 0.876 | 0.917 |

- **Both modes read 3-5 % low.** Energetic minus random averages -0.008 s.
- **Random escaped only through its noise:** its rows scatter 0.79-1.09 and were marked `wide`.
- **The Schroeder curves have the same shape in both modes.**

## The mechanism

- **The two anchors differ.** For a blocked receiver, `frozen2/method.py:75-80` puts 0 dB and the fit start at
  **that run's own first non-zero bin**. That bin comes at 0.048-0.058 s, against the straight-line arrival of
  0.0398 s, and it varies from run to run because the diffracted and reflected arrival there is sparse. The truth
  (`truth.py:174-177`) anchors at the first non-zero bin of the sum of 4 truth runs, which is earlier.
- **Late anchor, steep fit.** A tested run anchored late skips the decay's flat first dB (about 1 dB in 50 ms) and
  fits a steeper slope.
- **Moving the anchor fixes it.** Anchoring each run at the earliest arrival over all runs brings the energetic means
  within 0.5-3 % (mostly under 2 %). The 500 Hz error goes from -6 % to -1 %.

## Consequence (decisions 38 and 39)

- **v1:** EDT at a receiver with no line of sight is marked not validated, in both modes. Energetic EDT is otherwise
  validated: its H1-H6 pass except this subgroup (to be confirmed in the build).
- **v1.1:** an anchor for blocked receivers (geometric or diffraction-path arrival), as a v2.2 of the method with its
  own round. Backlog 62.
- **Not checked:** G3's blocked receiver, and the particle floor (`trans_epsilon`).
