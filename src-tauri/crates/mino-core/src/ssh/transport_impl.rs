//! `impl Transport for SshTransport`.
//!
//! Split from `mod.rs` for the same reason the local transport is: the state
//! and the trait are two things to read, not one.

use async_trait::async_trait;

use crate::error::{Result, TransportError};
use crate::transport::{GitHubTransport, GitTransport, Transport};
use crate::types::{
    ConnectionInfo, ConnectionTarget, DirEntry, FilePayload, PtySessionId, PtySize, PtySpawnSpec,
    PtyStream, ReadFileOptions, SearchHits, SearchQuery, ShellProbe, StructuredOutput,
    StructuredRequest, TransportKind, WriteRequest,
};

use super::{connect, fs, pty_open, read, search, structured, write, SshTransport};

#[async_trait]
impl Transport for SshTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Ssh
    }

    async fn connect(&self, target: &ConnectionTarget) -> Result<ConnectionInfo> {
        // Changing the working folder is a re-connect to the same host, and it
        // happens whenever someone picks a directory in the workbench. Doing a
        // fresh handshake for that would re-authenticate - and ask the agent
        // again - for what is really a re-pin of the root, so the live
        // connection is reused when the endpoint has not changed.
        if let Some(info) = self.reroot(target).await? {
            return Ok(info);
        }

        // A different host: the old connection and everything running on it
        // goes away before the new one is opened.
        self.disconnect().await?;
        let (info, connected) = connect::establish(self.config.clone(), target).await?;
        *self.state.write().await = Some(connected);
        Ok(info)
    }

    async fn disconnect(&self) -> Result<()> {
        // Sessions first: closing them after dropping the connection would
        // leave the remote shells to time out on their own.
        self.ptys.close_all().await;
        if let Some(connected) = self.state.write().await.take() {
            let _ = connected.sftp.close().await;
            let _ = connected
                .handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        }
        Ok(())
    }

    async fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let connected = self.connected().await?;
        fs::list_dir(&connected.sftp, &connected.root, path).await
    }

    async fn stat(&self, path: &str) -> Result<DirEntry> {
        let connected = self.connected().await?;
        fs::stat(&connected.sftp, &connected.root, path).await
    }

    async fn search_files(&self, query: SearchQuery) -> Result<SearchHits> {
        let connected = self.connected().await?;
        search::search(&connected.sftp, &connected.root, &query).await
    }

    async fn read_file(&self, path: &str, options: ReadFileOptions) -> Result<FilePayload> {
        let connected = self.connected().await?;
        read::read_file(&connected.sftp, &connected.root, path, options).await
    }

    async fn write_file(&self, path: &str, request: WriteRequest) -> Result<DirEntry> {
        let connected = self.connected().await?;
        write::write_file(&connected.sftp, &connected.root, path, request).await
    }

    async fn open_pty(&self, spec: PtySpawnSpec) -> Result<PtyStream> {
        let connected = self.connected().await?;
        pty_open::open(&self.ptys, &connected, spec).await
    }

    async fn write_pty(&self, id: &PtySessionId, data: &str) -> Result<()> {
        let _connected = self.connected().await?;
        self.ptys.write(id, data).await
    }

    async fn resize_pty(&self, id: &PtySessionId, size: PtySize) -> Result<()> {
        let _connected = self.connected().await?;
        self.ptys.resize(id, size).await
    }

    async fn close_pty(&self, id: &PtySessionId) -> Result<()> {
        let _connected = self.connected().await?;
        self.ptys.close(id).await
    }

    async fn run_structured(&self, request: StructuredRequest) -> Result<StructuredOutput> {
        let connected = self.connected().await?;
        let nu =
            connected.shell.nu_path.as_deref().ok_or_else(|| {
                TransportError::shell("nushell is not installed on the remote host")
            })?;

        // A cwd is a caller value, so it goes through the root guard before it
        // can reach a command line.
        let cwd = match request.cwd.as_deref() {
            Some(requested) => {
                Some(fs::resolve(&connected.sftp, &connected.root, requested).await?)
            }
            None => Some(connected.root.root().to_string()),
        };
        let request = StructuredRequest { cwd, ..request };
        structured::run(&connected.handle, nu, &request).await
    }

    async fn probe_shell(&self) -> Result<ShellProbe> {
        let connected = self.connected().await?;
        Ok(connected.shell.clone())
    }

    /// The remote host's own git, over the exec channel. Present whether or
    /// not the host has git installed - the first call is what says so.
    fn git(&self) -> Option<&dyn GitTransport> {
        Some(self)
    }

    /// The remote host's own `gh`, over the same exec channel, authenticated
    /// with the remote account's own credentials. This machine's `gh` login
    /// is not involved and no token crosses the connection - which is the
    /// credential position holding up exactly as well at a distance.
    fn github(&self) -> Option<&dyn GitHubTransport> {
        Some(self)
    }
}
