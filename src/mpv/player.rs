//! One embedded libmpv instance.
//!
//! Each player owns an `mpv_handle`, a dedicated event thread and a compact
//! cache of observed properties. The UI thread never calls into libmpv for
//! playback state: `time_pos`, `duration`, `paused`, `speed`, `volume`,
//! `eof_reached`, `core_idle` and `filename` are maintained by the event thread
//! and can be read every frame without blocking.

use std::collections::VecDeque;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, bail};

use crate::mpv::MpvEvent;
use crate::mpv::lib::{
    Api, LibMpv, MpvEventEndFile, MpvEventLogMessage, MpvEventProperty, MpvEventRaw, MpvHandle,
    event_id, format,
};

/// Upper bound on buffered events. The queue drops the oldest entry when full so
/// a stalled UI can never grow memory without bound.
const EVENT_QUEUE_CAPACITY: usize = 512;

/// Properties mirrored into [`PropertyCache`] (one entry each).
const OBSERVED_PROPERTIES: [&str; 8] = [
    "time-pos",
    "duration",
    "pause",
    "eof-reached",
    "volume",
    "speed",
    "core-idle",
    "filename",
];

/// Options applied before `mpv_initialize`.
#[derive(Clone, Debug)]
pub struct MpvOptions {
    /// `hwdec` value, e.g. `auto-safe`.
    pub hwdec: String,
    /// Video output driver (`gpu` / `gpu-next`).
    pub vo: String,
    /// `gpu-context` (`d3d11`).
    pub gpu_context: String,
    pub audio: bool,
    pub loop_file: bool,
    /// Seconds of demuxer cache.
    pub cache_secs: u32,
}

impl Default for MpvOptions {
    fn default() -> Self {
        Self {
            hwdec: "auto-safe".into(),
            vo: "gpu".into(),
            gpu_context: "d3d11".into(),
            audio: true,
            loop_file: true,
            cache_secs: 30,
        }
    }
}

/// Lock-free-ish snapshot of the properties the UI displays.
struct PropertyCache {
    time_pos: AtomicU64,
    duration: AtomicU64,
    volume: AtomicU64,
    speed: AtomicU64,
    paused: AtomicBool,
    eof_reached: AtomicBool,
    core_idle: AtomicBool,
    filename: Mutex<String>,
}

impl PropertyCache {
    fn new() -> Self {
        Self {
            time_pos: AtomicU64::new(0.0f64.to_bits()),
            duration: AtomicU64::new(0.0f64.to_bits()),
            volume: AtomicU64::new(100.0f64.to_bits()),
            speed: AtomicU64::new(1.0f64.to_bits()),
            paused: AtomicBool::new(false),
            eof_reached: AtomicBool::new(false),
            core_idle: AtomicBool::new(true),
            filename: Mutex::new(String::new()),
        }
    }

    /// Applies one observed `MPV_FORMAT_STRING` property value.
    fn update(&self, name: &str, value: &str) {
        match name {
            "time-pos" => store_number(&self.time_pos, value),
            "duration" => store_number(&self.duration, value),
            "volume" => store_number(&self.volume, value),
            "speed" => store_number(&self.speed, value),
            "pause" => self.paused.store(parse_bool(value), Ordering::Relaxed),
            "eof-reached" => self.eof_reached.store(parse_bool(value), Ordering::Relaxed),
            "core-idle" => self.core_idle.store(parse_bool(value), Ordering::Relaxed),
            "filename" => {
                let mut filename = lock(&self.filename);
                filename.clear();
                filename.push_str(value);
            }
            _ => {}
        }
    }

    fn time_pos(&self) -> f64 {
        read_number(&self.time_pos)
    }

    fn duration(&self) -> f64 {
        read_number(&self.duration)
    }

    fn volume(&self) -> f64 {
        read_number(&self.volume)
    }

    fn speed(&self) -> f64 {
        read_number(&self.speed)
    }

    fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    fn eof_reached(&self) -> bool {
        self.eof_reached.load(Ordering::Relaxed)
    }

    fn core_idle(&self) -> bool {
        self.core_idle.load(Ordering::Relaxed)
    }

    fn filename(&self) -> String {
        lock(&self.filename).clone()
    }
}

/// Bounded event queue drained by `poll_events`.
struct EventQueue {
    events: Mutex<VecDeque<MpvEvent>>,
}

