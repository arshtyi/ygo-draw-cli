use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Deserialize;
use tempfile::NamedTempFile;

use crate::ids::{CardId, CardKind};

const ARTWORK_URL: &str = "https://images.ygoprodeck.com/images/cards_cropped";

#[derive(Debug, Default)]
pub struct ArtworkBatch {
    pub ready: Vec<CardId>,
    pub issues: Vec<ArtworkIssue>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArtworkIssue {
    pub card: CardId,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct CardRecord {
    id: u64,
    #[serde(rename = "image")]
    image_id: u64,
}

#[derive(Debug)]
struct Catalogs {
    ot: HashMap<u64, u64>,
    rd: HashMap<u64, u64>,
}

impl Catalogs {
    fn load(project_dir: &Path) -> Result<Self> {
        Ok(Self {
            ot: load_catalog(&project_dir.join("assets/ot/card/ot.json"))?,
            rd: load_catalog(&project_dir.join("assets/rd/card/rd.json"))?,
        })
    }

    fn image_id(&self, card: CardId) -> Result<u64, &'static str> {
        let catalog = match card.kind {
            CardKind::Ot => &self.ot,
            CardKind::Rd => &self.rd,
        };
        match catalog.get(&card.value).copied() {
            Some(0) => Err("card data has no center image ID"),
            Some(image_id) => Ok(image_id),
            None => Err("card ID was not found in card data"),
        }
    }
}

pub fn prepare(cards: &[CardId], resource_dir: &Path) -> Result<ArtworkBatch> {
    let project_dir = resource_dir.join("typst-ygo");
    let catalogs = Catalogs::load(&project_dir).with_context(|| {
        format!(
            "failed to load card data from {}; run with --refresh first",
            project_dir.display()
        )
    })?;
    let client = Client::builder()
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .context("failed to create artwork HTTP client")?;
    let mut batch = ArtworkBatch::default();

    for &card in cards {
        let image_id = match catalogs.image_id(card) {
            Ok(image_id) => image_id,
            Err(reason) => {
                batch.issues.push(ArtworkIssue {
                    card,
                    reason: reason.to_owned(),
                });
                continue;
            }
        };
        let destination = project_dir
            .join("assets")
            .join(card.kind.directory_name())
            .join("images")
            .join(format!("{image_id}.jpg"));

        if let Err(error) = ensure_artwork(&client, image_id, &destination) {
            batch.issues.push(ArtworkIssue {
                card,
                reason: format!("failed to prepare image {image_id}: {error:#}"),
            });
            continue;
        }
        batch.ready.push(card);
    }
    Ok(batch)
}

pub fn print_issues(issues: &[ArtworkIssue]) -> Result<()> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    write_issues(&mut stderr, issues).context("failed to print artwork errors")
}

fn load_catalog(path: &Path) -> Result<HashMap<u64, u64>> {
    let file = File::open(path)
        .with_context(|| format!("failed to open card data {}", path.display()))?;
    load_catalog_reader(BufReader::new(file))
        .with_context(|| format!("failed to parse card data {}", path.display()))
}

fn load_catalog_reader(reader: impl Read) -> Result<HashMap<u64, u64>> {
    let records: Vec<CardRecord> = serde_json::from_reader(reader)?;
    let mut catalog = HashMap::with_capacity(records.len());
    for record in records {
        if catalog.insert(record.id, record.image_id).is_some() {
            bail!("card data contains duplicate ID {}", record.id);
        }
    }
    Ok(catalog)
}

fn ensure_artwork(client: &Client, image_id: u64, destination: &Path) -> Result<()> {
    if destination.metadata().is_ok_and(|metadata| metadata.len() > 0) {
        return Ok(());
    }
    let parent = destination
        .parent()
        .context("artwork destination has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;

    let url = format!("{ARTWORK_URL}/{image_id}.jpg");
    let mut response = client
        .get(&url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download returned an error for {url}"))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary image in {}", parent.display()))?;
    let size = io::copy(&mut response, &mut temporary)
        .with_context(|| format!("failed to save image {image_id}"))?;
    if size == 0 {
        bail!("downloaded image was empty");
    }
    temporary
        .flush()
        .with_context(|| format!("failed to flush image {image_id}"))?;
    temporary.persist(destination).map_err(|error| error.error).with_context(|| {
        format!("failed to install downloaded image at {}", destination.display())
    })?;
    Ok(())
}

fn write_issues(mut writer: impl Write, issues: &[ArtworkIssue]) -> io::Result<()> {
    for issue in issues {
        writeln!(
            writer,
            "Skipping card ID {}: {}",
            issue.card.value, issue.reason
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn card(value: u64, kind: CardKind) -> CardId {
        CardId { value, kind }
    }

    #[test]
    fn loads_minimal_fields_from_realistic_card_data() {
        let json = br#"[{"id": 483, "image": 123, "name": "card", "type": ["spell"]}]"#;

        let catalog = load_catalog_reader(Cursor::new(json)).unwrap();

        assert_eq!(catalog.get(&483), Some(&123));
    }

    #[test]
    fn rejects_duplicate_card_ids() {
        let json = br#"[{"id": 1, "image": 2}, {"id": 1, "image": 3}]"#;

        assert!(load_catalog_reader(Cursor::new(json)).is_err());
    }

    #[test]
    fn resolves_catalog_by_card_kind() {
        let catalogs = Catalogs {
            ot: HashMap::from([(1, 11)]),
            rd: HashMap::from([(100_000_001, 22)]),
        };

        assert_eq!(catalogs.image_id(card(1, CardKind::Ot)), Ok(11));
        assert_eq!(
            catalogs.image_id(card(100_000_001, CardKind::Rd)),
            Ok(22)
        );
    }

    #[test]
    fn reports_missing_and_zero_image_ids() {
        let catalogs = Catalogs {
            ot: HashMap::from([(1, 0)]),
            rd: HashMap::new(),
        };

        assert_eq!(
            catalogs.image_id(card(1, CardKind::Ot)),
            Err("card data has no center image ID")
        );
        assert_eq!(
            catalogs.image_id(card(2, CardKind::Ot)),
            Err("card ID was not found in card data")
        );
    }

    #[test]
    fn formats_artwork_issues() {
        let issues = [ArtworkIssue {
            card: card(7, CardKind::Ot),
            reason: "not found".to_owned(),
        }];
        let mut output = Vec::new();

        write_issues(&mut output, &issues).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Skipping card ID 7: not found\n"
        );
    }

    #[test]
    fn artwork_destination_uses_image_id() {
        let root = PathBuf::from("resources/typst-ygo");
        let destination = root
            .join("assets")
            .join(CardKind::Rd.directory_name())
            .join("images")
            .join(format!("{}.jpg", 120_100_001));

        assert_eq!(
            destination,
            PathBuf::from("resources/typst-ygo/assets/rd/images/120100001.jpg")
        );
    }
}
