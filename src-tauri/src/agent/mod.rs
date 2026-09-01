//! The command layer: natural language onto the command bus.
//!
//! Claude is given the bus catalog as tools and runs a tool-use loop against
//! it. Every capability the HUD has, the agent has — because both go through
//! `bus::registry`.
//!
//! Rust has no official Anthropic SDK, so this speaks the Messages API over
//! HTTPS directly, with `reqwest` (already a dependency for the HTTP panel).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::bus::registry;
use crate::config::AgentConfig;
use crate::AppState;

pub const AGENT_EVENT: &str = "agent::event";

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
/// Server-side refusal fallbacks: on a policy decline the API re-runs the turn
/// on a fallback model inside the same call, instead of the request simply
/// stopping.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// What the HUD renders as the turn unfolds.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentEvent {
    /// A summary of the model's reasoning, when reasoning display is on.
    Thinking {
        text: String,
    },
    Text {
        text: String,
    },
    ToolUse {
        name: String,
        input: Value,
    },
    ToolResult {
        name: String,
        ok: bool,
        detail: String,
    },
    Done {
        turns: usize,
    },
    Error {
        message: String,
    },
}

/// The conversation, kept in Rust so the API shape never reaches the frontend.
#[derive(Default)]
pub struct AgentSession {
    messages: parking_lot::Mutex<Vec<Value>>,
}

