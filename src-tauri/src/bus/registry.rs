//! The command catalog: every capability dev-layer has, described well enough
//! for a language model to choose between them.
//!
//! This is why the bus was built in milestone 1. The HUD calls these through
//! typed Tauri commands; the agent calls the same set through tool use. A new
//! capability added here is immediately available to both.

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::wm::LayoutKind;
use crate::AppState;

/// One callable capability.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema for the arguments, in the shape the Messages API expects.
    pub input_schema: Value,
    /// Guarded commands can act outside dev-layer's own UI — closing someone's
    /// window, making network requests. Off unless explicitly enabled.
    pub guarded: bool,
}

/// The catalog offered to the model. Guarded commands are *omitted* rather
/// than refused at call time: a tool the model cannot see is a tool it cannot
/// talk itself into using.
pub fn catalog(allow_guarded: bool) -> Vec<CommandSpec> {
    let all = vec![
        CommandSpec {
            name: "list_monitors",
            description: "List the connected displays: id, name, resolution, scale factor, and which is primary.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            guarded: false,
        },
        CommandSpec {
            name: "system_metrics",
            description: "Current CPU, memory, GPU, network and disk usage, plus the top processes by CPU. Use this for any question about what the machine is doing.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            guarded: false,
        },
        CommandSpec {
            name: "list_apps",
            description: "List installed applications dev-layer can launch. Optionally filter by a substring of the name.",
            input_schema: json!({
                "type": "object",
                "properties": { "filter": { "type": "string", "description": "Case-insensitive substring of the application name." } },
                "additionalProperties": false
            }),
            guarded: false,
        },
        CommandSpec {
            name: "launch_app",
            description: "Launch an installed application by name. Matches loosely, so \"vscode\" or \"code\" finds Visual Studio Code.",
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string", "description": "Application name as the user said it." } },
                "required": ["name"],
                "additionalProperties": false
            }),
            guarded: false,
        },
        CommandSpec {
            name: "list_windows",
            description: "List the windows currently under management: title, process, monitor, tiling slot, and which is focused.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            guarded: false,
        },
        CommandSpec {
            name: "focus_window",
            description: "Bring a window to the foreground. Matches on window title or process name.",
            input_schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "Part of the window title or process name." } },
                "required": ["query"],
                "additionalProperties": false
            }),
            guarded: false,
        },
        CommandSpec {
            name: "set_layout",
            description: "Set the tiling layout for a display. Layouts: mainStack (one large pane plus a stack), columns, grid, monocle (one window full-region), float (tiling off).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "layout": { "type": "string", "enum": ["mainStack", "columns", "grid", "monocle", "float"] },
                    "monitor": { "type": "string", "description": "Monitor id from list_monitors. Defaults to the display holding the focused window." }
                },
                "required": ["layout"],
                "additionalProperties": false
            }),
            guarded: false,
        },
        CommandSpec {
            name: "float_window",
            description: "Take a window out of tiling, or put it back. Matches on window title or process name.",
            input_schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"],
                "additionalProperties": false
            }),
            guarded: false,
        },
        CommandSpec {
            name: "close_window",
            description: "Ask a window to close. The application decides what to do — unsaved work may prompt.",
            input_schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"],
                "additionalProperties": false
            }),
            guarded: true,
        },
        CommandSpec {
            name: "http_request",
            description: "Send an HTTP request from this machine and return the status, timing and body. Use for checking whether a local service is up, or calling an API the user names.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "method": { "type": "string", "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] },
                    "url": { "type": "string" },
                    "body": { "type": "string", "description": "Request body, for methods that take one." }
                },
                "required": ["method", "url"],
                "additionalProperties": false
            }),
            guarded: true,
        },
    ];

    all.into_iter()
        .filter(|spec| allow_guarded || !spec.guarded)
        .collect()
}

