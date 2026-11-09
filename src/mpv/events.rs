//! Events surfaced from libmpv to the UI thread.

#[derive(Clone, Debug, PartialEq)]
pub enum MpvEvent {
    /// A file finished opening; carries the path mpv resolved.
    FileLoaded {
        path: String,
    },
    /// Playback of the current file ended (or was stopped).
    EndFile {
        reason: String,
    },
    PlaybackRestart,
    /// Cached property changed; `value` is already `Display`-formatted.
    PropertyChanged {
        name: String,
        value: String,
    },
    /// Log record, kept for the diagnostics panel.
    Log {
        level: String,
        text: String,
    },
}
