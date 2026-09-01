//! Push, against a **local bare repository** used as a remote.
//!
//! Split from `git_remote.rs` because it is the half with something to lose.
//! A fetch changes no file and a pull is refused over uncommitted work; a
//! push changes what other people see, and a *force* push can remove commits
//! that exist nowhere else.
//!
//! So the two assertions that matter here are about refusals. A
//! non-fast-forward push must report that **nothing was pushed** and name the
//! fix, and a force push must be refused while the remote is somewhere this
//! repository has never looked - which is `--force-with-lease` doing exactly
//! the job it was chosen for.

use mino_core::types::{GitPushOutcome, PushRequest};
use mino_core::GitRemoteTransport;

mod fixture;

use fixture::git::{commit_file, connected, git, git_available};
use fixture::git_remote::{second_clone, with_remote};

#[tokio::test]
async fn a_push_sends_the_branch_and_reports_it() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    commit_file(&work, "readme.md", "# mine\n", "mine");

    let transport = connected(&work).await;
    let result = transport.push(PushRequest::default()).await.expect("push");

    assert_eq!(result.remote, "origin");
    assert_eq!(result.branch, "main");
    assert_eq!(result.outcome, GitPushOutcome::Pushed);
    assert!(!result.forced);

    // And the remote really has it, which is the half a result cannot prove.
    let other = second_clone(dir.path(), "other");
    assert_eq!(
        std::fs::read_to_string(other.join("readme.md")).unwrap(),
        "# mine\n"
    );
    drop(dir);
}

#[tokio::test]
async fn a_push_with_nothing_to_send_says_so() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let transport = connected(&work).await;

    let result = transport.push(PushRequest::default()).await.expect("push");
    assert_eq!(result.outcome, GitPushOutcome::AlreadyUpToDate);
    drop(dir);
}

#[tokio::test]
async fn a_non_fast_forward_push_is_a_typed_error_naming_the_reason() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    // Somebody else pushes, and this clone is now behind.
    let other = second_clone(dir.path(), "other");
    commit_file(&other, "readme.md", "# theirs\n", "theirs");
    git(&other, &["push", "origin", "main"]);
    // And this clone commits something of its own on top of the old tip.
    commit_file(&work, "readme.md", "# mine\n", "mine");

    let transport = connected(&work).await;
    let refusal = transport.push(PushRequest::default()).await.unwrap_err();
    let sentence = refusal.to_string();

    assert!(sentence.contains("Fetch and pull first"), "{sentence}");
    assert!(sentence.contains("Nothing was pushed"), "{sentence}");

    // And nothing was: the remote still has theirs.
    let third = second_clone(dir.path(), "third");
    assert_eq!(
        std::fs::read_to_string(third.join("readme.md")).unwrap(),
        "# theirs\n"
    );
    drop(dir);
}

#[tokio::test]
async fn a_force_push_replaces_the_remote_branch() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    let other = second_clone(dir.path(), "other");
    commit_file(&other, "readme.md", "# theirs\n", "theirs");
    git(&other, &["push", "origin", "main"]);
    commit_file(&work, "readme.md", "# mine\n", "mine");

    let transport = connected(&work).await;
    // The lease has to be refreshed first, exactly as it does in life:
    // `--force-with-lease` refuses while the remote is somewhere this
    // repository has never seen. That refusal is the protection working.
    let leased = transport
        .push(PushRequest {
            force: true,
            ..PushRequest::default()
        })
        .await;
    assert!(leased.is_err(), "a stale lease must refuse");

    transport.fetch(None).await.expect("fetch");
    let result = transport
        .push(PushRequest {
            force: true,
            ..PushRequest::default()
        })
        .await
        .expect("force push after fetching");
    assert!(result.forced);

    let third = second_clone(dir.path(), "third");
    assert_eq!(
        std::fs::read_to_string(third.join("readme.md")).unwrap(),
        "# mine\n"
    );
    drop(dir);
}

#[tokio::test]
async fn a_push_on_a_detached_head_says_there_is_no_branch() {
    if !git_available() {
        return;
    }
    let (dir, work) = with_remote();
    git(&work, &["checkout", "--detach"]);

    let transport = connected(&work).await;
    let refusal = transport.push(PushRequest::default()).await.unwrap_err();
    assert!(refusal.to_string().contains("no branch checked out"));
    drop(dir);
}
