//! Native panels: the tools worth having *inside* the HUD rather than as
//! another window to tile.
//!
//! The rule of thumb from the design discussion: heavyweight apps you cannot
//! replace (the IDE, a real browser) stay real windows and get tiled; small
//! tools you reach for constantly are cheaper and better as panels.

pub mod http;
pub mod terminal;

pub use http::{send_request, HttpRequest, HttpResponse, RequestHistory};
pub use terminal::{TerminalSessions, CLOSED_EVENT, OUTPUT_EVENT};
