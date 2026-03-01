#[cfg(any(target_os = "macos", target_os = "linux"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::PathBuf;

/// Create a platform-native application shortcut for juliaupgui.
/// - macOS: `/Applications/Juliaup.app` bundle
/// - Linux: `~/.local/share/applications/juliaup.desktop`
/// - Windows: no-op (handled by MSI/MSIX)
pub fn create_app_shortcut(gui_bin: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    create_macos_app(gui_bin)?;

    #[cfg(target_os = "linux")]
    create_linux_desktop(gui_bin)?;

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let _ = gui_bin;

    Ok(())
}

/// Remove the platform-native application shortcut.
pub fn remove_app_shortcut() -> Result<()> {
    #[cfg(target_os = "macos")]
    remove_macos_app()?;

    #[cfg(target_os = "linux")]
    remove_linux_desktop()?;

    Ok(())
}

// ── macOS ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn app_bundle_path() -> PathBuf {
    PathBuf::from("/Applications/Juliaup.app")
}

#[cfg(target_os = "macos")]
fn create_macos_app(gui_bin: &std::path::Path) -> Result<()> {
    let app = app_bundle_path();
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");

    std::fs::create_dir_all(&macos)
        .with_context(|| format!("Failed to create {}", macos.display()))?;
    std::fs::create_dir_all(&resources)
        .with_context(|| format!("Failed to create {}", resources.display()))?;

    // Info.plist
    let gui_path = gui_bin.display();
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Juliaup</string>
    <key>CFBundleDisplayName</key>
    <string>Juliaup</string>
    <key>CFBundleIdentifier</key>
    <string>org.julialang.juliaup</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleExecutable</key>
    <string>juliaup-gui</string>
    <key>CFBundleIconFile</key>
    <string>juliaup</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
</dict>
</plist>"#;
    std::fs::write(contents.join("Info.plist"), plist)
        .with_context(|| "Failed to write Info.plist")?;

    // Launcher shell script
    let launcher = format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", gui_path);
    let launcher_path = macos.join("juliaup-gui");
    std::fs::write(&launcher_path, &launcher).with_context(|| "Failed to write launcher script")?;

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&launcher_path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| "Failed to set launcher script permissions")?;

    // Generate .icns icon
    write_icns(&resources.join("juliaup.icns"))?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_macos_app() -> Result<()> {
    let app = app_bundle_path();
    if app.exists() {
        std::fs::remove_dir_all(&app)
            .with_context(|| format!("Failed to remove {}", app.display()))?;
    }
    Ok(())
}

/// Write a minimal .icns file containing the Julia three-dot logo.
/// We generate 256x256 and 128x128 RGBA images and wrap them in the
/// Apple Icon Image format (a simple container of tagged chunks).
#[cfg(target_os = "macos")]
fn write_icns(path: &std::path::Path) -> Result<()> {
    let rgba_256 = julia_logo_rgba(256);
    let rgba_128 = julia_logo_rgba(128);

    let png_256 = encode_png(&rgba_256, 256, 256)?;
    let png_128 = encode_png(&rgba_128, 128, 128)?;

    // icns file: 8-byte header + icon entries
    // ic08 = 256x256 PNG, ic07 = 128x128 PNG
    let mut icns = Vec::new();
    // File header: 'icns' + total length (filled later)
    icns.extend_from_slice(b"icns");
    icns.extend_from_slice(&[0u8; 4]); // placeholder

    // ic08 entry (256x256)
    icns.extend_from_slice(b"ic08");
    let entry_len = (8 + png_256.len()) as u32;
    icns.extend_from_slice(&entry_len.to_be_bytes());
    icns.extend_from_slice(&png_256);

    // ic07 entry (128x128)
    icns.extend_from_slice(b"ic07");
    let entry_len = (8 + png_128.len()) as u32;
    icns.extend_from_slice(&entry_len.to_be_bytes());
    icns.extend_from_slice(&png_128);

    // Fill in total file length
    let total = icns.len() as u32;
    icns[4..8].copy_from_slice(&total.to_be_bytes());

    std::fs::write(path, &icns).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Rasterise the Julia three-dot logo into a square RGBA buffer.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn julia_logo_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let scale = 350.0 / size as f32;

    let circles: [(f32, f32, f32, u8, u8, u8); 3] = [
        (88.4, 250.0, 75.0, 0xCB, 0x3C, 0x33),  // red
        (175.0, 100.0, 75.0, 0x38, 0x98, 0x26), // green
        (261.6, 250.0, 75.0, 0x95, 0x58, 0xB2), // purple
    ];

    for py in 0..size {
        for px in 0..size {
            let fx = (px as f32 + 0.5) * scale;
            let fy = (py as f32 + 0.5) * scale;
            let idx = ((py * size + px) * 4) as usize;

            for &(cx, cy, r, red, green, blue) in &circles {
                let dist = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() - r;
                if dist < 1.0 {
                    let alpha = if dist < 0.0 {
                        255
                    } else {
                        (255.0 * (1.0 - dist)) as u8
                    };
                    if alpha > rgba[idx + 3] {
                        rgba[idx] = red;
                        rgba[idx + 1] = green;
                        rgba[idx + 2] = blue;
                        rgba[idx + 3] = alpha;
                    }
                }
            }
        }
    }
    rgba
}

