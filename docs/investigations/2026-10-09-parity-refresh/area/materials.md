# Parity re-check: 2.2 Materials, sources and receivers (M1-M44), as of R = parity-ro @ 95ec062, 2026-10-09

Paths: R = `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\parity-ro`. `ui/` = R\app\ui\src, `tauri/` = R\app\src-tauri\src, `core/` = R\crates\simpa-core\src. Status read from code, not docs. Matrix numbers `plan.md:133-134` have moved: the deferral text is now `docs/rebuild-plan.md:143-144` ("material library with Odeon and CATT import, receiver grids, source groups and intensity vectors"). `ui-parity-backlog.md:62` is the old-GUI list ("material categories").

## 1. Table

| ID | Feature | Imp. | 09-25 | Now | Receipt | Note |
|---|---|---|---|---|---|---|
| M1 | Reference material library | common | Deferred | Ready | ui/features/materials/LibraryMenu.tsx:1-118 ("+ From library"); tauri/commands.rs:716, runs.rs:729 | 11 of upstream's 12 (Default excluded, REFERENCE_MATERIALS[1..]); adds a copy, in-project entries disabled. Not a browsable read-only tree, same function |
| M2 | User materials: create, rename, delete, edit | core | Planned | Ready | MaterialsGrid.tsx:233 (rename), :268/:699 (add), :275/:709 (delete), ops.ts addMaterial/removeMaterial | No duplicate/clone verb |
| M3 | Material folders and categories | common | Deferred | Deferred | rebuild-plan.md:143-144; ui-parity-backlog.md:62; flat list (core/schema/model.rs:424) | Unchanged |
| M4 | Per-band absorption and scattering | core | Planned | Ready | MaterialsGrid.tsx:77-78 (tabs Absorption/Scattering/Transmission), :589 grid | Band set per project |
| M5 | Per-band reflection law (7 laws) | common | Core only | Partial | materials/law.ts:17-31, MaterialsGrid.tsx:764-797 (LawCell) | One law per material, all bands. Per-band law shown "per band", editable only as set-all (backlog 20). Semi-diffuse chosen in GUI blocks Run at export (backlog 22) |
| M6 | Per-band transmission switch and loss | common | Design only | Ready | MaterialsGrid.tsx:207, :739-746; materials/model.ts withTransmission, transmissionNote; decision 73(d) | Empty band = no transmission; note when alpha=0 or tau>alpha |
| M7 | Material consistency rules | core | Planned | Ready | core/validate/project.rs:97-203; materials/inline.tsx, MaterialsGrid cell issues (data-cell-issue); MaterialsPanel.tsx:105-108 | Reports, not silent rewrite |
| M8 | Side of material effect | niche | Core only | Core only | core/schema/model.rs:445-446; ui/ops.ts:145 (new = double_sided true) | No UI control |
| M9 | Material metadata | niche | Absent | Absent | no fields (model.rs:424-454); no UI | |
| M10 | Assign material to a surface group | core | Planned | Ready | MaterialsPanel.tsx:61-70 (pick, multi-group batch), :152-176 radio list | Variant-aware; unassigned flag :107-117 |
| M11 | Spreadsheet editing of band tables | common | Planned | Ready | materials/tsv.ts, paste.ts, fill.ts; MaterialsGrid.tsx:290 copyText, :455-500 copy/paste listeners, :719 fill-row | Materials only; no grid for source spectra or zone bands |
| M12 | Copy and paste elements across projects | common | Absent | Absent | no clipboard of elements in ui/ (only TSV of band cells) | |
| M13 | CATT-Acoustic material import | common | Deferred | Deferred | rebuild-plan.md:143-144 | CATT materials inside a `.proj` import (proj.rs:111-116) |
| M14 | Odeon .li8 import | common | Deferred | Deferred | rebuild-plan.md:143-144 | |
| M15 | Unassigned faces | core | Ready | Ready | MaterialsPanel.tsx:107-117 (UNASSIGNED flag); core validator `material_unassigned` | Stricter than upstream |
| M16 | Reference spectra | niche | Absent | Ready | tauri/commands.rs:722, runs.rs:746; core/geometry/import/appconst.rs:53 (8 spectra); ui/chrome/EmissionEditor.tsx:~130 spectrum select | Offered as the source's Spectrum list (white, pink, 6 traffic); no standalone library view |
| M17 | User spectrum library | common | Absent | Absent | EmissionEditor: only "Typed per band" per source (emission.ts:96-97) | |
| M18 | dB(A) entry for spectra and power | common | Absent | Absent | grep dB(A)/dBA in chrome/EmissionEditor.tsx, emission.ts: none | Results show dB(A) (backlog 70), source entry does not |
| M19 | Source position | core | Planned | Ready | SourcesPanel.tsx:220-246 (x,y,z fields), :380 + Source, Viewport.tsx:80 place-source tool, engine.ts:2208 | Outside-room refused (validator) |
| M20 | Source sound power and spectrum shape | core | Design only | Ready | EmissionEditor.tsx:1-218 (global dB, per-band dB, spectrum); decision 73(c) | |
| M21 | Attenuation on a spectrum | niche | Absent | Absent | none | Folded into custom levels on import |
| M22 | Analytic directivities | common | Design only | Ready | emission.ts DIRECTIVITIES (codes 0-4), EmissionEditor.tsx directivity select + direction x/y/z | TCR still treats all as omni (unverified, not re-read) |
| M23 | Balloon directivity sources | niche | Core only | Core only | EmissionEditor.tsx comment/disabled option "Measured balloon (file)"; model.rs:477-480 | Kept and shown, cannot be chosen or loaded in GUI; refused on spps-gpu (backlog 103) |
| M24 | Directivity database | niche | Absent | Absent | none | |
| M25 | Source time delay | niche | Core only | Core only | model.rs:551-552; ops.ts:191 (delay_s 0) | No field |
| M26 | Enable or disable a source | common | Core only | Ready | ScenePanel.tsx:48-60 SourceSwitch, SourcesPanel.tsx:300-305, actions.setSourceEnabled | SOURCE_NONE refusal inline |
| M27 | Source groups | common | Deferred | Core only | model.rs:553-560 (Source.group kept, imported proj.rs:920); not shown anywhere in ScenePanel/SourcesPanel | Display and create/edit both missing (rebuild-plan.md:143-144) |
| M28 | Source-group actions (enable/rotate/translate) | common | Absent | Absent | none | |
| M29 | Line of sources | niche | Absent | Absent | none | |
| M30 | Source and receiver markers in 3D | common | Planned | Partial | viewport/sprites.ts:35, engine.ts:213/243 DOM labels, ScenePanel.tsx:340 marker | Point + name label. Missing: per-item colour, show-name toggle, direction arrow |
| M31 | Descriptions on sources/receivers | niche | Absent | Absent | none | |
| M32 | Point receiver: position, label, orientation | core | Planned | Partial | SourcesPanel.tsx:150-246 (name, x,y,z, issues RECEIVER_OUTSIDE/LABEL_UNSAFE); ops.ts:179 orientation fixed [1,0,0] | Orientation not editable (omni receiver; matters only to oriented metrics, unverified) |
| M33 | Receiver orientation point | niche | Absent | Absent | none | |
| M34 | Link receiver to a source | niche | Absent | Absent | none | |
| M35 | Receiver background noise | niche | Core only | Core only | model.rs:583-586; ops.ts:179 (null) | No field |
| M36 | Receiver directivity | niche | Not porting | Not porting | matrix reasoning (solver ignores it) | |
| M37 | Point-receiver groups | common | Absent | Partial | sceneModel.ts:224 receiverFolder; ScenePanel.tsx:207, :374-380 (read-only group tag + filter); decision 45; model.rs:589-600 | Imported groups kept and shown; no create/edit |
| M38 | Receiver grid generator | common | Deferred | Deferred | rebuild-plan.md:143-144 | Only "+ Ear-height plane" (a map, not a grid of receivers) |
| M39 | Receiver-group actions | common | Absent | Absent | none | |
| M40 | Scene surface receiver | common | Core only | Core only | model.rs:626-631 (Scene{groups}); ui/bindings only; no ui/ code constructs kind 'scene' | Results can draw a map if a file brings one; GUI cannot create |
| M41 | Cutting-plane receiver | common | Core only | Partial | PlanesSection.tsx:1-177 (+ Ear-height plane, height above floor, resolution, Remove); ops.ts:204 newCuttingPlane; core cutting_plane_invalid | Level planes only; no free corners A/B/C; tilted shown not edited |
| M42 | Cutting-plane display before a run | common | Absent | Ready | engine.ts:1865-1900 updatePlanes (outline for every enabled plane; cell grid off the Results step), planeOutlines hook :780 | No colour/label options (see missed) |
| M43 | Enable/disable planes, surface receivers, zones | common | Core only | Core only | model.rs:604,667; engine.ts:1876 honours `enabled`; PlanesSection.tsx has no toggle | |
| M44 | "Clean" a face list | niche | Absent | Absent | none | |

