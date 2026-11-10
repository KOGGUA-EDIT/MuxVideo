//! Scrub cache: keeps decoded frames around so dragging backwards shows a
//! picture immediately instead of waiting for libmpv to re-decode.
//!
//! libmpv has no frame-reuse API — `seek` always replays from the previous
//! keyframe, which measured 60-220 ms on a 4K clip. What we can do is keep the
//! frames we have already rendered: a background thread grabs them through
//! `screenshot-raw` (a GPU readback, ~8 ms at 720p), scales them down to preview
//! size and stores them in an LRU. While the user scrubs, the nearest cached
//! frame is painted over the video surface, so the stall turns into a slightly
//! stale picture instead of a freeze.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use crate::mpv::player::MpvPlayer;
pub use crate::video::childwin::PreviewFrame;

/// Longest preview edge; 4K frames are downscaled to this before caching.
const PREVIEW_MAX_WIDTH: u32 = 1280;

/// How much memory the caches may use per plane.
pub const CACHE_BYTES: usize = 192 * 1024 * 1024;

/// A cached preview plus the frame number it belongs to.
pub struct CachedFrame {
    pub frame: u64,
    pub image: Arc<PreviewFrame>,
}

/// Least-recently-used frame cache keyed by frame number.
pub struct FrameCache {
    entries: VecDeque<CachedFrame>,
    bytes: usize,
    cap: usize,
    hits: u64,
}

