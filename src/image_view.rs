//! Rendering image files inside a pane.
//!
//! The filesystem panel opens files by splitting a pane and running
//! `$EDITOR`. For images that just dumps binary garbage, so instead we
//! re-invoke the `panels` binary with the hidden `render-image`
//! subcommand. It tries, in order:
//!
//! 1. An installed terminal image viewer (`chafa`, `viu`, `timg`,
//!    `kitty +kitten icat`) — works in any terminal, stays in the pane.
//! 2. If the host terminal's Kitty graphics relay is live (the parent
//!    passes `--kitty`), decode the image to RGBA and emit the Kitty
//!    graphics protocol so it draws inside the pane.
//! 3. Otherwise fall back to opening the OS default image viewer
//!    (Preview/`xdg-open`), so every format still displays *somehow*.

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

use base64::Engine;

/// Image extensions we intercept in the filesystem panel.
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "ico", "avif",
];

/// External viewers tried in order; the first found in `PATH` wins.
/// Each entry is the command plus the leading args before the file path.
const EXTERNAL_VIEWERS: &[&[&str]] = &[
    &["chafa"],
    &["viu"],
    &["timg"],
    &["kitty", "+kitten", "icat"],
];

/// Returns true when `path` has a recognised image extension.
pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            IMAGE_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

/// Entry point for the `panels render-image [--kitty] <path>` subcommand.
pub fn run_render_image(args: &[String]) -> io::Result<i32> {
    let kitty_relay = args.iter().any(|a| a == "--kitty");
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: panels render-image [--kitty] <path>");
        return Ok(2);
    };
    let path = Path::new(path);

    if !path.is_file() {
        eprintln!("render-image: not a file: {}", path.display());
        wait_for_keypress();
        return Ok(1);
    }

    // 1. Prefer an external terminal viewer (any terminal, stays in pane).
    if let Some(viewer) = find_external_viewer() {
        match run_external_viewer(&viewer, path) {
            Ok(true) => {
                wait_for_keypress();
                return Ok(0);
            }
            Ok(false) => {}
            Err(err) => eprintln!("render-image: {err}"),
        }
    }

    // 2. In-pane Kitty graphics, but only if the host relay is live —
    //    otherwise the escape sequence is parsed and silently dropped.
    if kitty_relay {
        match render_image_via_kitty(path) {
            Ok(true) => {
                wait_for_keypress();
                return Ok(0);
            }
            Ok(false) => {}
            Err(err) => eprintln!("render-image: {err}"),
        }
    }

    // 3. Fall back to the OS default image viewer.
    match open_in_os_viewer(path) {
        Ok(true) => {
            println!(
                "Opened {} in your default image viewer.\n\
                 (install chafa/viu/timg or enable [experimental] kitty_graphics \
                 for an in-pane preview)",
                path.display()
            );
        }
        Ok(false) | Err(_) => {
            println!(
                "Cannot preview {}: no terminal image viewer found and the OS \
                 viewer could not be launched.\n\
                 Install one of: chafa, viu, timg, or kitty.",
                path.display()
            );
        }
    }

    wait_for_keypress();
    Ok(0)
}

/// Finds the first external viewer available in `PATH`.
fn find_external_viewer() -> Option<Vec<String>> {
    for viewer in EXTERNAL_VIEWERS {
        let bin = viewer[0];
        if which(bin) {
            return Some(viewer.iter().map(|s| s.to_string()).collect());
        }
    }
    None
}

/// Minimal `PATH` lookup so we don't shell out just to probe.
fn which(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(bin);
        candidate.is_file()
    })
}

/// Runs an external viewer, inheriting stdio so it draws into the pane.
fn run_external_viewer(viewer: &[String], path: &Path) -> io::Result<bool> {
    let mut cmd = Command::new(&viewer[0]);
    cmd.args(&viewer[1..]);
    cmd.arg(path);
    let status = cmd.status()?;
    Ok(status.success())
}

/// Opens `path` in the OS default image viewer. Returns `Ok(true)` when
/// the opener launched successfully.
fn open_in_os_viewer(path: &Path) -> io::Result<bool> {
    let (bin, pre_args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[])
    } else if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", ""])
    } else {
        ("xdg-open", &[])
    };
    let mut cmd = Command::new(bin);
    cmd.args(pre_args);
    cmd.arg(path);
    Ok(cmd.status()?.success())
}

