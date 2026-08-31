//! Escape hatches.
//!
//! dev-layer mutates the live desktop (hides the taskbar now; restyles and
//! moves other apps' windows from milestone 4 on). Every such mutation must be
//! undoable, and the undo must run even when we crash — otherwise a panic
//! leaves the user staring at a desktop with no taskbar and misplaced windows.
//!
//! Contract: anything that changes system state registers its inverse here,
//! immediately after the change succeeds. [`run_all`] is idempotent and runs
//! on normal exit, on Ctrl-C, on panic, and from the global exit hotkey.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;

type Action = Box<dyn FnOnce() + Send + 'static>;

#[derive(Default)]
pub struct Teardown {
    actions: Mutex<Vec<(&'static str, Action)>>,
    ran: AtomicBool,
}

static TEARDOWN: OnceLock<Arc<Teardown>> = OnceLock::new();

fn teardown() -> &'static Arc<Teardown> {
    TEARDOWN.get_or_init(|| Arc::new(Teardown::default()))
}

/// Registers an undo action. `name` is logged so a partial teardown is diagnosable.
pub fn register<F>(name: &'static str, undo: F)
where
    F: FnOnce() + Send + 'static,
{
    teardown().actions.lock().push((name, Box::new(undo)));
    tracing::debug!(name, "teardown action registered");
}

/// Runs every registered action in reverse order. Safe to call repeatedly and
/// from any thread; later calls are no-ops.
pub fn run_all() {
    let t = teardown();
    if t.ran.swap(true, Ordering::SeqCst) {
        return;
    }

    // Take the whole list first: an action must never deadlock by registering
    // during teardown.
    let actions = std::mem::take(&mut *t.actions.lock());
    tracing::info!(count = actions.len(), "restoring desktop state");

    for (name, action) in actions.into_iter().rev() {
        // One failing restore must not strand the rest.
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
            tracing::error!(name, ?e, "teardown action panicked");
        } else {
            tracing::debug!(name, "teardown action done");
        }
    }
}

/// Installs the panic and Ctrl-C paths. Call once, before any state is mutated.
pub fn install_hooks() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(%info, "panic — restoring desktop before unwinding");
        run_all();
        previous(info);
    }));

    if let Err(e) = ctrlc::set_handler(|| {
        tracing::info!("interrupt received");
        run_all();
        std::process::exit(0);
    }) {
        tracing::warn!(error = %e, "could not install interrupt handler");
    }
}