impl EventQueue {
    fn new() -> Self {
        Self {
            events: Mutex::new(VecDeque::new()),
        }
    }

    fn push(&self, event: MpvEvent) {
        let mut events = lock(&self.events);
        if events.len() >= EVENT_QUEUE_CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
    }

    fn drain(&self) -> Vec<MpvEvent> {
        lock(&self.events).drain(..).collect()
    }
}

/// Coalescing wake-up signal shared between libmpv's callback and the event
/// thread.
struct Wakeup {
    pending: AtomicBool,
    wait_lock: Mutex<()>,
    condvar: Condvar,
}

impl Wakeup {
    fn new() -> Self {
        Self {
            pending: AtomicBool::new(false),
            wait_lock: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    fn signal(&self) {
        self.pending.store(true, Ordering::Release);
        let _guard = lock(&self.wait_lock);
        self.condvar.notify_all();
    }

    /// Waits up to `timeout` for a signal; returns immediately if one is
    /// already pending.
    fn wait(&self, timeout: Duration) {
        let guard = lock(&self.wait_lock);
        if self.pending.swap(false, Ordering::AcqRel) {
            return;
        }
        let (guard, _timed_out) = self
            .condvar
            .wait_timeout(guard, timeout)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        drop(guard);
        self.pending.store(false, Ordering::Release);
    }
}

/// libmpv wake-up callback: notifies the event thread that events are queued.
///
/// # Safety
///
/// `userdata` must be the pointer registered with `mpv_set_wakeup_callback` in
/// [`MpvPlayer::new`], i.e. a live `Arc<Wakeup>` allocation. The player keeps
/// that allocation alive until after `mpv_terminate_destroy`.
unsafe extern "C" fn wakeup_callback(userdata: *mut c_void) {
    if userdata.is_null() {
        return;
    }
    // SAFETY: see the function contract; libmpv passes back the pointer we
    // registered.
    let wakeup = unsafe { &*userdata.cast::<Wakeup>() };
    wakeup.signal();
}

/// One libmpv instance rendered into a native child window.
pub struct MpvPlayer {
    lib: Arc<LibMpv>,
    /// `mpv_handle *`; kept as `usize` so the struct stays `Send`.
    handle: usize,
    cache: Arc<PropertyCache>,
    events: Arc<EventQueue>,
    wakeup: Arc<Wakeup>,
    stopped: Arc<AtomicBool>,
    /// `None` after a failed spawn or after `Drop` joined the thread.
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl MpvPlayer {
    /// Creates and initialises a player bound to `wid`.
    pub(crate) fn new(lib: &Arc<LibMpv>, wid: i64, opts: &MpvOptions) -> anyhow::Result<Self> {
        let api = lib.api();

        // SAFETY: `mpv_create` takes no arguments and returns NULL on failure.
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            bail!("mpv_create() returned NULL");
        }

        // Created before `init` so the wake-up callback can never observe a
        // freed allocation: this outer reference survives until after
        // `mpv_terminate_destroy` in the error path.
        let wakeup = Arc::new(Wakeup::new());
        match Self::init(lib, handle, wid, opts, Arc::clone(&wakeup)) {
            Ok(player) => Ok(player),
            Err(err) => {
                // SAFETY: `handle` is ours alone; terminate_destroy accepts
                // both initialised and partially initialised handles.
                unsafe { (api.terminate_destroy)(handle) };
                drop(wakeup);
                Err(err)
            }
        }
    }

    fn init(
        lib: &Arc<LibMpv>,
        handle: MpvHandle,
        wid: i64,
        opts: &MpvOptions,
        wakeup: Arc<Wakeup>,
    ) -> anyhow::Result<Self> {
        let api = lib.api();
        apply_options(api, handle, wid, opts)?;

        // SAFETY: static NUL-terminated string.
        let rc = unsafe { (api.request_log_messages)(handle, c"info".as_ptr()) };
        if rc < 0 {
            bail!("mpv_request_log_messages failed: {}", error_string(api, rc));
        }

        for (index, name) in OBSERVED_PROPERTIES.iter().enumerate() {
            let c_name =
                CString::new(*name).with_context(|| format!("observing property {name}"))?;
            // SAFETY: the handle is valid and not yet initialised; `c_name`
            // outlives the call. Userdata is logged but unused.
            let rc = unsafe {
                (api.observe_property)(handle, index as u64 + 1, c_name.as_ptr(), format::STRING)
            };
            if rc < 0 {
                bail!(
                    "observing property {name} failed: {}",
                    error_string(api, rc)
                );
            }
        }

        // SAFETY: handle is valid and owned by this call path.
        let rc = unsafe { (api.initialize)(handle) };
        if rc < 0 {
            bail!("mpv_initialize failed: {}", error_string(api, rc));
        }

        let cache = Arc::new(PropertyCache::new());
        let events = Arc::new(EventQueue::new());
        let stopped = Arc::new(AtomicBool::new(false));

        // SAFETY: `wakeup` lives at least until `terminate_destroy`; libmpv only
        // invokes the callback while the handle exists.
        unsafe {
            (api.set_wakeup_callback)(
                handle,
                Some(wakeup_callback),
                Arc::as_ptr(&wakeup).cast_mut().cast::<c_void>(),
            );
        }

        let thread_api = Arc::clone(lib);
        let thread_cache = Arc::clone(&cache);
        let thread_events = Arc::clone(&events);
        let thread_wakeup = Arc::clone(&wakeup);
        let thread_stopped = Arc::clone(&stopped);
        let thread_handle = handle as usize;
        let join = thread::Builder::new()
            .name("mpv-events".to_owned())
            .spawn(move || {
                event_loop(
                    thread_api,
                    thread_handle,
                    thread_cache,
                    thread_events,
                    thread_wakeup,
                    thread_stopped,
                );
            })
            .context("spawning the libmpv event thread")?;

        Ok(Self {
            lib: Arc::clone(lib),
            handle: handle as usize,
            cache,
            events,
            wakeup,
            stopped,
            thread: Mutex::new(Some(join)),
        })
    }

    fn handle(&self) -> MpvHandle {
        self.handle as MpvHandle
    }

    /// Starts playing `path`, replacing the current file.
    pub fn load_file(&self, path: &str) {
        let _ = self.command(&["loadfile", path, "replace"]);
    }

    /// Appends `path` to the playlist (`loadfile ... append`).
    pub fn load_file_append(&self, path: &str) {
        let _ = self.command(&["loadfile", path, "append"]);
    }

    pub fn set_pause(&self, paused: bool) {
        self.cache.paused.store(paused, Ordering::Relaxed);
        let _ = self.set_property("pause", if paused { "yes" } else { "no" });
    }

    pub fn paused(&self) -> bool {
        self.cache.paused()
    }

    pub fn toggle_pause(&self) {
        self.cache
            .paused
            .store(!self.cache.paused(), Ordering::Relaxed);
        let _ = self.command(&["cycle", "pause"]);
    }

    pub fn time_pos(&self) -> f64 {
        self.cache.time_pos()
    }

    pub fn duration(&self) -> f64 {
        self.cache.duration()
    }

    /// True once the current file reached EOF and `keep-open` parked playback.
    pub fn eof_reached(&self) -> bool {
        self.cache.eof_reached()
    }

    /// True while the core has nothing to play (no file, stopped or idle).
    pub fn core_idle(&self) -> bool {
        self.cache.core_idle()
    }

    /// File name currently reported by mpv (empty until a file loads).
    pub fn filename(&self) -> String {
        self.cache.filename()
    }

    pub fn seek_absolute(&self, secs: f64) {
        if !secs.is_finite() {
            return;
        }
        let target = secs.max(0.0);
        let _ = self.command(&["seek", &format!("{target:.3}"), "absolute+exact"]);
    }

    pub fn seek_relative(&self, secs: f64) {
        if !secs.is_finite() {
            return;
        }
        let _ = self.command(&["seek", &format!("{secs:.3}"), "relative+exact"]);
    }

    pub fn set_speed(&self, speed: f64) {
        if !speed.is_finite() {
            return;
        }
        let speed = speed.clamp(0.01, 100.0);
        self.cache.speed.store(speed.to_bits(), Ordering::Relaxed);
        let _ = self.set_property("speed", &format!("{speed:.3}"));
    }

    pub fn speed(&self) -> f64 {
        self.cache.speed()
    }

    pub fn set_volume(&self, volume: f64) {
        if !volume.is_finite() {
            return;
        }
        let volume = volume.clamp(0.0, 130.0);
        self.cache.volume.store(volume.to_bits(), Ordering::Relaxed);
        let _ = self.set_property("volume", &format!("{volume:.1}"));
    }

    /// Current player volume (0-130).
    pub fn volume(&self) -> f64 {
        self.cache.volume()
    }

    /// Pans the video inside its surface by fractions of the video size.
    ///
    /// mpv itself accepts any finite value (positive pans right/down), so this
    /// forwards the values unchanged; callers that need bounds clamp first.
    pub fn set_pan(&self, x: f64, y: f64) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let x_ok = self.set_property("video-pan-x", &format!("{x:.6}"));
        let y_ok = self.set_property("video-pan-y", &format!("{y:.6}"));
        x_ok && y_ok
    }

    /// Sets the video zoom factor; `0.0` fits the video to the window.
    ///
    /// Values are mpv's `video-zoom` units (positive zooms in); mpv does not
    /// clamp them, so callers that need a range enforce it themselves.
    pub fn set_zoom(&self, zoom: f64) -> bool {
        if !zoom.is_finite() {
            return false;
        }
        self.set_property("video-zoom", &format!("{zoom:.6}"))
    }

    /// Adds `delta` to the current zoom, clamped to `-2.0..=3.0`.
    ///
    /// Returns `false` when `delta` is not finite or libmpv rejects the write.
    /// A missing or unparsable `video-zoom` readback is treated as `0.0`.
    pub fn adjust_zoom(&self, delta: f64) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let current = self
            .get_property("video-zoom")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|zoom| zoom.is_finite())
            .unwrap_or(0.0);
        self.set_zoom((current + delta).clamp(-2.0, 3.0))
    }

    pub fn set_mute(&self, mute: bool) {
        let _ = self.set_property("mute", if mute { "yes" } else { "no" });
    }

    pub fn frame_step(&self) {
        let _ = self.command(&["frame-step"]);
    }

    pub fn frame_back_step(&self) {
        let _ = self.command(&["frame-back-step"]);
    }

    /// Sets any libmpv property using its string representation.
    ///
    /// This works at runtime for options that are also properties, including
    /// `lavfi-complex`, `speed`, `volume`, `mute` and `pause`.
    pub fn set_property(&self, name: &str, value: &str) -> bool {
        let (Ok(c_name), Ok(c_value)) = (CString::new(name), CString::new(value)) else {
            return false;
        };
        // SAFETY: handle is valid while `self` exists; both C strings outlive
        // the call.
        unsafe {
            (self.lib.api().set_property_string)(self.handle(), c_name.as_ptr(), c_value.as_ptr())
                >= 0
        }
    }

    pub fn get_property(&self, name: &str) -> Option<String> {
        property_string(self.lib.api(), self.handle(), name)
    }

    /// Drains all queued events without blocking.
    pub fn poll_events(&self) -> Vec<MpvEvent> {
        self.events.drain()
    }

    /// Re-binds the video output to another native window.
    pub fn set_wid(&self, wid: i64) {
        let c_name = match CString::new("wid") {
            Ok(c_name) => c_name,
            Err(_) => return,
        };
        let value = wid;
        let pointer = std::ptr::from_ref(&value).cast_mut().cast::<c_void>();
        // SAFETY: `pointer` points to a valid `i64` for the duration of the
        // call and matches `format::INT64`.
        unsafe {
            (self.lib.api().set_property)(self.handle(), c_name.as_ptr(), format::INT64, pointer);
        }
    }

    pub fn stop(&self) {
        let _ = self.command(&["stop"]);
    }

    /// Saves the current video frame as a PNG / JPG at `path`.
    ///
    /// Uses mpv's `screenshot-to-file` with the `video` flag so subtitles and
    /// OSD never end up in the file.
    pub fn screenshot_to_file(&self, path: &str) -> bool {
        self.command(&["screenshot-to-file", path, "video"])
    }

    fn command(&self, args: &[&str]) -> bool {
        let api = self.lib.api();
        let mut owned = Vec::with_capacity(args.len());
        for arg in args {
            match CString::new(*arg) {
                Ok(c_arg) => owned.push(c_arg),
                Err(_) => return false,
            }
        }
        let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(std::ptr::null());
        // SAFETY: `pointers` is a NULL-terminated array of valid NUL-terminated
        // strings that all outlive the call.
        unsafe { (api.command)(self.handle(), pointers.as_ptr()) >= 0 }
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        let api = self.lib.api();
        let handle = self.handle();

        self.stopped.store(true, Ordering::Release);
        // SAFETY: handles are thread-safe; "quit" asks the core to wind down and
        // wakes the event thread.
        unsafe {
            (api.command_string)(handle, c"quit".as_ptr());
        }
        self.wakeup.signal();

        if let Some(join) = lock(&self.thread).take() {
            let _ = join.join();
        }

        // Release observations explicitly so teardown mirrors setup; the core
        // would also drop them on termination.
        for index in 0..OBSERVED_PROPERTIES.len() {
            // SAFETY: the handle is still valid and the event thread has exited.
            unsafe {
                (api.unobserve_property)(handle, (index + 1) as u64);
            }
        }
        // SAFETY: the event thread has exited, so no thread calls into libmpv
        // from our side; removing the callback keeps it from outliving the
        // `Wakeup` allocation.
        unsafe {
            (api.set_wakeup_callback)(handle, None, std::ptr::null_mut());
        }
        // SAFETY: we own the handle and nothing uses it after this point.
        unsafe {
            (api.terminate_destroy)(handle);
        }
        self.handle = 0;
    }
}

