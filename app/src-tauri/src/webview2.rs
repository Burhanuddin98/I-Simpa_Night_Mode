//! Everything specific to the WebView2 runtime lives in this module and nowhere else.
//!
//! Tauri 3 moves the runtime-specific APIs into extension traits on the runtime crates
//! (`with_webview` becomes `WebviewWryExt::with_wry_webview`; rebuild plan, risks[13]), so keeping
//! them here makes that migration one file. The virtual-host mapping fallback for large datasets
//! would live here too; the M9 benchmark decided it is not needed (docs/decisions/ipc.md).

use schemars::JsonSchema;
use serde::Serialize;

/// The webview engine and its runtime version, for the self-test and the Console.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WebviewInfo {
    pub engine: &'static str,
    /// `None` when the runtime cannot be queried (it is then missing or broken).
    pub version: Option<String>,
}

pub fn info() -> WebviewInfo {
    WebviewInfo {
        engine: if cfg!(windows) {
            "WebView2"
        } else {
            "system webview"
        },
        version: tauri::webview_version().ok(),
    }
}
