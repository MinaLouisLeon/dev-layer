//! The remote path guard.
//!
//! The same rule as `local::roots`, enforced over SFTP: a path that resolves
//! outside the connected root is rejected before any remote operation runs.
//!
//! One structural difference. Locally, canonicalisation is a syscall the guard
//! can make itself. Remotely it is a round trip (`SFTP realpath`), so the
//! guard is split: it builds the candidate path, the caller canonicalises, and
//! the guard rules on the answer. `fs::resolve` is the only place that
//! sequence is written, so no call site can skip the second half.

use crate::error::{Result, TransportError};

/// Remote paths are POSIX regardless of what the client runs on.
pub const SEP: char = '/';

#[derive(Debug, Clone)]
pub struct RemoteRoot {
    root: String,
}

impl RemoteRoot {
    /// `canonical` must already have been through `SFTP realpath`.
    pub fn new(canonical: &str) -> Result<Self> {
        let root = normalise(canonical);
        if root.is_empty() {
            return Err(TransportError::invalid(
                "the remote root resolved to an empty path",
            ));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    /// The path to hand to `realpath`. A relative path is taken as relative to
    /// the root, which matches how the tree addresses children.
    pub fn candidate(&self, path: &str) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return self.root.clone();
        }
        if trimmed.starts_with(SEP) {
            return trimmed.to_string();
        }
        if self.root.ends_with(SEP) {
            format!("{}{trimmed}", self.root)
        } else {
            format!("{}{SEP}{trimmed}", self.root)
        }
    }

    /// True when `canonical` is the root or sits beneath it. The separator in
    /// the prefix test is what stops `/srv/appdata` matching root `/srv/app`.
    pub fn contains(&self, canonical: &str) -> bool {
        let candidate = normalise(canonical);
        if candidate == self.root {
            return true;
        }
        let prefix = if self.root.ends_with(SEP) {
            self.root.clone()
        } else {
            format!("{}{SEP}", self.root)
        };
        candidate.starts_with(&prefix)
    }

    /// Rules on a canonicalised path, returning it only when it is inside.
    pub fn ensure(&self, requested: &str, canonical: String) -> Result<String> {
        if self.contains(&canonical) {
            Ok(normalise(&canonical))
        } else {
            Err(TransportError::PathEscapesRoot {
                path: requested.to_string(),
            })
        }
    }
}

/// Collapses repeated separators and drops a trailing one, so that string
/// comparison in `contains` is meaningful. `/` itself is preserved.
pub fn normalise(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let mut out = String::with_capacity(replaced.len());
    let mut last_sep = false;
    for ch in replaced.chars() {
        let is_sep = ch == SEP;
        if is_sep && last_sep {
            continue;
        }
        out.push(ch);
        last_sep = is_sep;
    }
    if out.len() > 1 && out.ends_with(SEP) {
        out.pop();
    }
    out
}

/// The last segment of a remote path, for `DirEntry::name`.
pub fn base_name(path: &str) -> String {
    let normalised = normalise(path);
    match normalised.rsplit_once(SEP) {
        Some((_, name)) if !name.is_empty() => name.to_string(),
        _ => normalised,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sibling_with_a_shared_prefix_is_outside() {
        let root = RemoteRoot::new("/srv/app").unwrap();
        assert!(root.contains("/srv/app"));
        assert!(root.contains("/srv/app/src/main.rs"));
        assert!(!root.contains("/srv/appdata"));
        assert!(!root.contains("/srv"));
        assert!(!root.contains("/etc/passwd"));
    }

    #[test]
    fn relative_paths_hang_off_the_root() {
        let root = RemoteRoot::new("/srv/app/").unwrap();
        assert_eq!(root.root(), "/srv/app");
        assert_eq!(root.candidate("src"), "/srv/app/src");
        assert_eq!(root.candidate("/etc"), "/etc");
        assert_eq!(root.candidate(""), "/srv/app");
    }

    #[test]
    fn escaping_is_reported_against_the_requested_path() {
        let root = RemoteRoot::new("/srv/app").unwrap();
        let err = root.ensure("../../etc", "/etc".to_string()).unwrap_err();
        assert!(matches!(err, TransportError::PathEscapesRoot { path } if path == "../../etc"));
    }

    #[test]
    fn normalise_collapses_and_trims() {
        assert_eq!(normalise("/srv//app/"), "/srv/app");
        assert_eq!(normalise("/"), "/");
        assert_eq!(base_name("/srv/app/main.rs"), "main.rs");
        assert_eq!(base_name("/"), "/");
    }
}
