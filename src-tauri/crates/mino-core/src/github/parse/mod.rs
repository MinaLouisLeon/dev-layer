//! `gh --json` output into typed rows.
//!
//! Two things are happening here, and only one of them is parsing.
//!
//! The first is **containment**. Everything in this module arrives from
//! GitHub, which is to say from whoever opened the pull request or filed the
//! issue. It becomes `String` fields on the types in
//! [`crate::types`] and nothing else: it is never re-serialised into a
//! command, never given to a renderer as markup, and never used to decide
//! which call to make next. A title is a title.
//!
//! The second is **failing usefully**. `gh` can change the shape of its JSON
//! between versions, so nothing here reaches into a document hopefully. A
//! missing or wrongly typed field is a [`TransportError::Protocol`] naming
//! what was being read, which the UI renders as a sentence a reader can act
//! on - "update gh" is a real answer, and a panic or a silently empty list is
//! not.
//!
//! The exception, and it is deliberate, is a field whose absence is *ordinary*
//! rather than a shape change: a run that has not started has no `startedAt`,
//! a deleted account has no `login`. Those read through [`instant`] and
//! [`login`], which answer with an absence rather than an error. The rule is
//! that a **missing structure** is a protocol error and a **missing value** is
//! not.

mod issues;
mod pulls;
mod review;
mod runs;

pub use issues::issues;
pub use pulls::{pull_request, pull_requests};
pub use review::{review_threads, thread_containing};
pub use runs::{jobs, runs};

use serde_json::Value;

use crate::error::{Result, TransportError};

/// The one sentence a shape this build cannot read produces.
///
/// `what` names the thing being read - `a run`, `the repository` - so the
/// reader is told which surface went wrong rather than being handed a JSON
/// pointer.
pub fn protocol(what: &str) -> TransportError {
    TransportError::protocol(format!(
        "gh answered with something this build could not read while reading {what}. This \
         usually means gh is a different version than expected; try updating it."
    ))
}

/// The whole of one `gh --json` answer.
pub fn document(stdout: &str, what: &str) -> Result<Value> {
    serde_json::from_str(stdout.trim()).map_err(|_| protocol(what))
}

/// A JSON array, or the protocol error. An empty array is a fine answer and
/// is *not* an error: no open pull requests is a state, not a failure.
pub fn array<'a>(value: &'a Value, what: &str) -> Result<&'a Vec<Value>> {
    value.as_array().ok_or_else(|| protocol(what))
}

/// A required string field.
pub fn text(row: &Value, name: &str, what: &str) -> Result<String> {
    row.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| protocol(what))
}

/// A required number field, as a `u64`.
pub fn number(row: &Value, name: &str, what: &str) -> Result<u64> {
    row.get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| protocol(what))
}

/// The same, narrowed to the width the domain types use for a number GitHub
/// assigns. A value that does not fit is a shape this build cannot read.
pub fn count(row: &Value, name: &str, what: &str) -> Result<u32> {
    u32::try_from(number(row, name, what)?).map_err(|_| protocol(what))
}

/// A string field that is allowed to be absent or null.
///
/// `conclusion` on a run still going is `null`, and that is not a shape
/// change - it is what a run that has not finished looks like.
pub fn optional_text(row: &Value, name: &str) -> Option<String> {
    row.get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// A boolean field, defaulting to false when it is absent.
pub fn flag(row: &Value, name: &str) -> bool {
    row.get(name).and_then(Value::as_bool).unwrap_or(false)
}

/// An RFC 3339 field as epoch milliseconds, or `None`.
///
/// Absence is ordinary here - a queued run has no start time - so this never
/// raises. See [`crate::github::time`].
pub fn instant(row: &Value, name: &str) -> Option<u64> {
    super::time::epoch_ms(row.get(name)?.as_str()?)
}

/// The `login` inside `gh`'s author object.
///
/// An empty string for a deleted account, which GitHub reports as `ghost` or
/// as nothing at all. Absence is ordinary, so this does not raise either: a
/// pull request whose author closed their account is still a pull request.
pub fn login(row: &Value, name: &str) -> String {
    row.get(name)
        .and_then(|author| author.get("login"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