## 2. Counts

Rows checked: 44 (M1-M44). Unverified: 0 (two notes marked "unverified" are side remarks, not row statuses).

Status now: READY 14 | PARTIAL 5 | CORE-ONLY 7 | DEFERRED 4 | ABSENT 13 | NOT-PORTING 1 | UNVERIFIED 0.

Importance x status now:

| Importance | n | Ready | Partial | Core only | Deferred | Absent | Not porting |
|---|---|---|---|---|---|---|---|
| core | 8 | 7 (M2,M4,M7,M10,M15,M19,M20) | 1 (M32) | 0 | 0 | 0 | 0 |
| common | 22 | 6 (M1,M6,M11,M22,M26,M42) | 4 (M5,M30,M37,M41) | 3 (M27,M40,M43) | 4 (M3,M13,M14,M38) | 5 (M12,M17,M18,M28,M39) | 0 |
| niche | 14 | 1 (M16) | 0 | 4 (M8,M23,M25,M35) | 0 | 8 (M9,M21,M24,M29,M31,M33,M34,M44) | 1 (M36) |

Moves since 09-25: 12 rows changed status (M1, M5, M6, M16, M20, M22, M26, M30, M37, M41, M42 plus M27 Deferred to Core only). Core-importance gap left: M32 (receiver orientation not editable).

