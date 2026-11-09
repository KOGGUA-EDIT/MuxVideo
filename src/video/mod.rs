//! Native video surfaces and window plumbing.

pub mod childwin;

pub use childwin::{InputEvent, MouseEvent, child_windows, take_input_events};
