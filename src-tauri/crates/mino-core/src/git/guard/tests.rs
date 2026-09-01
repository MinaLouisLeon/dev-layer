//! What the mutating path guard accepts and what it refuses.
//!
//! The deleted-file case is the one that shaped the guard: `RootGuard`
//! canonicalises, and a syscall cannot answer for a path that is not there -
//! yet staging a deleted file is half of what a source control panel is for.

use super::*;

const POSIX: PathStyle = PathStyle {
    separator: '/',
    case_insensitive: false,
};
const WINDOWS: PathStyle = PathStyle {
    separator: '\\',
    case_insensitive: true,
};

fn guard(root: &str, path: &str, style: PathStyle) -> Result<String> {
    guard_one(root, path, style)
}

#[test]
fn a_path_inside_the_root_comes_back_root_relative() {
    assert_eq!(
        guard("/srv/app", "/srv/app/src/main.rs", POSIX).unwrap(),
        "src/main.rs"
    );
    // Windows keeps its casing, and loses its backslashes: git takes
    // forward slashes on every platform.
    assert_eq!(
        guard(r"C:\Repo", r"C:\Repo\src\Main.rs", WINDOWS).unwrap(),
        "src/Main.rs"
    );
}

#[test]
fn a_deleted_file_is_still_guardable() {
    // The case `RootGuard` cannot serve: nothing is on disk at this path,
    // and staging its deletion is exactly what the panel is for.
    assert_eq!(
        guard("/srv/app", "/srv/app/gone.rs", POSIX).unwrap(),
        "gone.rs"
    );
}

#[test]
fn traversal_is_refused_rather_than_resolved() {
    for path in ["/srv/app/../etc/passwd", "/srv/app/./x", "/srv/app/a/../b"] {
        assert!(
            matches!(
                guard("/srv/app", path, POSIX),
                Err(TransportError::PathEscapesRoot { .. })
            ),
            "{path} should be refused"
        );
    }
}

#[test]
fn a_path_outside_the_root_is_refused() {
    for path in ["/etc/passwd", "/srv/appdata/x", "/srv"] {
        assert!(matches!(
            guard("/srv/app", path, POSIX),
            Err(TransportError::PathEscapesRoot { .. })
        ));
    }
}

#[test]
fn the_root_itself_is_not_a_path_for_these_calls() {
    assert!(guard("/srv/app", "/srv/app", POSIX).is_err());
    assert!(guard("/srv/app", "  ", POSIX).is_err());
}

#[test]
fn a_batch_is_all_or_nothing() {
    let paths = vec![
        "/srv/app/ok.rs".to_string(),
        "/etc/passwd".to_string(),
        "/srv/app/also-ok.rs".to_string(),
    ];
    assert!(guard_paths("/srv/app", &paths, POSIX).is_err());
    assert_eq!(
        guard_paths("/srv/app", &paths[..1], POSIX).unwrap(),
        vec!["ok.rs".to_string()]
    );
}
