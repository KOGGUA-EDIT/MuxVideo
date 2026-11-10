//! Measures how long random access actually takes, so the scrub cache work is
//! driven by numbers instead of guesses.
//!
//! Usage: `frame_probe <video> [seconds]`
//!
//! Prints, per probe: seek latency (ms from `seek` to the frame being on screen)
//! and how long `screenshot-raw` plus its RGBA conversion take.

use std::time::{Duration, Instant};

use anyhow::{Context, bail};

use muxvideo::mpv::lib::LibMpv;
use muxvideo::mpv::player::MpvOptions;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        bail!("usage: frame_probe <video> [start-seconds]");
    };
    let start: f64 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20.0);

    let lib = LibMpv::load()?;
    println!("libmpv: {}", lib.dll_path().display());

    let options = MpvOptions {
        audio: false,
        loop_file: false,
        ..MpvOptions::default()
    };
    let player = lib.new_player(0, &options).context("creating player")?;
    player.load_file(&path);
    wait_ready(&player, 15_000).context("waiting for the file to load")?;
    let duration = player.duration();
    println!("duration: {duration:.3} s");
    player.set_pause(true);
    std::thread::sleep(Duration::from_millis(400));

    println!("\n-- backward seek latency (paused, exact seek) --");
    let mut samples = Vec::new();
    let mut target;
    for step in 0..12 {
        // Alternate direction so the numbers show both cases.
        target = if step % 2 == 0 {
            (start - step as f64 * 0.5).max(0.0)
        } else {
            (start + step as f64 * 0.5).min(duration - 0.5)
        };
        let elapsed = timed_seek(&player, target);
        println!(
            "seek to {target:6.2} s -> {:7.1} ms",
            elapsed.as_secs_f64() * 1e3
        );
        samples.push(elapsed.as_secs_f64() * 1e3);
    }
    let average = samples.iter().sum::<f64>() / samples.len() as f64;
    let worst = samples.iter().cloned().fold(0.0, f64::max);
    println!("average {average:.1} ms, worst {worst:.1} ms");

    println!("\n-- screenshot-raw --");
    // Make sure a frame has actually been presented before grabbing.
    player.set_pause(false);
    std::thread::sleep(Duration::from_millis(600));
    player.set_pause(true);
    std::thread::sleep(Duration::from_millis(200));
    let (rc, value) = player.command_node_str(&["get_property", "time-pos"]);
    println!("  get_property time-pos -> rc={rc} value={value:?}");
    let (rc2, value2) = player.command_node_str(&["screenshot-raw"]);
    println!("  raw no-arg command -> rc={rc2} value={value2:?}");
    for mode in ["", "video", "window", "subtitles"] {
        let sample = if mode.is_empty() {
            player.screenshot_raw()
        } else {
            player.screenshot_raw_with(mode)
        };
        match sample {
            Some(frame) => println!(
                "  mode={mode:?} -> {}x{} stride={} format={} bytes={}",
                frame.width,
                frame.height,
                frame.stride,
                frame.format,
                frame.data.len()
            ),
            None => println!("  mode={mode:?} -> failed"),
        }
    }
    let mut grab_total = 0.0;
    let mut convert_total = 0.0;
    let mut size = (0, 0);
    for _ in 0..10 {
        let started = Instant::now();
        let frame = player.screenshot_raw();
        let grabbed = started.elapsed().as_secs_f64() * 1e3;
        grab_total += grabbed;
        if let Some(frame) = frame {
            size = (frame.width, frame.height);
            println!(
                "  {}x{} stride={} format={} bytes={} grab={grabbed:.1} ms",
                frame.width,
                frame.height,
                frame.stride,
                frame.format,
                frame.data.len()
            );
            let started = Instant::now();
            let rgba = frame.to_rgba();
            convert_total += started.elapsed().as_secs_f64() * 1e3;
            println!("    to_rgba: {:?} bytes", rgba.map(|pixels| pixels.len()));
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    println!(
        "average grab {:.1} ms, average convert {:.1} ms at {}x{}",
        grab_total / 10.0,
        convert_total / 10.0,
        size.0,
        size.1
    );

    // Dump the downscaled preview frame next to mpv's own PNG so the RGB order
    // and row stride of the cache path can be compared by eye.
    if let Some(raw) = player.screenshot_raw() {
        if let Some(preview) = muxvideo::app::scrub::to_preview(&raw) {
            write_bmp("preview-dump.bmp", &preview)?;
            println!(
                "wrote preview-dump.bmp ({}x{} stride={})",
                preview.width, preview.height, preview.stride
            );
        } else {
            println!("to_preview returned None");
        }
        let _ = player.screenshot_to_file("mpv-reference.png");
    }
    Ok(())
}

/// Writes a 32-bit BMP so the preview buffer can be inspected without pulling
/// in an image encoder.
fn write_bmp(path: &str, frame: &muxvideo::app::scrub::PreviewFrame) -> anyhow::Result<()> {
    let pixel_bytes = (frame.stride * frame.height as usize) as u32;
    let file_size = 54 + pixel_bytes;
    let mut out = Vec::with_capacity(file_size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(frame.width as i32).to_le_bytes());
    // Negative height: rows are stored top-down, same as the mpv buffer.
    out.extend_from_slice(&(-(frame.height as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&pixel_bytes.to_le_bytes());
    out.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&frame.bgra);
    std::fs::write(path, &out)?;
    Ok(())
}

fn wait_ready(player: &muxvideo::mpv::player::MpvPlayer, timeout_ms: u64) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if player.duration() > 0.0 && !player.core_idle() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    bail!("timed out")
}

/// Seeks to `target` and returns how long until that frame is on screen.
fn timed_seek(player: &muxvideo::mpv::player::MpvPlayer, target: f64) -> Duration {
    let started = Instant::now();
    player.seek_absolute(target);
    let deadline = started + Duration::from_secs(5);
    let mut settled = false;
    while Instant::now() < deadline {
        let position = player.time_pos();
        if (position - target).abs() < 0.02 {
            settled = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if !settled {
        // Report the timeout as a very slow sample instead of hiding it.
        return Duration::from_secs(5);
    }
    // Give the VO one frame to actually present the new picture.
    std::thread::sleep(Duration::from_millis(8));
    started.elapsed()
}
