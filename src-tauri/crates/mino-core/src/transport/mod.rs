//! The interface every filesystem, PTY and shell call goes through.
//!
//! This trait is the architectural rule of the project. No UI component and no
//! Tauri command touches the filesystem or spawns a process; they call a
//! `Transport`. Three implementations exist so the interface is proven against
//! three shapes: [`crate::local`] (working), [`crate::ssh`] and
//! [`crate::remote`] (compiling, every method returns
//! [`TransportError::Unimplemented`]).
//!
//! Git and GitHub are the two things that are *not* on this trait. Git is a
//! second trait,
//! [`GitTransport`], reached through [`Transport::git`]. Twenty-five git
//! methods on one trait would make every implementation file and the stub
//! macro grow for reasons that have nothing to do with cohesion, and "is there
//! git here?" is better asked once, at the type level, than answered by every
//! method separately. See `plan/decisions.md` D2.
//!
//! GitHub is a third, [`GitHubTransport`], reached through
//! [`Transport::github`], and it is deliberately two methods rather than ten:
//! five features share one enumerated query. See `plan/phase-5-github.md`.
//!
//! The TypeScript client in `apps/ui/src/Types/modules/api.ts` mirrors this
//! trait one method for one method, and `client.git` and `client.github`
//! mirror the other two.
//! The only deviation is `open_pty`: Rust returns a [`PtyStream`] (descriptor +
//! channel), while TypeScript returns the descriptor and takes the stream
//! through `onPtyEvent`, because a channel cannot cross the IPC boundary.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{
    ConnectionInfo, ConnectionTarget, DirEntry, FilePayload, PtySessionId, PtySize, PtySpawnSpec,
    PtyStream, ReadFileOptions, SearchHits, SearchQuery, ShellProbe, StructuredOutput,
    StructuredRequest, WriteRequest,
};

mod git;
mod github;

pub use git::{
    GitBranchTransport, GitConflictTransport, GitRemoteTransport, GitStashTransport, GitTransport,
};
pub use github::GitHubTransport;

#[async_trait]
pub trait Transport: Send + Sync + 'static {
    fn kind(&self) -> crate::types::TransportKind;

    /// Opens the session and pins the root every later path is checked
    /// against. Calling it twice re-roots the transport.
    async fn connect(&self, target: &ConnectionTarget) -> Result<ConnectionInfo>;

    /// Closes every PTY session and drops the connection. Must be safe to
    /// call when not connected, and must leave no orphaned child processes.
    async fn disconnect(&self) -> Result<()>;

    /// One level only. The tree lazy-loads per folder; there is deliberately
    /// no recursive walk on this interface.
    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>>;

    async fn stat(&self, path: &str) -> Result<DirEntry>;

    /// Walks the connected root looking for names matching `query`.
    ///
    /// The counterpart to `list_dir`'s one level: this is the only method
    /// that descends. It is bounded rather than exhaustive - by a result
    /// limit, an entry cap and a wall-clock deadline, all in
    /// [`crate::search`] - and says so through [`SearchHits::truncated`],
    /// because a search that never returns is worse than a partial answer.
    ///
    /// Matching and ranking happen in [`crate::search::fuzzy`], not in the
    /// implementation, so every transport orders results identically.
    async fn search_files(&self, query: SearchQuery) -> Result<SearchHits>;

    /// Enforces the size ceiling and the binary sniff before reading, so an
    /// oversized or binary file never reaches the UI as content.
    async fn read_file(&self, path: &str, options: ReadFileOptions) -> Result<FilePayload>;

    /// Saves a file, and is the only way anything in this app writes to one.
    ///
    /// Subject to the same path guard as every read: a path outside the
    /// connected root is refused before a byte is written. Refuses to
    /// overwrite a file that changed since the editor loaded it - see
    /// [`WriteRequest::expected_modified_ms`] - and returns the entry as it
    /// now stands, so the caller can update its baseline.
    async fn write_file(&self, path: &str, request: WriteRequest) -> Result<DirEntry>;

    /// Spawns `nu`, or the platform default shell when `nu` is absent, with
    /// `PtySession::fell_back` set so the UI can say so.
    async fn open_pty(&self, spec: PtySpawnSpec) -> Result<PtyStream>;

    async fn write_pty(&self, id: &PtySessionId, data: &str) -> Result<()>;

    async fn resize_pty(&self, id: &PtySessionId, size: PtySize) -> Result<()>;

    async fn close_pty(&self, id: &PtySessionId) -> Result<()>;

    /// Non-interactive Nushell call returning parsed structured data. See
    /// [`StructuredRequest`] for the injection rules.
    async fn run_structured(&self, request: StructuredRequest) -> Result<StructuredOutput>;

    /// Reports whether `nu` is on the target's PATH and what would be spawned
    /// instead. Drives the fallback notice and the tree's degrade path.
    async fn probe_shell(&self) -> Result<ShellProbe>;

    /// The git surface for this session, or `None` where the target has no
    /// concept of one. Not an error, and not "this folder is not a
    /// repository" either - that question is [`GitTransport::repository`]'s.
    fn git(&self) -> Option<&dyn GitTransport>;

    /// The GitHub surface for this session, on the same terms as
    /// [`Transport::git`]: `None` where the target has no concept of one, and
    /// never an answer to "is there a GitHub repository here" - that question
    /// is [`GitHubTransport::probe`]'s, and it has four answers rather than
    /// two.
    ///
    /// A third trait rather than two more methods here, for the reason git is
    /// a second one. It is small - two methods, because five features share
    /// one enumerated query - and it holds a rule none of the others do:
    /// every call under it shells out to `gh`, so this application never
    /// holds a GitHub credential.
    fn github(&self) -> Option<&dyn GitHubTransport>;
}
