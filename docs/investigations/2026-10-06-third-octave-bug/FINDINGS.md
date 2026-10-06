# Third-octave preset not used by SPPS (2026-10-06 06:12)

Burhan, verbatim: "i selected third octaves and yet it doesnt use that setting and just goes with default for SPPS,
begin an investigation".

## Evidence, from the files, before any theory

- `.out\ui\cr4\CR4.simpa` on disk: `bands.kind = octave`, 6 bands (125 to 4000 Hz), `spps.bands_computed` 6 of 6.
  Last saved 05:55:38, which is the start of the 05:55 run (the app saves on Run).
- Its last three runs, 05:54 (cancelled), 05:55 (OK) and 06:01 (OK): every `solve/config.xml` carries the six octave
  bands. The run of 06:01 did not re-save the file, so nothing was dirty between 05:55 and 06:01.
- `.out\ui\cr4-third\CR4-third.simpa` (built by script at 05:13): `third_octave`, 18 bands; its run's config carries
  the 18. So the solver path honours third-octave bands when the project holds them.

So the selection never reached the project. The question is why.

## The code path (`app/ui/src/features/simulate/SettingsEditor.tsx`, `BandsEditor`)

Before the fix: the "Band preset" dropdown set a local `pick` only. The bands changed only after the user then
pressed "Change bands…" and, in the confirmation that opened, "Change bands", which fires `actions.reband` ->
`backend.editReband` -> `accept(state)` (`actions.ts:286-293`). The m11 settings spec drives exactly that sequence
and checks `project().bands.kind === 'third_octave'` (`m11.settings.e2e.ts:225-231`), so the mechanism works when
all three steps are taken.

The trap: after step one, the dropdown read "Third-octave 100 Hz to 5 kHz" while the band row, the summary line and
the project stayed on the six octave bands. A control that shows a value not in effect. A user who picks and moves
on believes the bands are set; Run then exports the octaves.

What was not established: whether Burhan stopped after the dropdown, or confirmed and had the change refused. A
refusal would have shown an issue line under the preset (`refusals.get(presetKey)`) and a FAIL line in the Console;
the Console is not persisted, so this cannot be checked after the fact. Either way the dropdown lied, so the fix
stands; if he did confirm and saw a refusal, that is a second finding to chase with the Console open.

## The fix (same session)

Picking a preset opens the confirmation at once (no separate "Change bands…" button); Cancel or an outside change
puts the dropdown back to the bands in effect (`useEffect` on the current preset while no confirmation is open).
The spec's extra click on `[data-part="reband"]` is removed; the confirmation, apply and the `third_octave` check
are unchanged. Unit suite green; the spec re-runs at the next gate.

## Still owed

- Burhan's answer: did he press "Change bands…" and confirm? If yes: reproduce with the Console open and read
  the refusal.
- A guard against the class of trap: a check that every dropdown's displayed value is the project's value
  (the band preset was the only dropdown with a pending local choice; `AcousticsPane`'s selects and the reflection
  law select commit on change).
