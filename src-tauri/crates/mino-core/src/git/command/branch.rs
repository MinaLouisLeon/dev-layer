//! Argv for the branch calls.
//!
//! One caller value appears here - a branch name - and it arrives already
//! through [`crate::git::refname`], which has refused a leading `-` and asked
//! `git check-ref-format` whether git itself would take it.
//!
//! Every name is *also* placed where an option cannot be read: [`checkout_argv`]
//! and [`create_argv`] put an end-of-options `--` after the names, and
//! [`branch_write_argv`] puts one before them. Belt and braces on purpose -
//! the guard is the rule, and the argv shape is what makes a mistake in it
//! harmless rather than a way to run `--upload-pack`.

use crate::types::CreateBranchRequest;

use super::{GLOBAL, PATH_SEPARATOR};

/// Fields every branch row is read with, in one order, so nothing downstream
/// has to guess which column it is looking at.
///
/// `%1f` is the unit separator - a ref name cannot contain a control
/// character, and neither can an author name, so nothing in a record can be
/// mistaken for the separator between two of them. Rows are newline-separated,
/// which is safe for the same reason: `%(contents:subject)` is git's
/// single-line subject, not the whole message.
///
/// `%(symref)` is here only to be thrown away. `origin/HEAD` is a symbolic ref
/// pointing at another row already in the list, and offering it in a picker
/// would be offering the same branch twice under two names.
pub const BRANCH_FORMAT: &str = "--format=%(HEAD)%1f%(refname)%1f%(refname:short)%1f\
%(upstream:short)%1f%(upstream:track,nobracket)%1f%(symref)%1f\
%(objectname)%1f%(objectname:short)%1f%(contents:subject)%1f%(authorname)%1f%(authordate:unix)";

/// Every branch the picker can offer, local and remote in one call.
///
/// `--list --all` rather than two calls: the picker shows both together, and
/// two calls could answer from either side of a fetch.
pub fn branches_argv() -> Vec<String> {
    let mut argv = owned(GLOBAL);
    argv.extend(owned(&["branch", "--list", "--all"]));
    argv.push(BRANCH_FORMAT.to_string());
    argv
}

/// `git checkout <name> --`.
///
/// The trailing `--` is what stops a branch and a file of the same name being
/// ambiguous: with it, git knows the argument before it is a ref, so a working
/// tree containing a file called `main` cannot turn a branch switch into a
/// file restore. No `--no-optional-locks`: this call is *meant* to take the
/// index lock.
pub fn checkout_argv(name: &str) -> Vec<String> {
    vec![
        "checkout".to_string(),
        name.to_string(),
        PATH_SEPARATOR.to_string(),
    ]
}

/// `git checkout -b <name> [<from>] --`, or `git branch -- <name> [<from>]`.
///
/// Which one depends on `request.checkout`, and they are genuinely different
/// commands rather than a flag: creating a branch you stay off does not touch
/// the working tree at all, and should not go anywhere near a checkout.
pub fn create_argv(request: &CreateBranchRequest, name: &str) -> Vec<String> {
    if request.checkout {
        let mut argv = owned(&["checkout", "-b"]);
        argv.push(name.to_string());
        if let Some(from) = &request.from {
            argv.push(from.clone());
        }
        argv.push(PATH_SEPARATOR.to_string());
        return argv;
    }
    let mut argv = owned(&["branch", PATH_SEPARATOR]);
    argv.push(name.to_string());
    if let Some(from) = &request.from {
        argv.push(from.clone());
    }
    argv
}

/// `git branch -d|-D -- <name>`.
///
/// `-d` is the default and refuses a branch whose commits are nowhere else;
/// `-D` is what the UI sends only after saying what that means. The difference
/// is the whole safety of the operation, so it is a parameter and never a
/// default.
pub fn delete_argv(name: &str, force: bool) -> Vec<String> {
    branch_write_argv(if force { "-D" } else { "-d" }, name)
}

fn branch_write_argv(flag: &str, name: &str) -> Vec<String> {
    vec![
        "branch".to_string(),
        flag.to_string(),
        PATH_SEPARATOR.to_string(),
        name.to_string(),
    ]
}

fn owned<S: AsRef<str>>(args: &[S]) -> Vec<String> {
    args.iter().map(|arg| arg.as_ref().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checkout_names_the_ref_in_front_of_the_separator() {
        // Behind it, git would read `main` as a path and restore a file.
        let argv = checkout_argv("main");
        assert_eq!(argv, vec!["checkout", "main", "--"]);
    }

    #[test]
    fn creating_with_checkout_is_a_different_command_from_creating_without() {
        let request = CreateBranchRequest::new("feat").checkout(true);
        assert_eq!(
            create_argv(&request, "feat"),
            vec!["checkout", "-b", "feat", "--"]
        );

        let request = CreateBranchRequest::new("feat");
        assert_eq!(create_argv(&request, "feat"), vec!["branch", "--", "feat"]);
    }

    #[test]
    fn a_start_point_follows_the_name() {
        let request = CreateBranchRequest::new("feat").from("origin/main");
        assert_eq!(
            create_argv(&request, "feat"),
            vec!["branch", "--", "feat", "origin/main"]
        );
    }

    #[test]
    fn force_is_a_parameter_and_never_a_default() {
        assert!(delete_argv("gone", false).contains(&"-d".to_string()));
        assert!(delete_argv("gone", true).contains(&"-D".to_string()));
    }

    #[test]
    fn a_deleted_name_sits_behind_the_separator() {
        let argv = delete_argv("odd", false);
        let separator = argv.iter().position(|a| a == PATH_SEPARATOR).unwrap();
        let name = argv.iter().position(|a| a == "odd").unwrap();
        assert!(separator < name, "{argv:?}");
    }
}
