// Generates the ACL for the app's own commands: with an app manifest, each command is denied
// unless a capability grants its `allow-<command>` permission (capabilities/default.json).
fn main() {
    let commands = &[
        "ping",
        "panic_probe",
        "panic_probe_unguarded",
        "unguarded_panic_runs",
        "bench_prepare",
        "bench_take",
        "bench_clear",
        "run_events_probe",
        "exact_float_probe",
        "project_new",
        "project_load_text",
        "project_open",
        "project_info",
        "project_json",
        "project_apply",
        "project_undo",
        "project_redo",
        "app_startup",
        "selftest_report",
        "scene_state",
        "scene_new",
        "scene_open",
        "model_import",
        "project_save",
        "edit_apply",
        "edit_undo",
        "edit_redo",
        "scene_mesh",
    ];
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(commands));
    tauri_build::try_build(attributes).expect("tauri-build failed");
}
