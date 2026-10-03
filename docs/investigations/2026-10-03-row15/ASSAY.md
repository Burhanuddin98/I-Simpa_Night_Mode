# ASSAY row15-groups (f617b1f..6460fee)
Tests run: regroup_faces 9/9 ok; geometry_import_proj receiver filter 4/4 ok (needs SIMPA_UPSTREAM=B:/repos/I-Simpa-upstream).

1. Placeholder: SAFE. bridge.rs edit_apply exempts only MATERIAL_PLACEHOLDER on a group id absent from the pre-edit project. run_blockers counts groups_assigned, so the new group yields MATERIALS_UNASSIGNED (bridge test a_new_group_with_the_placeholder_is_accepted_and_blocks_the_run; existing group set to placeholder still refused).
2. RegroupFaces: all checks precede mutation; inverse = Batch(override clears, receiver/zone replace, SetGeometry old, RemoveSurfaceGroup); tests assert exact undo. Receivers/zones: all-or-none else Split. Variants: carried override or Split/placeholder. No material change under any variant found.
   LOW: placeholder group under a variant whose carried override is a real material is not flagged when that variant is active (placeholders() checks active variant only); materials still correct.
3. M37: stack walk keeps all receivers in order with " / " path; config.xml equal flat vs grouped for SPPS+TCR; field optional/skip-if-None. Tests only cover tutorial 1 (2 receivers, 1 per folder): no folder with several receivers or sibling-after-nested case.
   LOW: a recepteurp with no eid, formerly imported, is now refused (RECEIVER_GROUP_MALFORMED). Upstream skips it. Deliberate, documented, but a behaviour change.
   LOW: refusal message in receiver_elements holds a run of literal spaces mid-sentence.
4. E2E: outcome-based. Regroup checks name, material==plaster id, exact face list, emptied group stays, undo depth +1, row selected, material set on new group alone, two undos give exact text. Folders: project JSON, DOM folder per row, filter by folder, flat control.
   Gaps (would pass if broken): placeholder/blocker path, straddle refusal, grouped-receivers config.xml not exercised in e2e (covered only in cargo).
5. Scope: gate m11.ps1 spec + command count 39, solver-contract row: all in plan. Nothing outside found.
