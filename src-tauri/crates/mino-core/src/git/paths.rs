//! Turning git's answers into paths the rest of the app already speaks.
//!
//! Git always reports repository-relative paths with forward slashes. The tree
//! and the viewer address files by absolute path in the *target's* separator
//! style, so something has to translate - and that something must not guess,
//! because the client and the target are not always the same platform. A
//! Windows client browsing a Linux host over SSH is the ordinary case, not the
//! exotic one.
//!
//! [`PathStyle`] is that decision, made once by each transport and carried
//! into the shared code, rather than a `cfg!(windows)` sprinkled through it.

/// How one target writes and compares paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathStyle {
    pub separator: char,
    /// Whether two paths differing only in case name the same file. True on
    /// Windows, false on the POSIX hosts reached over SSH.
    pub case_insensitive: bool,
}

impl PathStyle {
    /// The style of the machine this process is running on.
    pub fn local() -> Self {
        Self {
            separator: std::path::MAIN_SEPARATOR,
            case_insensitive: cfg!(windows),
        }
    }

    /// Remote hosts are POSIX regardless of what the client runs on - the same
    /// assumption `ssh::roots` already makes.
    pub fn posix() -> Self {
        Self {
            separator: '/',
            case_insensitive: false,
        }
    }

    /// Rewrites a path in this style: the target's separator throughout, and
    /// no trailing one. Needed because git answers in forward slashes on every
    /// platform - `rev-parse --show-toplevel` returns `C:/repo` on Windows -
    /// while the tree and the viewer address files the way the OS writes them.
    pub fn normalise(&self, path: &str) -> String {
        let separator = self.separator.to_string();
        let swapped = path.replace(['/', '\\'], &separator);
        let trimmed = swapped.trim_end_matches(self.separator);
        if trimmed.is_empty() {
            swapped
        } else {
            trimmed.to_string()
        }
    }

    /// Joins a repository-relative path onto a root, in this style.
    pub fn absolute(&self, root: &str, relative: &str) -> String {
        let root = self.normalise(root);
        if relative.is_empty() {
            return root;
        }
        let relative = relative.replace('/', &self.separator.to_string());
        format!("{root}{}{relative}", self.separator)
    }

    /// True when `path` is `root` or sits beneath it.
    ///
    /// A string test, deliberately: this rules on deleted files too, and
    /// `canonicalize` has nothing to say about a path that no longer exists.
    /// Containment against the *session* root is still enforced by each
    /// transport's own guard before any syscall - this only decides which of
    /// git's rows the caller is allowed to see.
    pub fn within(&self, root: &str, path: &str) -> bool {
        let root = self.comparable(root);
        let path = self.comparable(path);
        if path == root {
            return true;
        }
        // The trailing separator is what stops root `/srv/app` swallowing
        // `/srv/appdata`.
        path.starts_with(&format!("{root}/"))
    }

    /// One normalised form for comparison: forward slashes, no trailing
    /// separator, and folded case where the target folds it.
    fn comparable(&self, path: &str) -> String {
        let normalised = path.replace('\\', "/");
        let trimmed = normalised.trim_end_matches('/');
        let owned = if trimmed.is_empty() {
            normalised.as_str()
        } else {
            trimmed
        };
        if self.case_insensitive {
            owned.to_lowercase()
        } else {
            owned.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSIX: PathStyle = PathStyle {
        separator: '/',
        case_insensitive: false,
    };
    const WINDOWS: PathStyle = PathStyle {
        separator: '\\',
        case_insensitive: true,
    };

    #[test]
    fn normalise_puts_gits_forward_slashes_into_the_targets_style() {
        assert_eq!(WINDOWS.normalise("C:/repo/src/"), r"C:\repo\src");
        assert_eq!(POSIX.normalise("/srv/app/"), "/srv/app");
        assert_eq!(POSIX.normalise("/"), "/");
    }

    #[test]
    fn absolute_uses_the_targets_separator() {
        assert_eq!(
            POSIX.absolute("/srv/app", "src/main.rs"),
            "/srv/app/src/main.rs"
        );
        assert_eq!(
            WINDOWS.absolute("C:/repo", "src/main.rs"),
            r"C:\repo\src\main.rs"
        );
        assert_eq!(WINDOWS.absolute(r"C:\repo\", ""), r"C:\repo");
    }

    #[test]
    fn a_sibling_with_a_shared_prefix_is_outside() {
        assert!(POSIX.within("/srv/app", "/srv/app/src/main.rs"));
        assert!(POSIX.within("/srv/app", "/srv/app"));
        assert!(!POSIX.within("/srv/app", "/srv/appdata/x"));
        assert!(!POSIX.within("/srv/app", "/srv"));
    }

    #[test]
    fn case_folds_only_where_the_target_folds_it() {
        assert!(WINDOWS.within(r"C:\Repo", r"c:\repo\src\main.rs"));
        assert!(!POSIX.within("/srv/Repo", "/srv/repo/main.rs"));
    }
}
