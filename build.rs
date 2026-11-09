//! Embeds the app icon into the executable (Explorer / shortcut / taskbar image).
//!
//! `windres` (shipped with the w64devkit toolchain we build with) turns the
//! resource script into a COFF object that the linker folds into the exe. When
//! it is not on PATH the build still succeeds, the exe just keeps the default
//! icon — the window icon set at runtime is independent of this.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/muxvideo.rc");
    println!("cargo:rerun-if-changed=assets/muxvideo.ico");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("muxvideo.res.o");
    if !manifest.join("assets/muxvideo.rc").is_file()
        || !manifest.join("assets/muxvideo.ico").is_file()
    {
        return;
    }

    // windres hands `-I` straight to the preprocessor, which chokes on the space
    // in "D:\app edit\…", so run it from the crate root with relative paths.
    let status = Command::new("windres")
        .current_dir(&manifest)
        .arg("--input-format=rc")
        .arg("--output-format=coff")
        .arg("-I")
        .arg("assets")
        .arg("assets/muxvideo.rc")
        .arg("-o")
        .arg(&out)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg={}", out.display());
        }
        Ok(s) => println!("cargo:warning=windres exited with {s}; exe icon skipped"),
        Err(e) => println!("cargo:warning=windres not available ({e}); exe icon skipped"),
    }
}
