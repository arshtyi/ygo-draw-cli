use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use tar::Archive;
use tempfile::TempDir;
use xz2::read::XzDecoder;

const OT_CARDS_URL: &str =
    "https://github.com/arshtyi/ygo-cards/releases/download/latest/ot.json";
const RD_CARDS_URL: &str =
    "https://github.com/arshtyi/ygo-cards/releases/download/latest/rd.json";
const ASSETS_URL: &str =
    "https://github.com/arshtyi/ygo-assets/releases/download/latest/assets.tar.xz";
const TYPST_YGO_URL: &str =
    "https://github.com/arshtyi/typst-ygo/archive/refs/heads/main.tar.gz";

const PROJECT_DIR: &str = "typst-ygo";

pub fn refresh(resource_dir: &Path) -> Result<()> {
    fs::create_dir_all(resource_dir).with_context(|| {
        format!(
            "failed to create resource directory {}",
            resource_dir.display()
        )
    })?;

    let workspace = TempDir::new_in(resource_dir).context("failed to create refresh workspace")?;
    let downloads = workspace.path().join("downloads");
    let staging = workspace.path().join(PROJECT_DIR);
    fs::create_dir_all(&downloads).context("failed to create download directory")?;
    fs::create_dir_all(&staging).context("failed to create staging directory")?;

    let client = Client::builder()
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to create HTTP client")?;

    println!("Refreshing typst-ygo...");
    let project_archive = downloads.join("typst-ygo.tar.gz");
    download(&client, TYPST_YGO_URL, &project_archive)?;
    extract_project(&project_archive, &staging)?;

    println!("Refreshing shared assets...");
    let assets_archive = downloads.join("assets.tar.xz");
    download(&client, ASSETS_URL, &assets_archive)?;
    let unpacked_assets = workspace.path().join("unpacked-assets");
    fs::create_dir_all(&unpacked_assets).context("failed to create asset staging directory")?;
    extract_xz_archive(&assets_archive, &unpacked_assets)?;
    let asset_root = find_asset_root(&unpacked_assets)?;
    copy_tree(&asset_root, &staging.join("assets"))?;

    println!("Refreshing card data...");
    let ot_cards = staging.join("assets/ot/card/ot.json");
    let rd_cards = staging.join("assets/rd/card/rd.json");
    download(&client, OT_CARDS_URL, &ot_cards)?;
    download(&client, RD_CARDS_URL, &rd_cards)?;

    require_file(&staging.join("lib/mod.typ"))?;
    require_file(&ot_cards)?;
    require_file(&rd_cards)?;
    install(staging, &resource_dir.join(PROJECT_DIR), workspace.path())?;

    println!(
        "Resources refreshed at {}",
        resource_dir.join(PROJECT_DIR).display()
    );
    Ok(())
}

fn download(client: &Client, url: &str, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download returned an error for {url}"))?;
    let file = File::create(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    let mut writer = BufWriter::new(file);
    let size = io::copy(&mut response, &mut writer)
        .with_context(|| format!("failed to save download from {url}"))?;
    if size == 0 {
        bail!("download from {url} was empty");
    }
    Ok(())
}

fn extract_project(archive_path: &Path, destination: &Path) -> Result<()> {
    let archive = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let mut archive = Archive::new(GzDecoder::new(archive));

    for entry in archive.entries().context("failed to read typst-ygo archive")? {
        let mut entry = entry.context("failed to read typst-ygo archive entry")?;
        let path = entry.path().context("typst-ygo archive contains an invalid path")?;
        let Some(relative) = strip_first_component(&path) else {
            continue;
        };
        entry
            .unpack(destination.join(relative))
            .context("failed to extract typst-ygo archive")?;
    }
    Ok(())
}

fn extract_xz_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    let archive = File::open(archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let mut archive = Archive::new(XzDecoder::new(archive));
    archive
        .unpack(destination)
        .context("failed to extract asset archive")?;
    Ok(())
}

fn strip_first_component(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    components.next()?;
    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!relative.as_os_str().is_empty()).then_some(relative)
}

fn find_asset_root(unpacked: &Path) -> Result<PathBuf> {
    let direct_assets = unpacked.join("assets");
    if is_asset_root(&direct_assets) {
        return Ok(direct_assets);
    }
    if is_asset_root(unpacked) {
        return Ok(unpacked.to_path_buf());
    }

    for entry in fs::read_dir(unpacked).context("failed to inspect extracted assets")? {
        let candidate = entry?.path();
        if candidate.is_dir() {
            if is_asset_root(&candidate) {
                return Ok(candidate);
            }
            let nested_assets = candidate.join("assets");
            if is_asset_root(&nested_assets) {
                return Ok(nested_assets);
            }
        }
    }

    bail!("asset archive does not contain ot and rd directories")
}

fn is_asset_root(path: &Path) -> bool {
    path.join("ot").is_dir() && path.join("rd").is_dir()
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn require_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("refreshed resources are missing {}", path.display());
    }
    Ok(())
}

fn install(staging: PathBuf, destination: &Path, workspace: &Path) -> Result<()> {
    let backup = workspace.join("previous");
    let had_previous = destination.exists();
    if had_previous {
        fs::rename(destination, &backup).with_context(|| {
            format!("failed to move existing resources at {}", destination.display())
        })?;
    }

    if let Err(error) = fs::rename(&staging, destination) {
        if had_previous {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error).with_context(|| {
            format!("failed to install resources at {}", destination.display())
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_archive_root_safely() {
        assert_eq!(
            strip_first_component(Path::new("typst-ygo-main/lib/mod.typ")),
            Some(PathBuf::from("lib/mod.typ"))
        );
        assert_eq!(strip_first_component(Path::new("typst-ygo-main")), None);
        assert_eq!(
            strip_first_component(Path::new("typst-ygo-main/../outside")),
            None
        );
    }

    #[test]
    fn locates_supported_asset_archive_layouts() {
        let temp = TempDir::new().expect("temporary directory should be created");
        fs::create_dir_all(temp.path().join("assets/ot")).unwrap();
        fs::create_dir_all(temp.path().join("assets/rd")).unwrap();

        assert_eq!(
            find_asset_root(temp.path()).unwrap(),
            temp.path().join("assets")
        );
    }

    #[test]
    fn installs_staged_resources_over_previous_version() {
        let temp = TempDir::new().expect("temporary directory should be created");
        let destination = temp.path().join(PROJECT_DIR);
        let staging = temp.path().join("staging");
        fs::create_dir_all(&destination).unwrap();
        fs::create_dir_all(&staging).unwrap();
        fs::write(destination.join("old"), "old").unwrap();
        fs::write(staging.join("new"), "new").unwrap();

        install(staging, &destination, temp.path()).unwrap();

        assert!(!destination.join("old").exists());
        assert_eq!(fs::read_to_string(destination.join("new")).unwrap(), "new");
    }

    #[test]
    fn asset_root_requires_both_card_kinds() {
        let temp = TempDir::new().expect("temporary directory should be created");
        fs::create_dir_all(temp.path().join("ot")).unwrap();

        assert!(find_asset_root(temp.path()).is_err());
    }
}
