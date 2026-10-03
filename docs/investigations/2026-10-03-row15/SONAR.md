# SONAR map: face regrouping (G19) + grouped receivers (M37). Read-only recon, 2026-10-03, rebuild @ 1a76770

## 1. Parity rows
- G19 (docs/investigations/2026-09-25-parity-audit/parity-matrix.md:133): Send selected faces to a new group | right-click selection, new group | core | Deferred | backlog:62 "surface group from selection". No regroup op (ops.rs:124-127 has SetGeometry only) | M | blocks G30. Verdict row ~400: Before v1, one invertible regroup op, G20 reuses it.
- M37 (parity-matrix.md:204): Point-receiver groups | nested folders | common | Absent | tree_scene/e_scene_recepteursp.h:193; projet.cpp:1437 | No group field; import-proj refuses nested receiver groups (proj.rs:910-916 [STALE, now ~921-928]). Verdict row ~448: before v1 import side only; flatten them, or keep a group path as sources do. GUI groups v1.x. M39 needs M37.
- docs/scope.md:15 row 21/22: both are "row 22"'s own piece.

## 2. Our importer (crates/simpa-core/src/geometry/import/proj.rs)
- Face -> group: face_group (Vec<Option<usize>>) from sgroupes `gr` .finfo lists, ~482-527 (finfo_faces fn ~1708). Scene surface receivers face_receiver ~537-620.
- SurfaceGroup built per key (material group, receiver, zones) ~731-790; SurfaceGroup{id: GroupId(ids.uuid("surface group", i)), name, material}; Face.group = key_group(f) ~783-822. Unassigned faces -> "(no surface group)" group ~747-779.
- Point receivers: `let mut point_receivers` ~920; opt_child(data,"recepteursp"), loops element_kids; any child whose tag != "recepteurp" -> ImportError::unsupported(FMT_XML, "<tag> in the point receivers") ~921-928 (this is the nested-group refusal; a nested `recepteursp` group hits it). read_point_receiver fn ~2400. Pins upstream id via report.upstream_ids UpstreamKind::PointReceiver.
- Sources contrast: source_elements() ~1051 walks nested source groups, returns (Node, group-path String); `source.group = Some(path)` ~908; codes::SOURCE_GROUP_MALFORMED ~242.
- Dropped: receiver groups (refused outright); .finfo "aire" param; group names beyond material-group name.
- Model (crates/simpa-core/src/schema/model.rs): Face{vertices:[u32;3], group:GroupId} :201; SurfaceGroup{id,name,material} :251 (deny_unknown_fields); Source.group: Option<String> :560; PointReceiver{id,name,position,orientation,background_noise,solver_id} :575 - NO group field.
- Ops (schema/ops.rs): SetGeometry :125 ("Every face's group must exist"), AddSurfaceGroup{index,group} :128 (apply ~766, inverse ~798), RemoveSurfaceGroup{id} :132, SetGroupMaterial{group,material} :135, AddPointReceiver :173, ReplacePointReceiver :180, MovePointReceiver :183. No move-faces/regroup op, no group rename for surface groups (Rename :115 is generic).

## 3. Upstream .proj format
- .proj = zip; instance2/projet_config.xml (+ N.finfo, sceneMesh.bin). Example: B:\repos\I-Simpa-upstream\src\isimpa\resources\doc\tutorial\tutorial 1\tutorial_1.proj
- Face groups: `<sgroupes><gr name="Ceiling" eid="5" wxid="1463" facesFile="3141.finfo"><p name="idmat" .. value="21"/></gr>`; face list in the .finfo (count + (i32 mesh group, i32 face) pairs).
- Receivers: `<recepteursp name="Punctual receivers" eid="7" wxid="1472"><recepteurp name="Receiver 1" eid="8" wxid="1473"><position ..><p name="x" value="1"/>..`. A receiver group = nested `<recepteursp>` inside `<recepteursp>` (e_scene_recepteursp.h:66-69,98-105: AppendFils new E_Scene_Recepteursp + SetUserGroup() :132). No real tutorial .proj found with nested receiver groups in tutorial 1 (only checked tutorial 1 XML; grep other .proj for nested not done).

## 4. Tests / fixtures
- crates/simpa-core/tests/geometry_import_proj.rs (gate_b tutorial2 elmia :153, gate_c tutorial1 :241, tutorial3 source groups :521, industrial :684, every_upstream_project_imports_or_is_refused_by_name :724, each_refusal_names_its_reason :903, helpers tutorial3_edited :1254, edited_once :504). Also tests/geometry_import_{fuzz,rooms}.rs, mesh_mbin_parity.rs, parity_inputs.rs, crates/simpa/tests/parity_tutorials.rs.
- tests/fixtures/rooms/ (tutorial1_box.simpa, elmia_corrected.simpa from tutorial_2.proj; PROVENANCE.md), tests/fixtures/projects/{cube,tutorial1}.simpa, tests/fixtures/upstream/tutorial1.
- testdata/ holds only elmia_corrected.ply (+ edt_parity/); README says rooms/elmia_corrected.simpa replaces it.

## 5. UI
- NOTE: UI is Tauri/React (app/ui/src), not ImGui. Outliner: app/ui/src/chrome/ScenePanel.tsx:129-178 (view.surface_groups.map, data-entity=`surface_group:${g.id}`); selection kinds 'group'/'faces'. Also chrome/GeometryPanel.tsx, chrome/sceneModel.ts, bindings/schema.ts (generated).
