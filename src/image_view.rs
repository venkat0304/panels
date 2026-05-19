//! Rendering image files inside a pane.
//!
//! The filesystem panel opens files by splitting a pane and running
//! `$EDITOR`. For images that just dumps binary garbage, so instead we
//! re-invoke the `panels` binary with the hidden `render-image`
//! subcommand. That prefers any installed terminal image viewer
//! (`chafa`, `viu`, `timg`, `kitty +kitten icat`) and otherwise falls
//! back to emitting the Kitty graphics protocol directly for PNGs,
//! which the pane's terminal parser relays to the host terminal.

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

/// Entry point for the `panels render-image <path>` subcommand.
pub fn run_render_image(args: &[String]) -> io::Result<i32> {
    let Some(path) = args.first() else {
        eprintln!("usage: panels render-image <path>");
        return Ok(2);
    };
    let path = Path::new(path);

    if !path.is_file() {
        eprintln!("render-image: not a file: {}", path.display());
        wait_for_keypress();
        return Ok(1);
    }

    let rendered = match find_external_viewer() {
        Some(viewer) => run_external_viewer(&viewer, path),
        None => render_png_via_kitty(path),
    };

    match rendered {
        Ok(true) => {}
        Ok(false) => {
            println!(
                "Cannot preview {}: no terminal image viewer found.\n\
                 Install one of: chafa, viu, timg, or kitty (for `kitten icat`).\n\
                 Non-PNG images need an external viewer.",
                path.display()
            );
        }
        Err(err) => {
            eprintln!("render-image: {err}");
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

/// Emits the Kitty graphics protocol for a PNG so the pane's terminal
/// parser picks it up and the host terminal draws it. Returns `Ok(false)`
/// when the file is not a PNG (the only format the protocol carries
/// without an external decoder).
fn render_png_via_kitty(path: &Path) -> io::Result<bool> {
    let bytes = std::fs::read(path)?;
    let Some(sequence) = encode_png_kitty(&bytes) else {
        return Ok(false);
    };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(&sequence)?;
    out.flush()?;
    Ok(true)
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
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(4096).collect();

    let mut out = Vec::new();
    // f=100 PNG, a=T transmit+display, t=d direct, q=2 suppress responses.
    for (idx, chunk) in chunks.iter().enumerate() {
        let m = u8::from(idx + 1 != chunks.len());
        if idx == 0 {
            out.extend_from_slice(format!("\x1b_Gf=100,a=T,t=d,q=2,m={m};").as_bytes());
        } else {
            out.extend_from_slice(format!("\x1b_Gm={m};").as_bytes());
        }
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out.push(b'\n');
    Some(out)
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
    fn non_png_bytes_are_rejected() {
        assert!(encode_png_kitty(b"not a png").is_none());
        assert!(encode_png_kitty(&[]).is_none());
    }

    #[test]
    fn png_is_wrapped_in_chunked_kitty_sequence() {
        // Minimal valid PNG signature + filler payload spanning >1 chunk.
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(std::iter::repeat(0xAB).take(8192));

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
}