/// Applies default options and the caller's embedding options.
fn apply_options(api: &Api, handle: MpvHandle, wid: i64, opts: &MpvOptions) -> anyhow::Result<()> {
    let mut options: Vec<(&str, String)> = vec![
        ("wid", wid.to_string()),
        ("vo", opts.vo.clone()),
        ("gpu-context", opts.gpu_context.clone()),
        ("hwdec", opts.hwdec.clone()),
        ("osc", "no".to_owned()),
        ("osd-level", "0".to_owned()),
        ("input-default-bindings", "no".to_owned()),
        ("input-vo-keyboard", "no".to_owned()),
        ("terminal", "no".to_owned()),
        ("config", "no".to_owned()),
        ("idle", "yes".to_owned()),
        ("keep-open", "yes".to_owned()),
        ("cache", "yes".to_owned()),
        ("cache-secs", opts.cache_secs.to_string()),
        ("demuxer-max-bytes", "256MiB".to_owned()),
        (
            "loop-file",
            if opts.loop_file { "inf" } else { "no" }.to_owned(),
        ),
    ];
    if !opts.audio {
        // `audio=yes` is not a valid value for mpv's `audio` option (it only
        // accepts `no`, `auto` or a track number). Muting is therefore
        // expressed through `aid=no`; with audio enabled we leave the default
        // (`auto`) untouched.
        options.push(("aid", "no".to_owned()));
    }

    for (name, value) in options {
        let c_name = CString::new(name).with_context(|| format!("option name {name}"))?;
        let c_value =
            CString::new(value.clone()).with_context(|| format!("value for option {name}"))?;
        // SAFETY: handle is valid and not yet initialised; both strings outlive
        // the call.
        let rc = unsafe { (api.set_option_string)(handle, c_name.as_ptr(), c_value.as_ptr()) };
        if rc < 0 {
            bail!(
                "mpv rejected option {name}={value}: {}",
                error_string(api, rc)
            );
        }
    }

    Ok(())
}

