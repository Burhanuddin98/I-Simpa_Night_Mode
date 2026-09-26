# Decision log

Every decision that shapes v1, who made it, why, and whether to revisit it in v1.1. Burhan's words are quoted
verbatim where he gave them. Any decision marked **Jarvis** is a technical call made under Burhan's division
of labour (2026-09-27 01:05): technical calls are made by Jarvis, written down here with the reason, and can be
overruled by Burhan at any time; calls that change what users see or where the product goes are Burhan's.

To change a decision, add a new row that supersedes the old one and keep the old one here, so the history stays readable.

| # | Date | Decision | By | Why (evidence) | Revisit in v1.1? |
|---|---|---|---|---|---|
| 1 | 2026-09-24 23:14 | **M8 reference:** Kuttruff's corrected Eyring with γ² computed from the room's geometry (5 %), with the independent transport as the tight cross-check; plain Eyring reported only. | Burhan | Plain Eyring is off by +1 to +11 % for Lambert walls. SPPS matches the transport within 0.04 % (`docs/params.md`). | Maybe: add the exact sphere reference (backlog item 1) and gate the transport tightly (about 0.5 %). |
| 2 | 2026-09-25 19:29 | Follow-up spec D1-D11 accepted (`docs/investigations/2026-09-25-followup-spec/`). | Burhan ("i accept") | The spec's evidence. **W1G (D1) was later withdrawn** after Z3 broke it (`docs/investigations/2026-09-25-z3-w1g-hunt/`). | W1G is replaced by the band (row 7). |
| 3 | 2026-09-25 23:13 | Fix the stale docs, then merge pre-M8 into rebuild. | Burhan | Every gate passed at `3f76711`; merged as `df7d8e5`. | No |
| 4 | 2026-09-25 23:33 | **The EDT method must be "FAR BETTER THAN THE I SIMPA EDT METHOD".** | Burhan (verbatim) | Upstream's EDT fits from emission and has no check (`projet_calculation.cpp:68-219`). | No, it stands. |
| 5 | 2026-09-25 23:53 | **The v1 filter:** v1 takes only what stops a wrong number reaching a user, or a feature Burhan asked for; everything else goes to `docs/v1.1-backlog.md`. | Burhan | Scope control. | It governs v1.1 itself. |
| 6 | 2026-09-26 00:41 | **Personal project:** Burhan's decisions are final; nothing is routed for approval. | Burhan (committed `eb9500f`) | | No |
| 7 | 2026-09-26 | **Early parameters (EDT, Ts, C50, C80, D50) use the model-free band**: a guaranteed range, no curve-shape assumption, exact per-step air, anchored at the direct arrival. | Burhan (row 4) / design | 0 wrong among 24,866 accepted EDTs; upstream is wrong beyond 2.5 % on 62 % of the same rows (`docs/investigations/2026-09-26-edt-band/`). | Tighten the bands with the direct-sound energy and the arrival-bin branch. |
| 8 | 2026-09-26 02:33 | **GPU particle tracer** is a v2 contender, after v1.1. | Burhan ("WE LEAGE THIS AS A SERIOUS V2 CONTENDER, ONCE V 1.1 IS DONE AND READY") | Tight bands need fine steps and many particles. | v2 |
| 9 | 2026-09-27 00:10 | **EDT display: "Show the range always".** EDT is never hidden for width; it is always shown with its guaranteed range, and wide ranges are flagged. Values outside the method's premises are still refused with a reason. | Burhan (verbatim) | His choice among 2.5 %, 0.5 % and always showing. | Maybe: whether to add a user-set tolerance. |
| 10 | 2026-09-27 00:13 | **Split M8:** M8a gates T30 first; M8b gates EDT once the band is in Rust. | Jarvis (Burhan: "no idea what to select here either") | Tests exactly the same things and drops nothing; the T30 physics proof starts sooner. | Reversible at any time. |
| 11 | 2026-09-27 01:05 | **Default step for acoustic-parameter runs: 1 ms** (the preset was 2 ms). | Jarvis (Burhan: "needs more research"; research done) | Tutorial 1 and 6x10x3 at 150k particles: 1.8 s a run at 2 ms, 2.1-2.2 s at 1 ms. At 1 ms the EDT band (about 1-1.5 %) roughly equals the per-run noise (about 1.5-2.7 %); finer steps buy nothing at this particle count (`docs/investigations/2026-09-27-step-cost/`, `EVAL.md`). | Yes: with more particles, or the GPU tracer, 0.5 ms may pay off. |
| 12 | 2026-09-27 01:05 | **Gate 2, the bound on unrecorded energy: a run-length rule.** The run must be long enough that the energy after its end, and the energy from particles SPPS kills at -50 dB, is provably negligible; otherwise the band widens or refuses. Never assume 0, and never fill it in with a log-linear model. | Jarvis | It is the only option that keeps the guarantee model-free. The critic's gate 2 (`workflow-reports.json`). | Maybe: trans_epsilon default, and the run-length cost. |
| 13 | 2026-09-27 01:05 | **Division of labour:** technical calls are Jarvis's, written here and overrulable; calls about what users see or where the product goes are Burhan's. | Burhan ("yeah", to the proposal) | Too many technical decisions were piling on Burhan. | Session order unless Burhan says "from now on". |
