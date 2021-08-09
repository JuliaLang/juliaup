use std::io::Cursor;
use std::fs::File;
use std::path::PathBuf;
use std::path::Path;
use anyhow::{Context, Result};

pub async fn download(url: String, folder: &Path, fname: Option<&Path>) -> Result<PathBuf> {
    let response = reqwest::get(&url).await
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let fname = match fname {
        Some(name) => name,
        None => Path::new(response
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .unwrap_or("tmp.bin"))
    };

    let fname = folder.join(fname);

    let mut file = File::create(&fname)
        .with_context(|| format!("Failed to create file '{}' while downloading '{}'.", fname.display(), url))?;

    let mut content = Cursor::new(response.bytes().await?);
    std::io::copy(&mut content, &mut file)?;

    Ok(fname)
}
