//! Opening a local session, and pinning the root every later path is checked
//! against.
//!
//! Split out of `transport_impl.rs` when the GitHub surface arrived and that
//! file reached its ceiling. It is the right thing to move: that file says of
//! itself that it holds no logic beyond dispatch, and this - canonicalising a
//! root, deriving a label, taking the write lock - was the one place it did.

use crate::error::{Result, TransportError};
use crate::types::{ConnectionInfo, ConnectionTarget, TransportKind};

use super::roots::RootGuard;
use super::LocalTransport;

/// Canonicalises `target`'s root, makes it the session's, and describes it.
///
/// Calling it twice re-roots the transport, which is what changing folder
/// does. The guard replaces whatever was there rather than being added
/// beside it, so there is never more than one root a path could be inside.
pub fn connect(transport: &LocalTransport, target: &ConnectionTarget) -> Result<ConnectionInfo> {
    let ConnectionTarget::Local { root } = target else {
        return Err(TransportError::invalid(
            "the local transport only accepts a local target",
        ));
    };
    let guard = RootGuard::new(root)?;
    let display = guard.root_display();
    let label = guard
        .root()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| display.clone());

    *transport
        .root
        .write()
        .map_err(|_| TransportError::io("the connection lock was poisoned"))? = Some(guard);

    Ok(ConnectionInfo {
        id: uuid::Uuid::new_v4().to_string(),
        kind: TransportKind::Local,
        root: display,
        label: format!("{label} (local)"),
    })
}
