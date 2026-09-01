//! Argv for the stash calls.
//!
//! Two caller values reach git here, and they are safe for different reasons.
//!
//! An **index** is a `u32`. It is formatted into `stash@{N}` by this file, so
//! there is no string from the caller in it at all - the strongest form the
//! rule takes, and the reason [`crate::types::GitStash::index`] is a number
//! rather than the `stash@{0}` text git printed.
//!
//! A **message** is text, and unlike a commit message it has no stdin form:
//! `git stash push -m` takes it in argv. Locally that is an argv element and
//! nothing parses it. Over SSH the argv becomes a command line and
//! `ssh::command::quote` *refuses* a value containing a single quote, so a
//! stash message with an apostrophe is a typed error on a remote target. That
//! is the documented limit of the SSH transport rather than a silent
//! difference; see `docs/mino-workbench/git-module.md`.

use super::GLOBAL;

/// Fields every stash row is read with.
///
/// - `%gd` is the selector git itself printed - `stash@{0}` - and is where the
///   index comes from. Read rather than assumed from the row's position,
///   because the position and the selector are two different facts and only
///   one of them is what a later `drop` will be given.
/// - `%gs` is the reflog subject: `WIP on main: 3f2a1c9 first`, or
///   `On main: <message>` when the user wrote one. Splitting it is
///   [`crate::git::stash`]'s job.
/// - `%at` is seconds; everything on this interface is milliseconds, and the
///   parser converts.
pub const STASH_FORMAT: &str = "--format=%gd%x1f%gs%x1f%at";

/// `git stash list -z`. `-z` for the same reason status uses it: a message is
/// a caller value and a NUL is the one byte it cannot contain.
pub fn stash_list_argv() -> Vec<String> {
    let mut argv = owned(GLOBAL);
    argv.extend(owned(&["stash", "list", "-z"]));
    argv.push(STASH_FORMAT.to_string());
    argv
}

/// `git stash push [--include-untracked] [-m <message>]`.
///
/// `push`, not the deprecated `save`: `save` reads its trailing arguments as a
/// message, which would make a message beginning with a dash into a flag.
/// `push` takes `-m` explicitly and stops there.
pub fn stash_push_argv(message: Option<&str>, include_untracked: bool) -> Vec<String> {
    let mut argv = owned(&["stash", "push"]);
    if include_untracked {
        argv.push("--include-untracked".to_string());
    }
    if let Some(message) = message {
        argv.push("-m".to_string());
        argv.push(message.to_string());
    }
    argv
}

/// `git stash apply|pop stash@{N}`.
///
/// `pop` is apply-and-drop, and the difference is worth a parameter rather
/// than two functions: an apply that conflicts leaves the entry on the stack,
/// and a pop that conflicts does too. Which one the caller asked for is the
/// only thing that differs.
pub fn stash_apply_argv(index: u32, pop: bool) -> Vec<String> {
    vec![
        "stash".to_string(),
        if pop { "pop" } else { "apply" }.to_string(),
        selector(index),
    ]
}

/// `git stash drop stash@{N}`. Destructive: what it removes is reachable only
/// through the reflog afterwards, which is not something this app offers.
pub fn stash_drop_argv(index: u32) -> Vec<String> {
    vec!["stash".to_string(), "drop".to_string(), selector(index)]
}

/// The only place `stash@{…}` is written. A number goes in, so nothing a
/// caller typed comes out.
pub fn selector(index: u32) -> String {
    format!("stash@{{{index}}}")
}

fn owned<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    args.iter().map(|arg| arg.as_ref().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_is_named_by_a_number_and_never_by_caller_text() {
        assert_eq!(selector(0), "stash@{0}");
        assert_eq!(stash_drop_argv(3), vec!["stash", "drop", "stash@{3}"]);
    }

    #[test]
    fn apply_and_pop_differ_by_one_word() {
        assert_eq!(stash_apply_argv(1, false)[1], "apply");
        assert_eq!(stash_apply_argv(1, true)[1], "pop");
    }

    #[test]
    fn a_message_follows_an_explicit_flag() {
        // `push -m`, never `save <message>`: under `save` a message starting
        // with a dash would be read as a flag.
        let argv = stash_push_argv(Some("-oops"), false);
        let flag = argv.iter().position(|a| a == "-m").unwrap();
        assert_eq!(argv[flag + 1], "-oops");
        assert!(!argv.contains(&"save".to_string()));
    }

    #[test]
    fn untracked_is_a_flag_and_never_a_default() {
        assert!(!stash_push_argv(None, false).contains(&"--include-untracked".to_string()));
        assert!(stash_push_argv(None, true).contains(&"--include-untracked".to_string()));
    }
}
