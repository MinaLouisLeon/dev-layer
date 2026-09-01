//! The Mino Workbench, hosted inside dev-layer.
//!
//! The file tree and viewer are served by `mino-core`, vendored from
//! github.com/MinaLouisLeon/mino-workbench (MIT). Nothing here touches the
//! filesystem directly: every call goes through that crate's `Transport`, so
//! the same panel will serve an SSH host the day the remote target is wired up
//! — and, more immediately, so its path guard applies. A session is confined
//! to the folder it was opened on.

use std::sync::Arc;

use mino_core::types::{ConnectionTarget, DirEntry, FilePayload, ReadFileOptions};
use mino_core::{transport_for, Transport};
use parking_lot::Mutex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchState {
    /// Folder the session is rooted at, or `None` before one is opened.
    pub root: Option<String>,
}

struct Session {
    transport: Arc<dyn Transport>,
    root: String,
}

#[derive(Default)]
pub struct Workbench {
    session: Mutex<Option<Session>>,
}

impl Workbench {
    pub fn state(&self) -> WorkbenchState {
        WorkbenchState {
            root: self.session.lock().as_ref().map(|s| s.root.clone()),
        }
    }

    /// Roots a session at `root`. Re-opening replaces the session, which is
    /// how the path guard follows the user to another folder.
    pub async fn open(&self, root: String) -> Result<WorkbenchState, String> {
        let target = ConnectionTarget::Local { root: root.clone() };
        let transport = transport_for(&target);

        transport
            .connect(&target)
            .await
            .map_err(|e| format!("could not open {root}: {e}"))?;

        *self.session.lock() = Some(Session {
            transport,
            root: root.clone(),
        });
        tracing::info!(root, "workbench opened");
        Ok(WorkbenchState { root: Some(root) })
    }

    pub async fn list_dir(&self, path: String) -> Result<Vec<DirEntry>, String> {
        self.transport()?
            .list_dir(&path)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn read_file(&self, path: String) -> Result<FilePayload, String> {
        let options = ReadFileOptions {
            max_bytes: None,
            allow_binary: false,
        };
        self.transport()?
            .read_file(&path, options)
            .await
            .map_err(|e| e.to_string())
    }

    /// The transport is cloned out of the lock so it is never held across an
    /// await point.
    fn transport(&self) -> Result<Arc<dyn Transport>, String> {
        self.session
            .lock()
            .as_ref()
            .map(|session| session.transport.clone())
            .ok_or_else(|| "no folder is open in the workbench".to_string())
    }
}
