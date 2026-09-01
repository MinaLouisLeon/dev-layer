//! `impl Transport for LocalTransport`.
//!
//! Kept apart from `mod.rs` so neither file grows past the project's file
//! ceiling. There is no logic here beyond dispatch, guard resolution and the
//! shell choice; the work lives in the sibling modules.

use async_trait::async_trait;

use crate::error::{Result, TransportError};
use crate::shell;
use crate::transport::{GitHubTransport, GitTransport, Transport};
use crate::types::{
    ConnectionInfo, ConnectionTarget, DirEntry, FilePayload, PtySessionId, PtySize, PtySpawnSpec,
    PtyStream, ReadFileOptions, SearchHits, SearchQuery, ShellKind, ShellProbe, StructuredOutput,
    StructuredRequest, TransportKind, WriteRequest,
};

use super::pty::SpawnRequest;
use super::roots;
use super::{connect, fs, read, search, structured, write, LocalTransport};

#[async_trait]
impl Transport for LocalTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Local
    }

    async fn connect(&self, target: &ConnectionTarget) -> Result<ConnectionInfo> {
        connect::connect(self, target)
    }

    async fn disconnect(&self) -> Result<()> {
        self.ptys.close_all();
        if let Ok(mut root) = self.root.write() {
            *root = None;
        }
        Ok(())
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let guard = self.guard()?;
        let dir = roots::display_path(&guard.resolve(path)?);
        match self.list_via_nu(&dir).await {
            Ok(entries) => Ok(entries),
            Err(err) => {
                tracing::debug!(%err, "structured listing failed, using the filesystem");
                fs::list_dir(&guard, &dir)
            }
        }
    }

    async fn stat(&self, path: &str) -> Result<DirEntry> {
        fs::stat(&self.guard()?, path)
    }

    async fn search_files(&self, query: SearchQuery) -> Result<SearchHits> {
        search::search(self.guard()?, query).await
    }

    async fn read_file(&self, path: &str, options: ReadFileOptions) -> Result<FilePayload> {
        read::read_file(&self.guard()?, path, options)
    }

    async fn write_file(&self, path: &str, request: WriteRequest) -> Result<DirEntry> {
        let guard = self.guard()?;
        write::write_file(&guard, path, request)
    }

    async fn open_pty(&self, spec: PtySpawnSpec) -> Result<PtyStream> {
        let guard = self.guard()?;
        let cwd = match &spec.cwd {
            Some(cwd) => roots::display_path(&guard.resolve(cwd)?),
            None => guard.root_display(),
        };
        let probe = shell::probe();
        let (program, kind) = match probe.nu_path {
            Some(nu) => (nu, ShellKind::Nu),
            None => (probe.fallback_program, ShellKind::Fallback),
        };
        self.ptys.open(SpawnRequest {
            fell_back: kind == ShellKind::Fallback,
            program,
            shell: kind,
            cwd,
            size: spec.size,
        })
    }

    async fn write_pty(&self, id: &PtySessionId, data: &str) -> Result<()> {
        self.ptys.write(id, data)
    }

    async fn resize_pty(&self, id: &PtySessionId, size: PtySize) -> Result<()> {
        self.ptys.resize(id, size)
    }

    async fn close_pty(&self, id: &PtySessionId) -> Result<()> {
        self.ptys.close(id)
    }

    async fn run_structured(&self, request: StructuredRequest) -> Result<StructuredOutput> {
        let guard = self.guard()?;
        let nu = shell::find_nu()
            .ok_or_else(|| TransportError::shell("nushell is not installed on this machine"))?;
        // A cwd outside the connected root is refused like any other path.
        let cwd = match request.cwd.as_deref() {
            Some(cwd) => roots::display_path(&guard.resolve(cwd)?),
            None => guard.root_display(),
        };
        let request = StructuredRequest {
            cwd: Some(cwd),
            ..request
        };
        structured::run(&nu, &request).await
    }

    async fn probe_shell(&self) -> Result<ShellProbe> {
        Ok(shell::probe())
    }

    /// Always present: whether *this folder* is a repository is
    /// `GitTransport::repository`'s question, and whether git is installed at
    /// all is answered by the first call rather than by hiding the surface.
    fn git(&self) -> Option<&dyn GitTransport> {
        Some(self)
    }

    /// Present on the same terms as `git`: whether `gh` is installed, signed
    /// in, and pointed at a GitHub remote are all `GitHubTransport::probe`'s
    /// questions, and hiding the surface would answer three at once.
    fn github(&self) -> Option<&dyn GitHubTransport> {
        Some(self)
    }
}
