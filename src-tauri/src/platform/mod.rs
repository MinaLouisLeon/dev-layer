//! Platform abstraction.
//!
//! Every OS-specific call lives behind `sys`. Windows gets the real Win32
//! implementation; other targets get a stub with a synthetic monitor, so the
//! HUD frontend can be built and iterated on from any machine.

#[cfg(windows)]
pub mod win;
#[cfg(windows)]
pub use win as sys;

#[cfg(not(windows))]
pub mod stub;
#[cfg(not(windows))]
pub use stub as sys;
