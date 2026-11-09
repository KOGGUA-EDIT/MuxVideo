//! Standalone probe: two child windows, two libmpv instances, clipA vs clipB.
//!
//! Owned by the mpv track. Run it (outside the sandbox) to prove that libmpv
//! embeds and renders; it must exit on its own after `PROBE_SECONDS`.

fn main() -> anyhow::Result<()> {
    anyhow::bail!("mpv_probe not implemented yet")
}