impl AgentSession {
    pub fn reset(&self) {
        self.messages.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.messages.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Runs one user turn to completion, emitting events as it goes.
pub async fn run(app: AppHandle, prompt: String) -> Result<String, String> {
    let config = {
        let state = app.state::<AppState>();
        state.config.agent.clone()
    };
    let api_key = resolve_api_key(&config)?;

    let tools: Vec<Value> = registry::catalog(config.allow_guarded)
        .into_iter()
        .map(|spec| {
            json!({
                "name": spec.name,
                "description": spec.description,
                "input_schema": spec.input_schema,
            })
        })
        .collect();

    {
        let state = app.state::<AppState>();
        let mut messages = state.agent.messages.lock();
        messages.push(json!({ "role": "user", "content": prompt }));
        // Live machine state as a mid-conversation system message: it keeps the
        // cached prefix (static system prompt + tools) intact while still
        // giving the model current facts. It must follow a user message and be
        // the last entry, which is exactly where it sits.
        messages.push(json!({ "role": "system", "content": describe_state(&app) }));
    }

    let client = reqwest::Client::new();
    let mut final_text = String::new();

    for turn in 1..=config.max_iterations {
        let body = {
            let state = app.state::<AppState>();
            let messages = state.agent.messages.lock().clone();
            json!({
                "model": config.model,
                "max_tokens": 16000,
                "system": SYSTEM_PROMPT,
                "thinking": { "type": "adaptive", "display": "summarized" },
                "output_config": { "effort": config.effort },
                "fallbacks": "default",
                "tools": tools,
                "messages": messages,
            })
        };

        let response = client
            .post(API_URL)
            .header("x-api-key", &api_key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", FALLBACK_BETA)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| emit_error(&app, format!("request failed: {e}")))?;

        let status = response.status();
        let payload: Value = response
            .json()
            .await
            .map_err(|e| emit_error(&app, format!("could not read response: {e}")))?;

        if !status.is_success() {
            let message = payload["error"]["message"]
                .as_str()
                .unwrap_or("unknown error");
            return Err(emit_error(&app, format!("Claude API {status}: {message}")));
        }

        let content = payload["content"].as_array().cloned().unwrap_or_default();
        let stop_reason = payload["stop_reason"].as_str().unwrap_or("");

        // A refusal is a normal HTTP 200; check it before reading content.
        if stop_reason == "refusal" {
            let category = payload["stop_details"]["category"]
                .as_str()
                .unwrap_or("unspecified");
            return Err(emit_error(&app, format!("declined ({category})")));
        }

        let mut tool_uses: Vec<(String, String, Value)> = Vec::new();
        for block in &content {
            match block["type"].as_str() {
                Some("thinking") => {
                    if let Some(text) = block["thinking"].as_str().filter(|t| !t.is_empty()) {
                        emit(
                            &app,
                            AgentEvent::Thinking {
                                text: text.to_string(),
                            },
                        );
                    }
                }
                Some("text") => {
                    if let Some(text) = block["text"].as_str() {
                        final_text = text.to_string();
                        emit(
                            &app,
                            AgentEvent::Text {
                                text: text.to_string(),
                            },
                        );
                    }
                }
                Some("tool_use") => {
                    tool_uses.push((
                        block["id"].as_str().unwrap_or_default().to_string(),
                        block["name"].as_str().unwrap_or_default().to_string(),
                        block["input"].clone(),
                    ));
                }
                _ => {}
            }
        }

        {
            let state = app.state::<AppState>();
            state
                .agent
                .messages
                .lock()
                .push(json!({ "role": "assistant", "content": content }));
        }

        if stop_reason != "tool_use" || tool_uses.is_empty() {
            emit(&app, AgentEvent::Done { turns: turn });
            return Ok(final_text);
        }

        // Every result goes back in ONE user message: splitting them teaches
        // the model to stop making parallel calls.
        let mut results = Vec::with_capacity(tool_uses.len());
        for (id, name, input) in tool_uses {
            emit(
                &app,
                AgentEvent::ToolUse {
                    name: name.clone(),
                    input: input.clone(),
                },
            );

            let (content, is_error, detail) = match registry::dispatch(&app, &name, &input).await {
                Ok(value) => {
                    let text = compact(&value);
                    (text.clone(), false, text)
                }
                // Failures are reported to the model as tool results, not
                // raised: it can then apologise, retry, or pick another tool.
                Err(message) => (format!("error: {message}"), true, message),
            };

            emit(
                &app,
                AgentEvent::ToolResult {
                    name,
                    ok: !is_error,
                    detail,
                },
            );
            results.push(json!({
                "type": "tool_result",
                "tool_use_id": id,
                "content": content,
                "is_error": is_error,
            }));
        }

        let state = app.state::<AppState>();
        state
            .agent
            .messages
            .lock()
            .push(json!({ "role": "user", "content": results }));
    }

    let message = format!(
        "stopped after {} turns without finishing",
        config.max_iterations
    );
    Err(emit_error(&app, message))
}

const SYSTEM_PROMPT: &str = "\
You are the command layer of dev-layer, a fullscreen developer HUD that overlays Windows. \
The user speaks to you through a prompt on their own machine; you act on it with the tools provided.

The tools are dev-layer's own capabilities: the application catalog, the tiling window manager, \
the telemetry sampler, and the displays. Prefer acting over explaining — if the user asks you to \
open something, arrange something, or tell them what the machine is doing, use a tool rather than \
describing what they could do themselves.

Answer in one or two sentences. This is a heads-up display, not a chat window: the user is \
mid-task and reading in their peripheral vision. State what you did, not how you did it. If a \
request is ambiguous between two windows or applications, pick the best match and say which you \
chose rather than asking. If you genuinely cannot do something with the tools available, say so \
plainly in one sentence.";

/// The live facts the model should not have to ask for.
fn describe_state(app: &AppHandle) -> String {
    let state = app.state::<AppState>();

    let monitors = state.monitors.snapshot();
    let displays: Vec<String> = monitors
        .iter()
        .map(|m| {
            format!(
                "{} ({}, {}x{}{})",
                m.id,
                m.name,
                m.bounds.width,
                m.bounds.height,
                if m.is_primary { ", primary" } else { "" }
            )
        })
        .collect();

    let windows = state.wm.windows();
    let open: Vec<String> = windows
        .iter()
        .map(|w| {
            format!(
                "{} [{}]{}",
                w.title,
                w.process,
                if w.focused { " (focused)" } else { "" }
            )
        })
        .collect();

    let metrics = state
        .metrics
        .latest()
        .map(|m| {
            format!(
                "CPU {:.0}%, memory {:.0}% of {:.1} GB, GPU {}",
                m.cpu.usage,
                if m.memory.total > 0 {
                    m.memory.used as f64 / m.memory.total as f64 * 100.0
                } else {
                    0.0
                },
                m.memory.total as f64 / 1e9,
                m.gpus
                    .first()
                    .and_then(|g| g.utilization)
                    .map(|u| format!("{u:.0}%"))
                    .unwrap_or_else(|| "unavailable".into())
            )
        })
        .unwrap_or_else(|| "not sampled yet".into());

    format!(
        "Current state.\nDisplays: {}\nOpen windows: {}\nUsage: {}\nApplications installed: {}",
        if displays.is_empty() {
            "none detected".into()
        } else {
            displays.join("; ")
        },
        if open.is_empty() {
            "none".into()
        } else {
            open.join("; ")
        },
        metrics,
        state.apps.entries().len()
    )
}

/// Tool results are read by the model, not stored: keep them small.
fn compact(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
    if text.len() <= 6000 {
        return text;
    }
    format!("{}… (truncated)", &text[..6000])
}

fn resolve_api_key(config: &AgentConfig) -> Result<String, String> {
    config
        .api_key
        .clone()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            "No API key. Set ANTHROPIC_API_KEY in the environment, or agent.apiKey in config.json."
                .to_string()
        })
}

fn emit(app: &AppHandle, event: AgentEvent) {
    if let Err(e) = app.emit(AGENT_EVENT, &event) {
        tracing::warn!(error = %e, "could not emit agent event");
    }
}

/// Emits the error and returns it, so callers can `return Err(emit_error(..))`.
fn emit_error(app: &AppHandle, message: String) -> String {
    // Deliberately not logging the request body: it carries the API key header
    // and the user's prompt.
    tracing::warn!(%message, "agent turn failed");
    emit(
        app,
        AgentEvent::Error {
            message: message.clone(),
        },
    );
    message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub enabled: bool,
    pub configured: bool,
    pub model: String,
    pub allow_guarded: bool,
    pub turns: usize,
}

pub fn status(app: &AppHandle) -> AgentStatus {
    let state = app.state::<AppState>();
    let config = &state.config.agent;

    AgentStatus {
        enabled: config.enabled,
        configured: resolve_api_key(config).is_ok(),
        model: config.model.clone(),
        allow_guarded: config.allow_guarded,
        turns: state.agent.len(),
    }
}
