//! GitHub: what `gh` said, decided in one place.
//!
//! The third module of its kind, after [`crate::search`] and [`crate::git`],
//! and it earns the shape for the same reason both of those did: the
//! per-transport code *runs* `gh` - `tokio::process` locally, an exec channel
//! over SSH - and everything it *decides* is here. Local and SSH cannot drift
//! into disagreeing about what a failed run looks like, because neither of
//! them reads it.
//!
//! ## The credential position, which is the reason for all of this
//!
//! **This application never holds a GitHub token.** Not on disk, not in
//! memory, not for the length of one call. Every request shells out to the
//! `gh` CLI, which owns its own authentication and keeps it in the operating
//! system's keychain under its own name. The standing rule - no credential,
//! token or passphrase written to disk, to a log or to browser storage - is
//! honoured here by there being nothing to write.
//!
//! Three consequences follow, and they are features rather than apologies:
//!
//! - `gh` must be installed and logged in. Where it is not, [`probe`] says so
//!   and every GitHub surface goes quiet - the same shape the `nu` and `git`
//!   probes already have.
//! - Only GitHub remotes are served. A GitLab or Bitbucket remote is a quiet
//!   absence, not an error.
//! - The app cannot offer to log anybody in. It can only name the command,
//!   `gh auth login`, which is the correct division of responsibility: an
//!   interactive browser handshake is not something a workbench should be
//!   standing in the middle of.
//!
//! ## Two rules this module exists to hold
//!
//! - **Argv only, and named subcommands only.** No caller value is
//!   interpolated into a `gh` command line, and no caller can name a
//!   subcommand: [`crate::types::GitHubQuery`] is an enum, and the program
//!   text for each variant lives in [`command`]. The one value large and free
//!   enough to be awkward - a pull request body - travels on **stdin** and
//!   never reaches argv at all.
//! - **`gh` output is untrusted input.** Titles, branch names and bodies come
//!   from whoever opened the pull request or the issue. [`parse`] turns them
//!   into `String` fields on typed rows and nothing else: they are never
//!   markup, and never go back into a command.
//!
//! And one rule about failure. `gh` is a program whose JSON shape can change
//! between versions, so every field is asked for explicitly with `--json` and
//! anything missing or malformed is a **typed protocol error**, never a panic
//! and never a silently empty list. See [`parse::protocol`].

pub mod browse;
pub mod call;
pub mod command;
pub mod create;
pub mod parse;
pub mod probe;
pub mod time;

/// What one `gh` call produced.
///
/// Structurally identical to [`crate::git::GitOutput`], because it is the same
/// three facts any child process reports. Reused rather than redeclared: a
/// second shape would mean every runner picking which one to fill in, and the
/// two drifting the first time one of them learned something the other did
/// not.
pub use crate::git::GitOutput as GhOutput;

/// The program. The only place its name is written down.
pub const GH_PROGRAM: &str = "gh";

/// Wall-clock ceiling for one `gh` call.
///
/// Longer than a git call's, and for a different reason: this one goes over
/// the network. Long enough for a cold `gh pr list` on a slow connection,
/// short enough that a stalled request becomes a sentence in the section
/// rather than a pane that never finishes loading.
pub const DEFAULT_TIMEOUT_MS: u64 = 20_000;

/// Absolute path to `gh`, or `None` when it is not on PATH. The counterpart of
/// [`crate::git::find_git`] and [`crate::shell::find_nu`].
pub fn find_gh() -> Option<String> {
    which::which(GH_PROGRAM)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// What a failed `gh` call should say: `gh`'s own words when it had any, and a
/// sentence naming the operation when it did not.
pub fn message_or(output: &GhOutput, what: &str) -> String {
    let text = output.stderr.trim();
    if text.is_empty() {
        format!("gh {what} failed")
    } else {
        text.to_string()
    }
}
