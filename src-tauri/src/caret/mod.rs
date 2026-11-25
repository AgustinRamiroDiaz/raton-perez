//! Finds where the user is typing in another app, so the writing-tools popup
//! can open beside the text instead of mid-screen.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::{enable_accessibility, locate};
#[cfg(target_os = "macos")]
pub use macos::locate;

/// The caret's rectangle, or the focused field's when the app won't say where
/// the caret is. Global coordinates with a top-left origin: points on macOS,
/// physical pixels on Linux (X11).
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Apps without a real answer report zeros.
fn plausible(a: &Anchor) -> bool {
    [a.x, a.y, a.width, a.height].iter().all(|v| v.is_finite())
        && a.height > 0.0
        && !(a.x == 0.0 && a.y == 0.0)
}