/// Renders any supported image inside the pane via the Kitty graphics
/// protocol. PNGs take a zero-decode fast path; everything else is
/// decoded to RGBA. Returns `Ok(false)` when the image can't be
/// rendered so the caller can fall back to the OS viewer.
fn render_image_via_kitty(path: &Path) -> io::Result<bool> {
    let bytes = std::fs::read(path)?;

    // PNG fast path: ship the file as-is, no decode.
    if let Some(sequence) = encode_png_kitty(&bytes) {
        return write_sequence(&sequence).map(|()| true);
    }

    // Anything else: decode to RGBA and ship raw pixels.
    let Ok(decoded) = image::load_from_memory(&bytes) else {
        return Ok(false);
    };
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return Ok(false);
    }
    let sequence = encode_rgba_kitty(&rgba.into_raw(), width, height);
    write_sequence(&sequence).map(|()| true)
}

/// Writes an escape sequence to stdout and flushes.
fn write_sequence(sequence: &[u8]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(sequence)?;
    out.flush()
}

/// Encodes PNG bytes as a chunked Kitty graphics transmit+display
/// escape sequence, or `None` if the bytes are not a PNG. Pure so it
/// can be unit-tested without touching stdout.
fn encode_png_kitty(bytes: &[u8]) -> Option<Vec<u8>> {
    // PNG signature.
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    // f=100 PNG, a=T transmit+display, t=d direct, q=2 suppress responses.
    Some(chunk_kitty(encoded.as_bytes(), "f=100,a=T,t=d,q=2"))
}

/// Encodes raw RGBA pixels as a chunked Kitty graphics sequence. Pure
/// so it can be unit-tested without touching stdout.
fn encode_rgba_kitty(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(rgba);
    // f=32 RGBA, s=width v=height pixels, a=T transmit+display, t=d direct.
    let header = format!("f=32,s={width},v={height},a=T,t=d,q=2");
    chunk_kitty(encoded.as_bytes(), &header)
}

/// Splits base64 `payload` into 4096-byte Kitty APC chunks, putting
/// `first_keys` on the opening chunk and `m=` continuation flags on all.
fn chunk_kitty(payload: &[u8], first_keys: &str) -> Vec<u8> {
    let chunks: Vec<&[u8]> = payload.chunks(4096).collect();
    let mut out = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        let m = u8::from(idx + 1 != chunks.len());
        if idx == 0 {
            out.extend_from_slice(format!("\x1b_G{first_keys},m={m};").as_bytes());
        } else {
            out.extend_from_slice(format!("\x1b_Gm={m};").as_bytes());
        }
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out.push(b'\n');
    out
}

/// Keeps the pane open until the user dismisses it.
fn wait_for_keypress() {
    println!("\n[image] press Enter to close…");
    let _ = io::stdout().flush();
    let mut buf = [0u8; 1];
    let _ = io::stdin().read(&mut buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detects_image_extensions_case_insensitively() {
        assert!(is_image_path(Path::new("photo.PNG")));
        assert!(is_image_path(Path::new("/a/b/c.jpeg")));
        assert!(is_image_path(Path::new("icon.webp")));
        assert!(!is_image_path(Path::new("main.rs")));
        assert!(!is_image_path(Path::new("README")));
        assert!(!is_image_path(Path::new("notes.txt")));
    }

    #[test]
    fn non_png_bytes_are_rejected_by_png_fast_path() {
        assert!(encode_png_kitty(b"not a png").is_none());
        assert!(encode_png_kitty(&[]).is_none());
    }

    #[test]
    fn png_is_wrapped_in_chunked_kitty_sequence() {
        // Minimal valid PNG signature + filler payload spanning >1 chunk.
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(std::iter::repeat_n(0xAB, 8192));

        let seq = encode_png_kitty(&png).expect("png should encode");
        let text = String::from_utf8_lossy(&seq);

        // First chunk carries the format/transmit keys.
        assert!(text.starts_with("\x1b_Gf=100,a=T,t=d,q=2,m=1;"));
        // Continuation chunks omit the keys.
        assert!(text.contains("\x1b\\\x1b_Gm="));
        // Final chunk closes the stream with m=0.
        assert!(text.contains("m=0;"));
        // Every APC block is terminated.
        assert!(text.ends_with("\x1b\\\n"));
    }

    #[test]
    fn rgba_sequence_carries_dimensions_and_format() {
        // 2x1 RGBA = 8 bytes; small enough for a single chunk.
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255];
        let seq = encode_rgba_kitty(&rgba, 2, 1);
        let text = String::from_utf8_lossy(&seq);

        assert!(text.starts_with("\x1b_Gf=32,s=2,v=1,a=T,t=d,q=2,m=0;"));
        assert!(text.ends_with("\x1b\\\n"));
    }
}