impl FrameCache {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            cap,
            hits: 0,
        }
    }

    /// (frames, bytes, hits) for the diagnostics log.
    pub fn stats(&self) -> (usize, usize, u64) {
        (self.entries.len(), self.bytes, self.hits)
    }

    /// Exact frame lookup; touches the entry so it survives eviction.
    pub fn get(&mut self, frame: u64) -> Option<Arc<PreviewFrame>> {
        let index = self.entries.iter().position(|entry| entry.frame == frame)?;
        let entry = self.entries.remove(index)?;
        let image = Arc::clone(&entry.image);
        self.entries.push_back(entry);
        self.hits += 1;
        Some(image)
    }

    /// Closest cached frame to `frame`, but only within `window` frames.
    pub fn nearest(&self, frame: u64, window: u64) -> Option<Arc<PreviewFrame>> {
        let entry = self
            .entries
            .iter()
            .min_by_key(|entry| entry.frame.abs_diff(frame))?;
        if entry.frame.abs_diff(frame) <= window {
            Some(Arc::clone(&entry.image))
        } else {
            None
        }
    }

    /// Stores `image` under `frame` and evicts the oldest entries past the cap.
    pub fn insert(&mut self, frame: u64, image: Arc<PreviewFrame>) {
        if self.get(frame).is_some() {
            return;
        }
        // A single oversized frame would evict everything for nothing.
        if image_stride(&image) > self.cap {
            return;
        }
        self.bytes += image_stride(&image);
        self.entries.push_back(CachedFrame { frame, image });
        while self.bytes > self.cap {
            match self.entries.pop_front() {
                Some(old) => self.bytes = self.bytes.saturating_sub(image_stride(&old.image)),
                None => break,
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

fn image_stride(image: &PreviewFrame) -> usize {
    image.stride * image.height as usize
}

/// Grab request handed to the worker thread.
struct GrabJob {
    player: Arc<MpvPlayer>,
    frame: u64,
}

/// Result queue item: frame number plus its decoded preview bitmap.
type GrabResult = (u64, Arc<PreviewFrame>);

/// Background frame grabber.
///
/// Grabs run off the UI thread because a 4K readback plus downscale costs tens
/// of milliseconds; results are collected with [`Grabber::drain`].
pub struct Grabber {
    jobs: Sender<GrabJob>,
    results: Arc<Mutex<VecDeque<GrabResult>>>,
    pending: Arc<Mutex<VecDeque<u64>>>,
    live: Arc<std::sync::atomic::AtomicBool>,
}

impl Grabber {
    pub fn spawn() -> Self {
        let (jobs, job_rx): (Sender<GrabJob>, Receiver<GrabJob>) = channel();
        let results = Arc::new(Mutex::new(VecDeque::new()));
        let pending = Arc::new(Mutex::new(VecDeque::new()));
        let live = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let thread_results = Arc::clone(&results);
        let thread_pending = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("muxvideo-scrub-grab".to_owned())
            .spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    let grabbed = grab(&job.player);
                    if let Ok(mut pending_frames) = thread_pending.lock()
                        && let Some(index) = pending_frames
                            .iter()
                            .position(|queued| *queued == job.frame)
                    {
                        pending_frames.remove(index);
                    }
                    if let Some(image) = grabbed {
                        if let Ok(mut queue) = thread_results.lock() {
                            queue.push_back((job.frame, image));
                            // A stalled UI must not grow this without bound.
                            while queue.len() > 4 {
                                queue.pop_front();
                            }
                        }
                    } else {
                        crate::diag!("[cache] grab failed at frame {}", job.frame);
                    }
                }
            })
            .expect("spawning the scrub grab thread");
        Self {
            jobs,
            results,
            pending,
            live,
        }
    }

    /// Queues a grab of the frame currently on screen; duplicates are ignored.
    pub fn request(&self, player: &Arc<MpvPlayer>, frame: u64) {
        if !self.live.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if let Ok(mut pending) = self.pending.lock() {
            if pending.contains(&frame) {
                return;
            }
            // Keep the queue shallow: during a fast drag only recent targets
            // matter, and every job costs a readback.
            while pending.len() >= 2 {
                pending.pop_front();
            }
            pending.push_back(frame);
        }
        let _ = self.jobs.send(GrabJob {
            player: Arc::clone(player),
            frame,
        });
    }

    /// Takes every finished grab since the last call.
    pub fn drain(&self) -> Vec<GrabResult> {
        let mut out = Vec::new();
        if let Ok(mut queue) = self.results.lock() {
            while let Some(item) = queue.pop_front() {
                out.push(item);
            }
        }
        out
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        self.live.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Grabs the current frame and prepares a downscaled BGRA preview.
fn grab(player: &MpvPlayer) -> Option<Arc<PreviewFrame>> {
    let raw = player.screenshot_raw()?;
    to_preview(&raw)
}

/// Converts a `RawFrame` into a top-down BGRA buffer for GDI blitting.
pub fn to_preview(raw: &crate::mpv::player::RawFrame) -> Option<Arc<PreviewFrame>> {
    let width = raw.width.max(1);
    let height = raw.height.max(1);
    let bpp = match raw.format.as_str() {
        "rgb24" | "bgr24" => 3,
        _ => 4,
    };
    let swap = matches!(raw.format.as_str(), "bgr0" | "bgr24" | "bgra");
    let has_alpha = matches!(raw.format.as_str(), "rgba" | "bgra");
    let source_row = (width as usize).checked_mul(bpp)?;
    if source_row > raw.stride {
        return None;
    }

    let scale = (PREVIEW_MAX_WIDTH as f32 / width as f32).min(1.0);
    let out_width = ((width as f32 * scale).round() as u32).max(1);
    let out_height = ((height as f32 * scale).round() as u32).max(1);
    let out_stride = out_width as usize * 4;
    let mut bgra = vec![0u8; out_stride * out_height as usize];

    for y in 0..out_height as usize {
        // Nearest neighbour is fine for a preview and keeps this cheap.
        let source_y = (y * height as usize / out_height as usize).min(height as usize - 1);
        let source = &raw.data[source_y * raw.stride..source_y * raw.stride + source_row];
        let row = &mut bgra[y * out_stride..(y + 1) * out_stride];
        for x in 0..out_width as usize {
            let source_x = (x * width as usize / out_width as usize).min(width as usize - 1);
            let s = source_x * bpp;
            let (b, g, r, a) = if swap {
                (source[s], source[s + 1], source[s + 2], source[s + 3])
            } else {
                (source[s + 2], source[s + 1], source[s], source[s + 3])
            };
            let d = x * 4;
            row[d] = b;
            row[d + 1] = g;
            row[d + 2] = r;
            row[d + 3] = if has_alpha { a } else { 255 };
        }
    }

    Some(Arc::new(PreviewFrame {
        width: out_width,
        height: out_height,
        stride: out_stride,
        bgra: Arc::new(bgra),
    }))
}
