//! Taking secrets out of git's own words before anybody sees them.
//!
//! Phase 6 is the first time this crate reports text from a call that talked
//! to a remote, and remote text is the one place a credential can appear
//! without anybody having put it there. A remote configured as
//! `https://mina:ghp_abc123@github.com/o/r` is an ordinary thing to find in
//! somebody's `.git/config`, and git will happily print it back in a progress
//! line, an error, or `git remote -v`.
//!
//! So the rule for this phase is: **no text from a remote call reaches a
//! message, a result or a log line without passing through here first.** Not
//! "when it looks like it might contain one" - always. `redact` is cheap, it
//! is idempotent, and the failure mode of forgetting it once is a token in a
//! screenshot.
//!
//! What it does *not* do is try to recognise secrets by shape. A regex for
//! "things that look like tokens" is a guess that fails open, and the one
//! format it has not been taught is the one that leaks. This works on
//! **structure** instead - the userinfo field of a URL is the place a
//! credential is allowed to be, so that field is removed wherever it appears -
//! which is a rule that cannot be out of date.

/// What replaces a credential. Kept obviously artificial so a reader can tell
/// a redaction from a username.
const MASK: &str = "***";

/// The longest text worth carrying into a message.
///
/// A push can print a hundred lines of progress. Nothing downstream renders
/// more than a sentence or two, and an unbounded string is an unbounded thing
/// to put in an error type that crosses an IPC boundary.
const MAX_SUMMARY_BYTES: usize = 2_000;

/// Every `scheme://userinfo@host` in `text`, with the userinfo removed.
///
/// Handles the shapes git actually produces:
///
/// | Before | After |
/// | --- | --- |
/// | `https://user:token@host/o/r` | `https://***@host/o/r` |
/// | `https://token@host/o/r` | `https://***@host/o/r` |
/// | `ssh://git@host/o/r` | `ssh://git@host/o/r` |
///
/// The third row is the interesting one. `git@host` is not a credential, it is
/// the conventional SSH login, and masking it would make every ordinary SSH
/// remote unreadable for no gain. A userinfo with **no colon** is a username,
/// and a username on its own is not a secret - except over HTTP(S), where a
/// bare userinfo *is* how a token is passed. So the rule is applied by scheme
/// rather than by shape.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find("://") {
        let (before, after) = rest.split_at(at);
        let scheme_start = scheme_start(before);
        let scheme = &before[scheme_start..];
        // Everything before the scheme is ordinary text and passes through.
        out.push_str(&before[..scheme_start]);
        out.push_str(scheme);
        out.push_str("://");

        let authority = &after[3..];
        // The authority ends at the first `/`, whitespace, or quote; a `@`
        // after that belongs to a path, not to userinfo.
        let end = authority
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(authority.len());
        let (host_part, tail) = authority.split_at(end);

        match host_part.rsplit_once('@') {
            Some((userinfo, host)) if masks(scheme, userinfo) => {
                out.push_str(MASK);
                out.push('@');
                out.push_str(host);
            }
            _ => out.push_str(host_part),
        }
        rest = tail;
    }
    out.push_str(rest);
    truncate(&out)
}

/// The same, for a value that may be absent, dropping it when nothing is left.
///
/// An empty summary helps nobody, and `Some("")` renders as a blank line where
/// a reader expects a sentence.
pub fn summary(text: &str) -> Option<String> {
    let redacted = redact(text);
    let trimmed = redacted.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Whether this userinfo is a credential rather than a login name.
///
/// Over HTTP(S) any userinfo is treated as one: a bare `https://<token>@host`
/// is exactly how a personal access token is written into a remote URL. Over
/// SSH and the git protocol, a userinfo with no password half is the login -
/// `git@github.com` - and masking it would be noise.
fn masks(scheme: &str, userinfo: &str) -> bool {
    let http = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
    http || userinfo.contains(':')
}

/// Where the scheme starts in the text preceding `://`.
///
/// Git's lines are usually `remote: https://…` or `fatal: unable to access
/// 'https://…'`, so the scheme is the last run of scheme-legal characters.
fn scheme_start(before: &str) -> usize {
    before
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'))
        .map_or(0, |at| at + 1)
}

/// Cut at the ceiling, on a character boundary, saying that it was cut.
fn truncate(text: &str) -> String {
    if text.len() <= MAX_SUMMARY_BYTES {
        return text.to_string();
    }
    let mut end = MAX_SUMMARY_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
mod tests;
