//! Reading one commit: what it touched, what it changed, and who wrote each
//! line. Paging through the log itself is in `git_log.rs`.

use mino_core::types::{GitFileState, LogRequest};
use mino_core::{GitTransport, LocalTransport, Transport};

mod fixture;
use fixture::git::{connected, git, git_available, repository};

fn surface(transport: &LocalTransport) -> &dyn GitTransport {
    transport.git().expect("the local transport always has git")
}

#[tokio::test]
async fn show_lists_the_files_a_commit_touched() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    std::fs::write(root.join("added.txt"), "new\n").unwrap();
    std::fs::remove_file(root.join("readme.md")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() { }\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "a mixed commit"]);
    let transport = connected(root).await;

    let head = surface(&transport)
        .log(LogRequest::new())
        .await
        .unwrap()
        .commits[0]
        .sha
        .clone();
    let detail = surface(&transport).show(&head).await.unwrap();

    assert_eq!(detail.commit.summary, "a mixed commit");
    let mut files: Vec<(&str, GitFileState)> = detail
        .files
        .iter()
        .map(|f| (f.relative_path.as_str(), f.state))
        .collect();
    // Git's own order is by path already, but sorting makes the assertion
    // independent of that rather than quietly depending on it.
    files.sort_by_key(|(path, _)| *path);
    assert_eq!(
        files,
        vec![
            ("added.txt", GitFileState::Added),
            ("readme.md", GitFileState::Deleted),
            ("src/main.rs", GitFileState::Modified),
        ]
    );
}

#[tokio::test]
async fn a_commits_own_diff_works_on_a_root_commit() {
    if !git_available() {
        return;
    }
    // The root commit has no parent, which is what `<sha>^!` handles and
    // `<sha>^..<sha>` does not.
    let dir = repository();
    let transport = connected(dir.path()).await;
    let log = surface(&transport).log(LogRequest::new()).await.unwrap();
    let root_commit = log.commits.last().unwrap().sha.clone();

    let diff = surface(&transport)
        .commit_diff(&root_commit, None)
        .await
        .unwrap();
    let paths: Vec<&str> = diff
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    assert!(paths.contains(&"src/main.rs"), "{paths:?}");
}

#[tokio::test]
async fn blame_attributes_lines_to_the_commits_that_introduced_them() {
    if !git_available() {
        return;
    }
    let dir = repository();
    let root = dir.path();
    git(root, &["config", "user.name", "Second Author"]);
    std::fs::write(root.join("src/main.rs"), "fn main() {}\nfn second() {}\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "add second"]);
    let transport = connected(root).await;

    let blame = surface(&transport)
        .blame(&root.join("src/main.rs").to_string_lossy())
        .await
        .unwrap();

    assert_eq!(blame.lines.len(), 2);
    assert!(!blame.truncated);
    assert_eq!(blame.lines[0].line, 1);
    assert_eq!(blame.lines[0].author, "Test");
    assert_eq!(blame.lines[1].line, 2);
    assert_eq!(blame.lines[1].author, "Second Author");
    assert_eq!(blame.lines[1].summary, "add second");
    // Each line knows its own commit, and the short sha is a prefix of it.
    assert_ne!(blame.lines[0].sha, blame.lines[1].sha);
    assert!(blame.lines[1].sha.starts_with(&blame.lines[1].short_sha));
}
