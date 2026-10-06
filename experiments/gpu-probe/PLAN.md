# GPU probe: the number decision 51 asked for (2026-10-06 19:10)

Burhan, 19:00: "OKAY DUDE WHAT ABOUT A GPU CAPABLE VERSION"; 19:10: "PROCEED WITH 3" (the probe first).
Decision 8 (2026-09-26) and decision 51 (2026-10-05 19:00) hold: the GPU particle tracer is v2, after
M12 and M13, and "no GPU speed-up is forecast until a kernel is timed". This is that timing. It is an
experiment in its own directory, imports nothing from the codebase, and changes no solver.

## The question

How many particle-steps a second does a straightforward CUDA kernel trace in a shoebox, against SPPS
on the same box at equal particles and step? One number per machine, labelled realised, with the box,
the particle count, the step, the duration and the GPU named.

## The box (the committed fixture `tests/fixtures/rooms/seats_box.simpa`)

6 x 10 x 3 m; ceiling 30 % absorbing, floor 10 %, walls 20 %, specular, no scattering; source at
(3, 5, 1.8); point receivers Seat (1, 1, 1.8) and Seat2 (3, 7, 1.8), radius 0.31 m. One band (the
materials are flat across bands). Energetic method: a particle keeps its direction law (specular) and
loses energy by (1 - alpha) at each reflection; it is dropped below 10^-5 of its start (SPPS's
`extinction_exponent` 5). No air absorption, no fittings, no transmission, no surface receivers, so
both arms do the same work: transport and point-receiver crossings.

## The two arms

- **SPPS**: `box.simpa` written from the fixture with those settings, `simpa run --solver spps`,
  the verified build (`solvers/manifest.json`). SPPS threads per band only and a fixed seed turns
  threading off (decision 51), so this is one CPU thread. Its rate is read from the run's wall
  time, `nominal particle-steps = particles x steps`.
- **CUDA** (`box_tracer.cu`): one thread per particle, the whole run in registers (position,
  direction, energy), each step advancing c dt with specular reflection on the six planes, the
  receiver crossing as the segment's length inside the sphere, accumulated per step with atomics.
  Initial directions from a counter-based hash of the particle id (uniform on the sphere), no RNG
  state. The same function compiled for the host runs a smaller particle count single-threaded, so
  the GPU's histograms are checked against the CPU's bit for bit (same arithmetic, same order per
  particle; the atomics' order differs, so the check is to 1e-5 relative).

## What counts as the result

- `rate_gpu` and `rate_spps` in particle-steps a second, `speedup = rate_gpu / rate_spps`. The GPU
  number carries the kernel time (cudaEvent) and the wall time including the copies.
- A sanity check, not a bed: T20 of each receiver's decay (Schroeder-integrated echogram) from both
  arms agree within the Monte-Carlo noise. If they do not, the probe's physics differs from SPPS's
  and the speed number stands alone, said so.
- Grace (RTX 5070, CUDA 13.2) tonight; Zeph (RTX 2060 Max-Q, CUDA 11.8) when Burhan runs
  `build.ps1` and `run.ps1` there.

## What this is not

Not the tracer. Not SPPS's receiver accounting (SPPS weights crossings its own way; the slope is
compared, not the level). Not a bed: BED-style receipts come with the tracer itself (decision 51's
road: the per-particle CPU restructure with the counter-based RNG, then the same walk in CUDA).
