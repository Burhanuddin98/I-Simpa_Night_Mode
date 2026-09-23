//! The JSON Schemas the UI's TypeScript types are generated from: `app.exe --dump-schema <dir>`
//! writes them, `npm run bindings` turns them into `ui/src/bindings/*.ts`, and both are
//! committed. Regenerating must give no diff (M9 gate e).
//!
//! - `schema.json`: the project ([`simpa_core::schema::json_schema`]) and the edit op
//!   ([`simpa_core::schema::Op`]) in one document, so each shared definition (`F64`, the ids,
//!   `BandSet`, ...) is declared once.
//! - `ipc.json`: what the app's commands return and stream (this crate's types).
//!
//! Two definitions with one name but different bodies are refused, never merged.

use serde_json::{Map, Value, json};
use simpa_core::schema::{Op, json_schema};

use crate::bench::Prepared;
use crate::bridge::{FloatProbe, ProjectInfo};
use crate::commands::{EventsProbeReport, StartupInfo};
use crate::events::RunEventBatch;
use crate::guard::CmdError;

struct Dump {
    file: &'static str,
    title: &'static str,
    description: &'static str,
    /// (property name, type name, schema)
    roots: Vec<(&'static str, &'static str, Value)>,
}

fn dumps() -> Vec<Dump> {
    use schemars::schema_for;
    vec![
        Dump {
            file: "schema.json",
            title: "SchemaBindings",
            description: "simpa_core::schema, dumped by app.exe --dump-schema. Do not edit.",
            roots: vec![
                ("project", "Project", json_schema().to_value()),
                ("op", "Op", schema_for!(Op).to_value()),
            ],
        },
        Dump {
            file: "ipc.json",
            title: "IpcBindings",
            description: "The app's command results and events, dumped by app.exe --dump-schema. Do not edit.",
            roots: vec![
                ("cmd_error", "CmdError", schema_for!(CmdError).to_value()),
                (
                    "startup_info",
                    "StartupInfo",
                    schema_for!(StartupInfo).to_value(),
                ),
                (
                    "project_info",
                    "ProjectInfo",
                    schema_for!(ProjectInfo).to_value(),
                ),
                ("prepared", "Prepared", schema_for!(Prepared).to_value()),
                (
                    "float_probe",
                    "FloatProbe",
                    schema_for!(FloatProbe).to_value(),
                ),
                (
                    "run_event_batch",
                    "RunEventBatch",
                    schema_for!(RunEventBatch).to_value(),
                ),
                (
                    "events_probe_report",
                    "EventsProbeReport",
                    schema_for!(EventsProbeReport).to_value(),
                ),
            ],
        },
    ]
}

fn merged(dump: Dump) -> Result<Value, String> {
    let mut defs = Map::new();
    let mut properties = Map::new();
    for (property, name, schema) in dump.roots {
        let Value::Object(mut root) = schema else {
            return Err(format!("the {name} schema is not an object"));
        };
        // As a definition it is named by its key; the root-only `title` would make it differ from
        // the copy another root carries in its `$defs`.
        root.remove("$schema");
        root.remove("title");
        let own_defs = root.remove("$defs");
        // A recursive root (`Op::Batch` holds ops) refers to itself as "#"; once the root is a
        // definition, that reference must name it.
        let self_ref = format!("#/$defs/{name}");
        if let Some(Value::Object(sub)) = own_defs {
            for (k, mut v) in sub {
                retarget_root_refs(&mut v, &self_ref);
                insert_def(&mut defs, k, v)?;
            }
        }
        let mut body = Value::Object(root);
        retarget_root_refs(&mut body, &self_ref);
        insert_def(&mut defs, name.to_string(), body)?;
        properties.insert(property.to_string(), json!({ "$ref": self_ref }));
    }
    let required: Vec<Value> = properties.keys().cloned().map(Value::String).collect();
    Ok(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": dump.title,
        "description": dump.description,
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
        "$defs": defs,
    }))
}

fn retarget_root_refs(value: &mut Value, target: &str) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if k == "$ref" && v.as_str() == Some("#") {
                    *v = Value::String(target.to_string());
                } else {
                    retarget_root_refs(v, target);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| retarget_root_refs(v, target)),
        _ => {}
    }
}

fn insert_def(defs: &mut Map<String, Value>, name: String, body: Value) -> Result<(), String> {
    match defs.get(&name) {
        Some(existing) if *existing != body => Err(format!(
            "definition '{name}' differs between two roots of one dump"
        )),
        Some(_) => Ok(()),
        None => {
            defs.insert(name, body);
            Ok(())
        }
    }
}

/// Every dump as (file name, text): pretty JSON with a final newline, keys sorted.
pub fn dump_texts() -> Result<Vec<(&'static str, String)>, String> {
    dumps()
        .into_iter()
        .map(|d| {
            let file = d.file;
            let mut text = serde_json::to_string_pretty(&merged(d)?).map_err(|e| e.to_string())?;
            text.push('\n');
            Ok((file, text))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed() -> Vec<(&'static str, Value)> {
        dump_texts()
            .unwrap()
            .into_iter()
            .map(|(f, t)| (f, serde_json::from_str(&t).unwrap()))
            .collect()
    }

    #[test]
    fn every_reference_resolves_inside_its_own_dump() {
        for (file, s) in parsed() {
            let defs = s["$defs"].as_object().unwrap();
            let text = s.to_string();
            for (i, _) in text.match_indices("\"$ref\":\"") {
                let rest = &text[i + 8..];
                let target = &rest[..rest.find('"').unwrap()];
                let name = target
                    .strip_prefix("#/$defs/")
                    .unwrap_or_else(|| panic!("{file}: {target}"));
                assert!(defs.contains_key(name), "{file}: dangling {target}");
            }
        }
    }

    #[test]
    fn the_schema_dump_holds_project_and_op_with_shared_definitions() {
        let (_, s) = parsed().remove(0);
        let defs = s["$defs"].as_object().unwrap();
        for name in ["Project", "Op", "BandSet", "Geometry", "Material", "Camera"] {
            assert!(defs.contains_key(name), "missing {name}");
        }
        assert_eq!(s["properties"]["project"]["$ref"], "#/$defs/Project");
        assert_eq!(s["properties"]["op"]["$ref"], "#/$defs/Op");
        // Op is recursive (Batch); its self-reference must name the definition.
        assert!(
            s["$defs"]["Op"]
                .to_string()
                .contains("\"$ref\":\"#/$defs/Op\"")
        );
    }

    #[test]
    fn the_ipc_dump_holds_every_command_result() {
        let (_, s) = parsed().remove(1);
        let defs = s["$defs"].as_object().unwrap();
        for name in [
            "CmdError",
            "StartupInfo",
            "ProjectInfo",
            "Prepared",
            "FloatProbe",
            "RunEventBatch",
            "RunEvent",
            "EventsProbeReport",
        ] {
            assert!(defs.contains_key(name), "missing {name}");
        }
    }

    #[test]
    fn the_dump_is_deterministic() {
        assert_eq!(dump_texts().unwrap(), dump_texts().unwrap());
    }

    #[test]
    fn a_conflicting_definition_is_refused() {
        let mut defs = Map::new();
        insert_def(&mut defs, "A".into(), json!(1)).unwrap();
        insert_def(&mut defs, "A".into(), json!(1)).unwrap();
        assert!(insert_def(&mut defs, "A".into(), json!(2)).is_err());
    }
}