/// Runs until the player is dropped or libmpv shuts down.
fn event_loop(
    lib: Arc<LibMpv>,
    handle: usize,
    cache: Arc<PropertyCache>,
    events: Arc<EventQueue>,
    wakeup: Arc<Wakeup>,
    stopped: Arc<AtomicBool>,
) {
    let api = lib.api();
    let handle = handle as MpvHandle;
    loop {
        if stopped.load(Ordering::Acquire) {
            return;
        }
        wakeup.wait(Duration::from_millis(50));

        loop {
            // SAFETY: `handle` is valid while the player owns it and this is the
            // only thread consuming its event queue. A zero timeout never
            // blocks.
            let event = unsafe { (api.wait_event)(handle, 0.0) };
            if event.is_null() {
                break;
            }
            // SAFETY: libmpv keeps the event valid until the next
            // `mpv_wait_event` call; we copy everything needed immediately.
            let event = unsafe { &*event };
            if event.event_id == event_id::NONE {
                break;
            }
            let shutdown = event.event_id == event_id::SHUTDOWN;
            // SAFETY: event and payload pointers are valid until the next
            // wait_event call.
            unsafe {
                handle_event(api, handle, event, &cache, &events);
            }
            if shutdown {
                return;
            }
        }
    }
}

/// Translates one libmpv event into the app-facing queue/cache.
///
/// # Safety
///
/// `event` must point into libmpv-owned memory that stays valid until the next
/// `mpv_wait_event` call, and its `data` payload must match its event id.
unsafe fn handle_event(
    api: &Api,
    handle: MpvHandle,
    event: &MpvEventRaw,
    cache: &PropertyCache,
    events: &EventQueue,
) {
    match event.event_id {
        event_id::PROPERTY_CHANGE => {
            if event.data.is_null() {
                return;
            }
            // SAFETY: MPV_EVENT_PROPERTY_CHANGE carries an mpv_event_property.
            let property = unsafe { &*event.data.cast::<MpvEventProperty>() };
            if property.name.is_null() {
                return;
            }
            // SAFETY: `name` is a NUL-terminated libmpv-owned string.
            let name = unsafe { c_string(property.name) };
            // SAFETY: `data` is interpreted according to `format` below.
            let value = match property.format {
                format::NONE => return,
                format::STRING => {
                    if property.data.is_null() {
                        return;
                    }
                    // `MPV_FORMAT_STRING` stores a `char *` in the value slot,
                    // so the payload pointer has to be dereferenced once more
                    // (this matches `mpv_get_property`'s `char **` argument).
                    // SAFETY: libmpv wrote a readable `*const c_char` at `data`.
                    let text = unsafe { *property.data.cast::<*const c_char>() };
                    if text.is_null() {
                        return;
                    }
                    // SAFETY: `text` is a NUL-terminated libmpv-owned string that
                    // stays valid until the next `mpv_wait_event` call.
                    unsafe { c_string(text) }
                }
                format::DOUBLE => {
                    let value = unsafe { property.data.cast::<f64>().as_ref() };
                    match value {
                        Some(value) => value.to_string(),
                        None => return,
                    }
                }
                format::INT64 => {
                    let value = unsafe { property.data.cast::<i64>().as_ref() };
                    match value {
                        Some(value) => value.to_string(),
                        None => return,
                    }
                }
                format::FLAG => {
                    let value = unsafe { property.data.cast::<c_int>().as_ref() };
                    match value {
                        Some(value) if *value != 0 => "yes".to_owned(),
                        Some(_) => "no".to_owned(),
                        None => return,
                    }
                }
                _ => return,
            };
            cache.update(&name, &value);
            events.push(MpvEvent::PropertyChanged { name, value });
        }
        event_id::FILE_LOADED => {
            // Prefer a fresh query: the observed `filename` event may still be
            // queued behind this event on very fast loads.
            let path = property_string(api, handle, "filename").unwrap_or_else(|| cache.filename());
            events.push(MpvEvent::FileLoaded { path });
        }
        event_id::END_FILE => {
            let reason = if event.data.is_null() {
                "unknown".to_owned()
            } else {
                // SAFETY: MPV_EVENT_END_FILE carries an mpv_event_end_file.
                let info = unsafe { &*event.data.cast::<MpvEventEndFile>() };
                match info.reason {
                    0 => "eof".to_owned(),
                    2 => "stop".to_owned(),
                    3 => "quit".to_owned(),
                    4 => format!("error: {}", error_string(api, info.error)),
                    5 => "redirect".to_owned(),
                    other => format!("reason {other}"),
                }
            };
            events.push(MpvEvent::EndFile { reason });
        }
        event_id::PLAYBACK_RESTART => events.push(MpvEvent::PlaybackRestart),
        event_id::LOG_MESSAGE => {
            if event.data.is_null() {
                return;
            }
            // SAFETY: MPV_EVENT_LOG_MESSAGE carries an mpv_event_log_message.
            let message = unsafe { &*event.data.cast::<MpvEventLogMessage>() };
            // SAFETY: each string is NUL-terminated while the event is valid.
            let level = unsafe { c_string(message.level) };
            let prefix = unsafe { c_string(message.prefix) };
            let text = unsafe { c_string(message.text) };
            let text = text.trim_end_matches(['\r', '\n']);
            let text = if prefix.is_empty() {
                text.to_owned()
            } else {
                format!("[{prefix}] {text}")
            };
            events.push(MpvEvent::Log { level, text });
        }
        _ => {}
    }
}