/// Runs one command. Errors come back as strings because they are handed
/// straight to the model as a failed `tool_result`.
pub async fn dispatch(app: &AppHandle, name: &str, input: &Value) -> Result<Value, String> {
    let state = app.state::<AppState>();

    match name {
        "list_monitors" => Ok(json!(state.monitors.snapshot())),

        "system_metrics" => state
            .metrics
            .latest()
            .map(|snapshot| json!(snapshot))
            .ok_or_else(|| "no metrics sampled yet".to_string()),

        "list_apps" => {
            let filter = input
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let apps: Vec<Value> = state
                .apps
                .entries()
                .into_iter()
                .filter(|app| filter.is_empty() || app.name.to_lowercase().contains(&filter))
                .map(|app| json!({ "name": app.name, "pinned": app.pinned, "devTool": app.is_dev_tool }))
                .collect();
            Ok(json!({ "count": apps.len(), "apps": apps }))
        }

        "launch_app" => {
            let query = required_str(input, "name")?;
            let apps = state.apps.entries();
            let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
            let index = best_match(query, &names)
                .ok_or_else(|| format!("no installed application matches {query:?}"))?;

            let entry = &apps[index];
            crate::platform::sys::launch(&entry.launch_path, "", None)
                .map_err(|e| format!("could not launch {}: {e}", entry.name))?;
            Ok(json!({ "launched": entry.name }))
        }

        "list_windows" => Ok(json!(state.wm.windows())),

        "focus_window" | "float_window" | "close_window" => {
            let query = required_str(input, "query")?;
            let windows = state.wm.windows();
            let labels: Vec<String> = windows
                .iter()
                .map(|w| format!("{} {}", w.title, w.process))
                .collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let index = best_match(query, &refs)
                .ok_or_else(|| format!("no open window matches {query:?}"))?;
            let window = &windows[index];

            match name {
                "focus_window" => {
                    state.wm.set_focused(Some(window.id));
                    crate::platform::sys::focus_window(window.id as isize)
                        .map_err(|e| e.to_string())?;
                    Ok(json!({ "focused": window.title }))
                }
                "float_window" => {
                    let floating = state.wm.toggle_float(window.id).unwrap_or(false);
                    crate::wm::retile(app);
                    Ok(json!({ "window": window.title, "floating": floating }))
                }
                _ => {
                    crate::platform::sys::close_window(window.id as isize)
                        .map_err(|e| e.to_string())?;
                    Ok(json!({ "closing": window.title }))
                }
            }
        }

        "set_layout" => {
            let layout = required_str(input, "layout")?;
            let kind: LayoutKind = serde_json::from_value(json!(layout))
                .map_err(|_| format!("unknown layout {layout:?}"))?;

            let monitor = match input.get("monitor").and_then(Value::as_str) {
                Some(id) => id.to_string(),
                None => state
                    .wm
                    .windows()
                    .iter()
                    .find(|w| w.focused)
                    .map(|w| w.monitor_id.clone())
                    .or_else(|| {
                        state
                            .monitors
                            .snapshot()
                            .into_iter()
                            .find(|m| m.is_primary)
                            .map(|m| m.id)
                    })
                    .ok_or_else(|| "no monitor to apply a layout to".to_string())?,
            };

            state.wm.set_layout(&monitor, kind);
            crate::wm::retile(app);
            Ok(json!({ "monitor": monitor, "layout": layout }))
        }

        "http_request" => {
            let request = crate::panels::HttpRequest {
                method: required_str(input, "method")?.to_string(),
                url: required_str(input, "url")?.to_string(),
                headers: Vec::new(),
                body: input
                    .get("body")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                timeout_ms: Some(15_000),
            };
            let response = crate::panels::send_request(request)
                .await
                .map_err(|e| e.to_string())?;

            // Bodies can be enormous; the model needs the shape, not the payload.
            let body: String = response.body.chars().take(4000).collect();
            Ok(json!({
                "status": response.status,
                "elapsedMs": response.elapsed_ms,
                "size": response.size,
                "body": body,
                "truncated": body.len() < response.body.len()
            }))
        }

        other => Err(format!("unknown command {other:?}")),
    }
}

fn required_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required argument {key:?}"))
}

/// Loose name matching, so "vscode", "code" and "visual studio" all resolve.
///
/// Ranked: exact, then prefix, then whole-word, then substring, then a
/// subsequence fallback that catches abbreviations. Ties go to the shortest
/// candidate — "Code" beats "Code Helper (Renderer)".
pub fn best_match(query: &str, candidates: &[&str]) -> Option<usize> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }

    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let hay = candidate.to_lowercase();
            let rank = if hay == needle {
                0
            } else if hay.starts_with(&needle) {
                1
            } else if hay
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word == needle)
            {
                2
            } else if hay.contains(&needle) {
                3
            } else if is_subsequence(&needle, &hay) {
                4
            } else {
                return None;
            };
            Some((rank, hay.len(), index))
        })
        .min()
        .map(|(_, _, index)| index)
}

/// Every character of `needle` appears in `hay`, in order: catches "vscode"
/// inside "visual studio code".
fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut chars = hay.chars();
    needle.chars().all(|wanted| chars.any(|c| c == wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPS: [&str; 6] = [
        "Visual Studio Code",
        "Visual Studio Code Insiders",
        "Postman",
        "Google Chrome",
        "Docker Desktop",
        "Windows Terminal",
    ];

    #[test]
    fn matches_the_way_people_say_names() {
        let pick = |q: &str| APPS[best_match(q, &APPS).expect(q)];

        assert_eq!(pick("postman"), "Postman");
        assert_eq!(pick("chrome"), "Google Chrome");
        assert_eq!(pick("docker"), "Docker Desktop");
        // Abbreviation, matched as a subsequence.
        assert_eq!(pick("vscode"), "Visual Studio Code");
    }

    #[test]
    fn prefers_the_shorter_candidate_on_a_tie() {
        // Both contain "visual studio code"; the plain one should win.
        assert_eq!(
            APPS[best_match("visual studio code", &APPS).unwrap()],
            "Visual Studio Code"
        );
    }

    #[test]
    fn returns_nothing_rather_than_a_wrong_guess() {
        assert!(best_match("photoshop", &APPS).is_none());
        assert!(best_match("   ", &APPS).is_none());
    }

    #[test]
    fn guarded_commands_are_hidden_unless_enabled() {
        let safe = catalog(false);
        assert!(safe.iter().all(|spec| !spec.guarded));
        assert!(!safe.iter().any(|spec| spec.name == "close_window"));

        let full = catalog(true);
        assert!(full.len() > safe.len());
        assert!(full.iter().any(|spec| spec.name == "http_request"));
    }

    #[test]
    fn every_command_declares_an_object_schema() {
        for spec in catalog(true) {
            assert_eq!(spec.input_schema["type"], "object", "{}", spec.name);
            assert!(!spec.description.is_empty(), "{}", spec.name);
        }
    }
}