## 3. Features the matrix missed (upstream receipts, relative to src/isimpa/data_manager/)

1. Material display colour editable per material (`tree_scene/e_scene_bdd_materiaux_rendermateriau.h:56`, "mat_color"). Night Mode stores `color` (model.rs:429) but the grid shows a read-only swatch (MaterialsGrid.tsx:637, MaterialsPanel.tsx swatch). Common.
2. Source and receiver render properties: colour and "Show name" (`tree_scene/e_scene_sources_source_rendu.h:46,48`; `e_scene_recepteursp_recepteur_rendu.h:46,48`); same for fitting zones (`e_scene_encombrements_encombrement_rendu.h:57,59`). Matrix M30 notes "per-item colour, show-name" only in a cell, no row. Niche.
3. Cutting-plane display options: show grid, show vertex names, cut colour (`tree_scene/e_scene_recepteurss_recepteurcoupe_rendu.h:54-56`). Niche.
4. Source spectrum editor rows: per-band dB(A) column and a Global row carrying dB, dB(A), Attenuation and Lw, linked so editing Lw or Attenuation moves the other (`generic_element/e_property_freq.cpp:181-190, :89-110`; `e_data_row_bandefreq.h:81-82`; `e_data_row_ext_bandefreq.h:67-68`). M18/M21 name dB(A) and attenuation but not the linked Global row or the per-band dB(A) readout. Common.
5. Descriptions on surface receivers and cutting planes (`tree_scene/e_scene_recepteurss_recepteur_proprietes.h:50`; `e_scene_recepteurss_recepteurcoupe_proprietes.h:50`); M31 covers sources and point receivers only. Niche.
6. A source's own description field exists too (`e_scene_sources_source_properties.h:58`) and a spectrum referenced by `idspectre` stays live-linked to the user-spectrum library entry (`e_property_freq.cpp:151-175`): editing the library spectrum changes every source using it. Part of M17, not stated. Niche.

Count: 6 (items 1, 4 and 5 are new rows; 2, 3 and 6 are sub-features of existing rows).

## 4. In the current app, absent or weaker upstream

- Source emission editor with inline validator messages and exact one-step undo (EmissionEditor.tsx); upstream edits through a property grid with no checks.
- Unlimited exact undo/redo on every material, source, receiver and plane edit (edit_undo/edit_redo, tauri/commands.rs).
- Variant-aware material assignment: a pick is the variant's override (MaterialsPanel.tsx:92-96); upstream has no variants.
- Multi-group material pick as one undo step (MaterialsPanel.tsx:61-70).
- Materials usage count ("used by n"), natural sort, TSV paste with row fill (MaterialsGrid.tsx:131, :642).
- Transmission rule explained in the cell (tau vs alpha clamp, alpha=0 off), which upstream applies silently (materials/model.ts transmissionNote).
- Place source/receiver by clicking the floor with ear-height snap, and RECEIVER_OUTSIDE / SOURCE outside-room refusals before launch (actions.placeAt, engine.ts:2208).
- "+ Ear-height plane" one-click sound map (PlanesSection.tsx; upstream's plane needs its three corners typed).
- Run-size forecast on planes (PlanesSection imports runSize cube text and REFUSE_GB/WARN_GB), a cost warning upstream lacks.
- Source on/off with SOURCE_NONE guard fixing upstream's all-disabled bug (M26 note).
- Reference spectra offered on any band set, including 1/1-octave (appconst.rs `shape_on`).
