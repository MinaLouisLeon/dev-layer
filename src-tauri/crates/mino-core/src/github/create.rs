//! The one query that writes: what is checked before it runs, and what is read
//! back afterwards.
//!
//! Creating a pull request is the only irreversible thing this module does -
//! not destructive, but public, and visible to everybody watching the
//! repository the moment it lands. The UI confirms first and shows exactly
//! what will be created; this file is the second half of that promise, and
//! refuses the requests that could only produce something nobody meant.

use crate::error::{Result, TransportError};
use crate::git::refname;
use crate::types::{MAX_PR_BODY_BYTES, MAX_PR_TITLE_BYTES};

/// A title and a base worth sending, or the sentence saying why not.
///
/// Returns the trimmed title and base, because a title of spaces is not a
/// title and `gh` would happily create a pull request called `"   "`.
pub fn validate(title: &str, body: &str, base: &str) -> Result<(String, String)> {
    let title = title.trim();
    if title.is_empty() {
        return Err(TransportError::invalid("a pull request needs a title"));
    }
    if title.len() > MAX_PR_TITLE_BYTES {
        return Err(TransportError::invalid(format!(
            "that title is {} bytes, above the {MAX_PR_TITLE_BYTES} byte ceiling",
            title.len()
        )));
    }
    // A title travels in argv and, over SSH, through the remote quoting rule.
    // A newline in one would also make `gh` read the rest as a second thing
    // entirely, so it is refused here rather than sent.
    if let Some(bad) = title.chars().find(|ch| ch.is_control()) {
        return Err(TransportError::invalid(format!(
            "a pull request title cannot contain the control character U+{:04X}. Put the \
             detail in the description instead.",
            bad as u32
        )));
    }
    if body.len() > MAX_PR_BODY_BYTES {
        return Err(TransportError::invalid(format!(
            "that description is {} bytes, above the {MAX_PR_BODY_BYTES} byte ceiling",
            body.len()
        )));
    }
    // The base is a branch name, so it gets the branch guard rather than a
    // length check of its own.
    let base = refname::precheck(base)?;
    Ok((title.to_string(), base))
}

/// The URL `gh pr create` printed, and the number in it.
///
/// `gh` writes the address of the new pull request on stdout. Taking the last
/// line that looks like one, rather than the whole of stdout, is what keeps an
/// update notice or a push progress line out of the answer.
pub fn parse(stdout: &str) -> Result<(String, Option<u32>)> {
    let url = stdout
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("https://"))
        .ok_or_else(|| {
            TransportError::protocol(
                "gh created the pull request but did not print its address. Check the \
                 repository on github.com.",
            )
        })?;
    Ok((url.to_string(), number_in(url)))
}

/// The trailing `/pull/123` of a pull request URL, when there is one.
fn number_in(url: &str) -> Option<u32> {
    url.rsplit('/').next()?.parse().ok()
}

#[cfg(test)]
mod tests;
