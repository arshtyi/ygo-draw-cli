use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use tar::Archive;
use tempfile::TempDir;
use xz2::read::XzDecoder;

use crate::download::{self, DownloadRecord};

const CARD_RELEASE_URL: &str =
    "https://api.github.com/repos/arshtyi/ygo-cards/releases/tags/latest";
const ASSET_RELEASE_URL: &str =
    "https://api.github.com/repos/arshtyi/ygo-assets/releases/tags/latest";
const TYPST_YGO_COMMIT_URL: &str =
    "https://api.github.com/repos/arshtyi/typst-ygo/commits/main";

const PROJECT_DIR: &str = "typst-ygo";
const MANIFEST_FILE: &str = ".ygo-draw-resources.json";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    published_at: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    size: u64,
    digest: Option<String>,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
}

#[derive(Debug, Serialize)]
struct ResourceManifest {
    schema_version: u32,
    generated_by: String,
    typst_ygo_commit: String,
    releases: Vec<ReleaseVersion>,
    downloads: Vec<DownloadRecord>,
}

#[derive(Debug, Serialize)]
struct ReleaseVersion {
    repository: String,
    tag: String,
    published_at: String,
}

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
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .context("failed to create HTTP client")?;

    println!("Resolving resource versions...");
    let card_release: GitHubRelease = download::get_json(&client, CARD_RELEASE_URL)?;
    let asset_release: GitHubRelease = download::get_json(&client, ASSET_RELEASE_URL)?;
    let typst_ygo_commit: GitHubCommit =
        download::get_json(&client, TYPST_YGO_COMMIT_URL)?;
    let mut records = Vec::new();

    println!("Refreshing typst-ygo...");
    let project_archive = downloads.join("typst-ygo.tar.gz");
    let project_url = format!(
        "https://github.com/arshtyi/typst-ygo/archive/{}.tar.gz",
        typst_ygo_commit.sha
    );
    records.push(download::to_file(
        &client,
        "typst-ygo.tar.gz",
        &project_url,
        &project_archive,
        None,
        None,
    )?);
    extract_project(&project_archive, &staging)?;

    println!("Refreshing shared assets...");
    let asset = find_release_asset(&asset_release, "assets.tar.xz")?;
    let assets_archive = downloads.join("assets.tar.xz");
    records.push(download_release_asset(
        &client,
        asset,
        &assets_archive,
    )?);
    let unpacked_assets = workspace.path().join("unpacked-assets");
    fs::create_dir_all(&unpacked_assets).context("failed to create asset staging directory")?;
    extract_xz_archive(&assets_archive, &unpacked_assets)?;
    let asset_root = find_asset_root(&unpacked_assets)?;
    copy_tree(&asset_root, &staging.join("assets"))?;

    println!("Refreshing card data...");
    let ot_cards = staging.join("assets/ot/card/ot.json");
    let rd_cards = staging.join("assets/rd/card/rd.json");
    records.push(download_release_asset(
        &client,
        find_release_asset(&card_release, "ot.json")?,
        &ot_cards,
    )?);
    records.push(download_release_asset(
        &client,
        find_release_asset(&card_release, "rd.json")?,
        &rd_cards,
    )?);

    require_file(&staging.join("lib/mod.typ"))?;
    require_file(&ot_cards)?;
    require_file(&rd_cards)?;
    write_manifest(
        &staging.join(MANIFEST_FILE),
        &ResourceManifest {
            schema_version: 1,
            generated_by: format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ),
            typst_ygo_commit: typst_ygo_commit.sha,
            releases: vec![
                release_version("arshtyi/ygo-cards", &card_release),
                release_version("arshtyi/ygo-assets", &asset_release),
            ],
            downloads: records,
        },
    )?;
    install(staging, &resource_dir.join(PROJECT_DIR), workspace.path())?;

    println!(
        "Resources refreshed at {}",
        resource_dir.join(PROJECT_DIR).display()
    );
    Ok(())
}

fn find_release_asset<'a>(release: &'a GitHubRelease, name: &str) -> Result<&'a GitHubAsset> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .with_context(|| format!("release {} does not contain {name}", release.tag_name))
}

fn download_release_asset(
    client: &Client,
    asset: &GitHubAsset,
    destination: &Path,
) -> Result<DownloadRecord> {
    let digest = asset
        .digest
        .as_deref()
        .with_context(|| format!("GitHub did not provide a digest for {}", asset.name))?;
    download::to_file(
        client,
        &asset.name,
        &asset.browser_download_url,
        destination,
        Some(asset.size),
        Some(digest),
    )
}

fn release_version(repository: &str, release: &GitHubRelease) -> ReleaseVersion {
    ReleaseVersion {
        repository: repository.to_owned(),
        tag: release.tag_name.clone(),
        published_at: release.published_at.clone(),
    }
}

fn write_manifest(path: &Path, manifest: &ResourceManifest) -> Result<()> {
    let file = File::create(path)
        .with_context(|| format!("failed to create resource manifest {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, manifest)
        .context("failed to serialize resource manifest")?;
    writer.write_all(b"\n")?;
    writer.flush()?;
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

    fn release() -> GitHubRelease {
        GitHubRelease {
            tag_name: "latest".to_owned(),
            published_at: "2026-01-01T00:00:00Z".to_owned(),
            assets: vec![GitHubAsset {
                name: "ot.json".to_owned(),
                size: 3,
                digest: Some(format!("sha256:{}", "a".repeat(64))),
                browser_download_url: "https://example.com/ot.json".to_owned(),
            }],
        }
    }

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

    #[test]
    fn locates_named_release_asset() {
        let release = release();

        assert_eq!(find_release_asset(&release, "ot.json").unwrap().size, 3);
        assert!(find_release_asset(&release, "rd.json").is_err());
    }

    #[test]
    fn writes_versioned_resource_manifest() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(MANIFEST_FILE);
        let manifest = ResourceManifest {
            schema_version: 1,
            generated_by: "ygo-draw/test".to_owned(),
            typst_ygo_commit: "abc123".to_owned(),
            releases: vec![release_version("arshtyi/ygo-cards", &release())],
            downloads: Vec::new(),
        };

        write_manifest(&path, &manifest).unwrap();

        let value: serde_json::Value =
            serde_json::from_reader(File::open(path).unwrap()).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["typst_ygo_commit"], "abc123");
        assert_eq!(value["releases"][0]["tag"], "latest");
    }
}