/// Minimal PNG encoder for RGBA data (no compression for simplicity; uses
/// filter=None with zlib stored blocks).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    use std::io::Write;

    let mut out = Vec::new();
    // PNG signature
    out.write_all(&[137, 80, 78, 71, 13, 10, 26, 10])?;

    // IHDR
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr)?;

    // IDAT: build raw image data (filter byte 0 + row pixels)
    let row_len = (width as usize) * 4 + 1; // +1 for filter byte
    let mut raw = Vec::with_capacity(row_len * height as usize);
    for y in 0..height as usize {
        raw.push(0); // filter: None
        let start = y * width as usize * 4;
        let end = start + width as usize * 4;
        raw.extend_from_slice(&rgba[start..end]);
    }
    let compressed = deflate_stored(&raw);
    write_png_chunk(&mut out, b"IDAT", &compressed)?;

    // IEND
    write_png_chunk(&mut out, b"IEND", &[])?;

    Ok(out)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) -> Result<()> {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = png_crc32(&crc_data);
    out.extend_from_slice(&crc.to_be_bytes());
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn png_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC_TABLE[idx] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// zlib "stored" (no compression) wrapper.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // zlib header (CM=8, CINFO=7, no dict, FCHECK)
    out.push(0x78);
    out.push(0x01);

    // Split into 65535-byte blocks
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let last = i == chunks.len() - 1;
        out.push(if last { 1 } else { 0 }); // BFINAL
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes()); // NLEN
        out.extend_from_slice(chunk);
    }

    // Adler-32 checksum
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
static CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0u32;
    while n < 256 {
        let mut c = n;
        let mut k = 0;
        while k < 8 {
            if c & 1 != 0 {
                c = 0xEDB88320 ^ (c >> 1);
            } else {
                c >>= 1;
            }
            k += 1;
        }
        table[n as usize] = c;
        n += 1;
    }
    table
};

// ── Linux ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn desktop_file_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine XDG data directory"))?;
    Ok(data_dir.join("applications").join("juliaup.desktop"))
}

#[cfg(target_os = "linux")]
fn icon_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine XDG data directory"))?;
    Ok(data_dir
        .join("icons")
        .join("hicolor")
        .join("256x256")
        .join("apps"))
}

#[cfg(target_os = "linux")]
fn create_linux_desktop(gui_bin: &std::path::Path) -> Result<()> {
    // Install icon
    let icon_dest_dir = icon_dir()?;
    std::fs::create_dir_all(&icon_dest_dir)
        .with_context(|| format!("Failed to create {}", icon_dest_dir.display()))?;
    let icon_path = icon_dest_dir.join("juliaup.png");
    let rgba = julia_logo_rgba(256);
    let png = encode_png(&rgba, 256, 256)?;
    std::fs::write(&icon_path, &png)
        .with_context(|| format!("Failed to write {}", icon_path.display()))?;

    // .desktop file
    let desktop_dir = desktop_file_path()?;
    if let Some(parent) = desktop_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Juliaup\n\
         Comment=Julia version manager\n\
         Exec={}\n\
         Icon=juliaup\n\
         Terminal=false\n\
         Categories=Development;\n\
         StartupWMClass=juliaup\n",
        gui_bin.display()
    );
    std::fs::write(&desktop_dir, &entry)
        .with_context(|| format!("Failed to write {}", desktop_dir.display()))?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_linux_desktop() -> Result<()> {
    let desktop = desktop_file_path()?;
    if desktop.exists() {
        std::fs::remove_file(&desktop)
            .with_context(|| format!("Failed to remove {}", desktop.display()))?;
    }
    let icon = icon_dir()?.join("juliaup.png");
    if icon.exists() {
        std::fs::remove_file(&icon)
            .with_context(|| format!("Failed to remove {}", icon.display()))?;
    }
    Ok(())
}
