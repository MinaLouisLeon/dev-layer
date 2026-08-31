//! Monitor topology: the spine of the multi-monitor model.
//!
//! Everything downstream (HUD windows, reserved regions, window-manager
//! layouts) is keyed by [`MonitorInfo::id`], which stays stable across
//! hot-plugs and reboots because it is the OS display device name.

mod watcher;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::geometry::Rect;
use crate::platform::sys;

pub use watcher::{spawn_watcher, MONITORS_CHANGED_EVENT};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    /// OS display device name, e.g. `\\.\DISPLAY1`. Stable identity.
    pub id: String,
    /// Human-readable adapter/monitor description.
    pub name: String,
    pub is_primary: bool,
    /// Position in left-to-right, top-to-bottom order. Recomputed on every
    /// topology change, so never persist it — persist `id`.
    pub index: usize,
    /// Full monitor rect, virtual-desktop physical pixels.
    pub bounds: Rect,
    /// Monitor rect minus taskbar and appbars.
    pub work_area: Rect,
    /// Physical pixels per logical pixel. Mixed-DPI setups differ per monitor.
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorChange {
    pub added: Vec<MonitorInfo>,
    /// Ids only — the monitor is gone, there is nothing left to describe.
    pub removed: Vec<String>,
    /// Same id, different geometry/DPI/primary flag (resolution change,
    /// rearrangement, display scaling change).
    pub changed: Vec<MonitorInfo>,
}

impl MonitorChange {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// Sorts left-to-right then top-to-bottom and assigns `index`.
fn normalize(mut monitors: Vec<MonitorInfo>) -> Vec<MonitorInfo> {
    monitors.sort_by_key(|m| (m.bounds.x, m.bounds.y));
    for (i, m) in monitors.iter_mut().enumerate() {
        m.index = i;
    }
    monitors
}

pub fn diff(old: &[MonitorInfo], new: &[MonitorInfo]) -> MonitorChange {
    let mut change = MonitorChange::default();

    for n in new {
        match old.iter().find(|o| o.id == n.id) {
            None => change.added.push(n.clone()),
            Some(o) if o != n => change.changed.push(n.clone()),
            Some(_) => {}
        }
    }
    for o in old {
        if !new.iter().any(|n| n.id == o.id) {
            change.removed.push(o.id.clone());
        }
    }
    change
}

/// The current topology snapshot. Read by IPC commands and the HUD manager.
#[derive(Default)]
pub struct MonitorRegistry {
    inner: RwLock<Vec<MonitorInfo>>,
}

impl MonitorRegistry {
    pub fn snapshot(&self) -> Vec<MonitorInfo> {
        self.inner.read().clone()
    }

    pub fn get(&self, id: &str) -> Option<MonitorInfo> {
        self.inner.read().iter().find(|m| m.id == id).cloned()
    }

    /// Re-enumerates and swaps in the new topology, returning what moved.
    pub fn refresh(&self) -> Result<MonitorChange> {
        let next = normalize(sys::enumerate_monitors()?);
        let mut guard = self.inner.write();
        let change = diff(&guard, &next);
        *guard = next;
        Ok(change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: &str, x: i32) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            name: id.into(),
            is_primary: x == 0,
            index: 0,
            bounds: Rect {
                x,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x,
                y: 0,
                width: 1920,
                height: 1040,
            },
            scale_factor: 1.0,
        }
    }

    #[test]
    fn indexes_left_to_right() {
        let m = normalize(vec![mon("c", 3840), mon("a", 0), mon("b", 1920)]);
        assert_eq!(
            m.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(m[2].index, 2);
    }

    #[test]
    fn diff_reports_add_remove_and_geometry_change() {
        let old = vec![mon("a", 0), mon("b", 1920)];
        let mut moved = mon("b", 2560);
        moved.name = "b".into();
        let new = vec![mon("a", 0), moved, mon("c", 5000)];

        let d = diff(&old, &new);
        assert_eq!(d.added.len(), 1);
        assert_eq!(d.added[0].id, "c");
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0].id, "b");
        assert!(d.removed.is_empty());

        let d = diff(&new, &old);
        assert_eq!(d.removed, vec!["c"]);
    }

    #[test]
    fn identical_topology_is_no_change() {
        let m = vec![mon("a", 0), mon("b", 1920)];
        assert!(diff(&m, &m).is_empty());
    }
}
