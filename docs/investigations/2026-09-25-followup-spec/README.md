# The pre-M8 follow-up spec (2026-09-25)

`followup-spec.md` is the spec for the follow-up piece that runs after the pre-M8 merge and before
M8: the W1G early-reverberation (EDT/Ts) check with guards G1-G6, the seed-batch noise rule for
energetic T20/T30 (SB-1 to SB-9, pre-registered in `receipts/REGISTERED.txt`), the refusal
wording, and the confirmation runs (Z1, Z3, Z4 in Python; R1-R7 after the pre-M8 gates).

**Burhan accepted decisions D1-D11 as recommended on 2026-09-25 at 19:29 ("i accept").** Nothing
ships until Z3 (a fresh hill-climbing skeptic), Z4 (Rust-port parity) and the runs pass.

Receipts: the evaluation scripts and their outputs are in `receipts/`. The row pickles they read
(about 39 MB) stay in `target/agents/followup-design/spec/`, with the attack sets under
`target/agents/followup-design/` and `target/agents/t30-edt-diagnosis/`.
