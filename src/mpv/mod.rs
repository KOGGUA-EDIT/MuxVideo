//! libmpv integration.
//!
//! `lib` loads `libmpv-2.dll` at runtime, `player` owns one libmpv instance
//! bound to a native child window (the classic `wid` embedding path), and
//! `events` mirrors libmpv events onto a lock-free-ish queue polled by the UI.
//!
//! Contract (consumed by the app + the mpv probe binary):
//! * Players are created with a resolved `wid` (HWND as i64).
//! * `poll_events` drains events without blocking; property changes are cached
//!   inside the player so `time_pos` / `duration` / `paused` never block the UI.

pub mod events;
pub mod lib;
pub mod player;

pub use events::MpvEvent;
pub use lib::LibMpv;
pub use player::{MpvOptions, MpvPlayer};