/// Copies a libmpv string into an owned `String`.
///
/// # Safety
///
/// `pointer` must be null or point to a valid NUL-terminated string.
unsafe fn c_string(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: caller guarantees a valid NUL-terminated string.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

/// Reads a property as string, freeing the libmpv allocation.
fn property_string(api: &Api, handle: MpvHandle, name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    // SAFETY: handle is valid for the player's lifetime; the returned pointer is
    // a NUL-terminated allocation owned by the caller.
    let pointer = unsafe { (api.get_property_string)(handle, c_name.as_ptr()) };
    if pointer.is_null() {
        return None;
    }
    // SAFETY: libmpv returned a NUL-terminated allocation.
    let value = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: the allocation came from libmpv and is released exactly once.
    unsafe { (api.free)(pointer.cast::<c_void>()) };
    Some(value)
}

/// Human-readable text for an `mpv_error` code.
fn error_string(api: &Api, code: c_int) -> String {
    // SAFETY: `mpv_error_string` returns a static NUL-terminated string for any
    // code, including unknown ones.
    let pointer = unsafe { (api.error_string)(code) };
    // SAFETY: the returned pointer is static and NUL-terminated.
    unsafe { c_string(pointer) }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn store_number(atomic: &AtomicU64, value: &str) {
    if let Ok(number) = value.parse::<f64>() {
        atomic.store(number.to_bits(), Ordering::Relaxed);
    }
}

fn read_number(atomic: &AtomicU64) -> f64 {
    f64::from_bits(atomic.load(Ordering::Relaxed))
}

/// Parses mpv's string rendering of a flag property.
fn parse_bool(value: &str) -> bool {
    matches!(value, "yes" | "true" | "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_cache_tracks_observed_values() {
        let cache = PropertyCache::new();
        assert_eq!(cache.time_pos(), 0.0);
        assert_eq!(cache.volume(), 100.0);
        assert_eq!(cache.speed(), 1.0);
        assert!(cache.core_idle());

        cache.update("time-pos", "12.5");
        cache.update("duration", "30");
        cache.update("pause", "yes");
        cache.update("eof-reached", "no");
        cache.update("volume", "87.5");
        cache.update("speed", "1.5");
        cache.update("core-idle", "no");
        cache.update("filename", r"D:\media\clipA.mp4");

        assert!((cache.time_pos() - 12.5).abs() < f64::EPSILON);
        assert!((cache.duration() - 30.0).abs() < f64::EPSILON);
        assert!(cache.paused());
        assert!(!cache.eof_reached());
        assert!((cache.volume() - 87.5).abs() < f64::EPSILON);
        assert!((cache.speed() - 1.5).abs() < f64::EPSILON);
        assert!(!cache.core_idle());
        assert_eq!(cache.filename(), r"D:\media\clipA.mp4");

        cache.update("time-pos", "not-a-number");
        assert!(
            (cache.time_pos() - 12.5).abs() < f64::EPSILON,
            "unparsable values must keep the previous cache"
        );
    }

    #[test]
    fn event_queue_is_bounded_and_drops_oldest() {
        let queue = EventQueue::new();
        for index in 0..EVENT_QUEUE_CAPACITY + 10 {
            queue.push(MpvEvent::PropertyChanged {
                name: "time-pos".to_owned(),
                value: index.to_string(),
            });
        }

        let drained = queue.drain();
        assert_eq!(drained.len(), EVENT_QUEUE_CAPACITY);
        assert_eq!(
            drained.first(),
            Some(&MpvEvent::PropertyChanged {
                name: "time-pos".to_owned(),
                value: 10.to_string(),
            })
        );
        assert!(queue.drain().is_empty());
    }

    #[test]
    fn defaults_match_mpv_embedding_expectations() {
        let options = MpvOptions::default();
        assert_eq!(options.hwdec, "auto-safe");
        assert_eq!(options.vo, "gpu");
        assert_eq!(options.gpu_context, "d3d11");
        assert!(options.audio);
        assert!(options.loop_file);
        assert_eq!(options.cache_secs, 30);
    }

    #[test]
    fn flag_parsing_accepts_mpv_spellings() {
        assert!(parse_bool("yes"));
        assert!(parse_bool("true"));
        assert!(parse_bool("1"));
        assert!(!parse_bool("no"));
        assert!(!parse_bool(""));
    }

    #[test]
    fn wakeup_signal_is_coalescing() {
        let wakeup = Wakeup::new();
        wakeup.signal();
        wakeup.signal();
        // A pending signal must return immediately instead of waiting 30s.
        let start = std::time::Instant::now();
        wakeup.wait(Duration::from_secs(30));
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
