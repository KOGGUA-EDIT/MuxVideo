//! Native open-file dialog (`GetOpenFileNameW`, multi-select).

use std::path::PathBuf;

use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_ALLOWMULTISELECT, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

/// Shows the Windows file picker; returns one path per selected file.
///
/// The dialog is owned by `owner` (our toplevel HWND) so it is modal to
/// MuxVideo. `initial_dir` only seeds the first open.
pub fn pick_videos(owner: i64, initial_dir: Option<&str>) -> Vec<PathBuf> {
    let filter: Vec<u16> = "视频文件\0*.mp4;*.mkv;*.mov;*.avi;*.webm;*.m4v;*.mpg;*.mpeg;*.ts;*.m2ts;*.flv;*.wmv;*.ogv;*.rmvb;*.mts;*.3gp\0所有文件\0*.*\0\0"
        .encode_utf16()
        .collect();

    let mut file_buf = vec![0u16; 32 * 1024];
    let initial: Vec<u16> = initial_dir
        .map(|s| s.encode_utf16().collect())
        .unwrap_or_default();
    if initial.len() < file_buf.len() {
        file_buf[..initial.len()].copy_from_slice(&initial);
    }

    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = if owner > 0 { owner as *mut core::ffi::c_void } else { std::ptr::null_mut() };
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = file_buf.as_mut_ptr();
    ofn.nMaxFile = file_buf.len() as u32;
    ofn.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_HIDEREADONLY
        | OFN_NOCHANGEDIR
        | OFN_ALLOWMULTISELECT;

    // The title string must outlive the call — keep it bound here.
    let title: Vec<u16> = "选择视频\0".encode_utf16().collect();
    ofn.lpstrTitle = title.as_ptr();

    let ok = unsafe { GetOpenFileNameW(&mut ofn) };
    if ok == 0 {
        return Vec::new();
    }

    // Multi-select: directory \0 file1 \0 file2 \0 \0
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0usize;
    for i in 0..file_buf.len() {
        if file_buf[i] == 0 {
            if i == start {
                break;
            }
            parts.push(String::from_utf16_lossy(&file_buf[start..i]));
            start = i + 1;
        }
    }
    if parts.is_empty() {
        return Vec::new();
    }
    if parts.len() == 1 {
        return vec![PathBuf::from(parts.remove(0))];
    }
    let dir = PathBuf::from(parts.remove(0));
    parts.into_iter().map(|name| dir.join(name)).collect()
}
