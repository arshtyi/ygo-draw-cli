use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use tempfile::NamedTempFile;
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::{LazyHash, Scalar};
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;
use typst_render::RenderOptions;

use crate::ids::{CardId, CardKind};
use crate::progress;

const PPI: f64 = 600.0;
const POINTS_PER_INCH: f64 = 72.0;
const MAIN_PATH: &str = "template/ygo-draw.typ";

#[derive(Debug, Default)]
pub struct RenderBatch {
    pub rendered: Vec<CardId>,
    pub issues: Vec<RenderIssue>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RenderIssue {
    pub card: CardId,
    pub reason: String,
}

struct WorldAssets {
    root: PathBuf,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    sources: Mutex<HashMap<FileId, Source>>,
    files: Mutex<HashMap<FileId, Bytes>>,
}

impl WorldAssets {
    fn load(root: PathBuf) -> Result<Self> {
        let fonts = load_fonts(&root.join("assets"))?;
        if fonts.is_empty() {
            bail!("no fonts found in {}", root.join("assets").display());
        }
        let book = FontBook::from_fonts(&fonts);
        Ok(Self {
            root,
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            sources: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
        })
    }

    fn path(&self, id: FileId) -> FileResult<PathBuf> {
        match id.root() {
            VirtualRoot::Project => id.vpath().realize(&self.root).map_err(Into::into),
            VirtualRoot::Package(_) => Err(FileError::AccessDenied),
        }
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(source) = self.sources.lock().unwrap().get(&id) {
            return Ok(source.clone());
        }
        let path = self.path(id)?;
        let text = fs::read_to_string(&path).map_err(|error| FileError::from_io(error, &path))?;
        let source = Source::new(id, text);
        self.sources.lock().unwrap().insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(bytes) = self.files.lock().unwrap().get(&id) {
            return Ok(bytes.clone());
        }
        let path = self.path(id)?;
        let bytes = Bytes::new(
            fs::read(&path).map_err(|error| FileError::from_io(error, &path))?,
        );
        self.files.lock().unwrap().insert(id, bytes.clone());
        Ok(bytes)
    }
}

struct CardWorld<'a> {
    assets: &'a WorldAssets,
    main: Source,
}

impl World for CardWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.assets.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.assets.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            self.assets.source(id)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            Ok(Bytes::from_string(self.main.text().to_owned()))
        } else {
            self.assets.file(id)
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.assets.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

pub fn render_all(
    cards: &[CardId],
    resource_dir: &Path,
    output_dir: &Path,
) -> Result<RenderBatch> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output directory {}", output_dir.display()))?;
    let project_dir = resource_dir.join("typst-ygo");
    let assets = WorldAssets::load(project_dir.clone()).with_context(|| {
        format!(
            "failed to initialize Typst resources from {}; run with --refresh first",
            project_dir.display()
        )
    })?;
    let main_id = RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(MAIN_PATH).expect("static Typst path should be valid"),
    )
    .intern();
    let options = render_options();
    let mut batch = RenderBatch::default();
    let progress_bar = progress::items("Rendering cards", cards.len());

    for &card in cards {
        let source = Source::new(main_id, card_source(card));
        let world = CardWorld {
            assets: &assets,
            main: source,
        };
        let destination = output_dir.join(format!("{}.png", card.value));
        match render_card(&world, &options, &destination) {
            Ok(()) => batch.rendered.push(card),
            Err(error) => batch.issues.push(RenderIssue {
                card,
                reason: format!("{error:#}"),
            }),
        }
        progress_bar.inc(1);
    }
    progress_bar.finish_and_clear();
    Ok(batch)
}

pub fn print_issues(issues: &[RenderIssue]) -> Result<()> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    write_issues(&mut stderr, issues).context("failed to print render errors")
}

fn render_card(world: &dyn World, options: &RenderOptions, destination: &Path) -> Result<()> {
    let warned = typst::compile::<PagedDocument>(world);
    let document = warned.output.map_err(|errors| {
        anyhow::anyhow!("Typst compilation failed: {}", format_diagnostics(&errors))
    })?;
    let [page] = document.pages() else {
        bail!(
            "Typst produced {} pages instead of exactly one",
            document.pages().len()
        );
    };
    let pixmap = typst_render::render(page, options);
    let png = pixmap.encode_png().context("failed to encode rendered card as PNG")?;
    write_atomic(destination, &png)
}

fn render_options() -> RenderOptions {
    RenderOptions {
        pixel_per_pt: Scalar::new(PPI / POINTS_PER_INCH),
        render_bleed: false,
    }
}

fn card_source(card: CardId) -> String {
    let prefix = match card.kind {
        CardKind::Ot => "ot",
        CardKind::Rd => "rd",
    };
    format!(
        "#import \"../lib/mod.typ\": {prefix}_card_by_id, {prefix}_card_data\n\
         #let cards = {prefix}_card_data()\n\
         #{prefix}_card_by_id({}, cards: cards)\n",
        card.value
    )
}

fn load_fonts(assets_dir: &Path) -> Result<Vec<Font>> {
    let mut paths = Vec::new();
    collect_font_paths(assets_dir, &mut paths)?;
    paths.sort();
    let mut fonts = Vec::new();
    for path in paths {
        let data = fs::read(&path)
            .with_context(|| format!("failed to read font {}", path.display()))?;
        let parsed: Vec<_> = Font::iter(Bytes::new(data)).collect();
        if parsed.is_empty() {
            bail!("failed to parse font {}", path.display());
        }
        fonts.extend(parsed);
    }
    Ok(fonts)
}

fn collect_font_paths(directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to scan font directory {}", directory.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_font_paths(&path, output)?;
        } else if path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("ttf")
                || extension.eq_ignore_ascii_case("otf")
                || extension.eq_ignore_ascii_case("ttc")
        }) {
            output.push(path);
        }
    }
    Ok(())
}

fn write_atomic(destination: &Path, data: &[u8]) -> Result<()> {
    let parent = destination
        .parent()
        .context("render destination has no parent directory")?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary output in {}", parent.display()))?;
    temporary
        .write_all(data)
        .with_context(|| format!("failed to write {}", destination.display()))?;
    temporary
        .flush()
        .with_context(|| format!("failed to flush {}", destination.display()))?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to install {}", destination.display()))?;
    Ok(())
}

fn format_diagnostics(diagnostics: &[SourceDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn write_issues(mut writer: impl Write, issues: &[RenderIssue]) -> io::Result<()> {
    for issue in issues {
        writeln!(
            writer,
            "Skipping card ID {} during render: {}",
            issue.card.value, issue.reason
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(value: u64, kind: CardKind) -> CardId {
        CardId { value, kind }
    }

    #[test]
    fn generates_ot_card_source() {
        assert_eq!(
            card_source(card(483, CardKind::Ot)),
            "#import \"../lib/mod.typ\": ot_card_by_id, ot_card_data\n\
             #let cards = ot_card_data()\n\
             #ot_card_by_id(483, cards: cards)\n"
        );
    }

    #[test]
    fn generates_rd_card_source() {
        assert!(
            card_source(card(120_100_001, CardKind::Rd))
                .contains("#rd_card_by_id(120100001, cards: cards)")
        );
    }

    #[test]
    fn uses_600_ppi_scale() {
        assert_eq!(
            render_options().pixel_per_pt,
            Scalar::new(600.0 / 72.0)
        );
    }

    #[test]
    fn formats_render_issues() {
        let issues = [RenderIssue {
            card: card(7, CardKind::Ot),
            reason: "failed".to_owned(),
        }];
        let mut output = Vec::new();

        write_issues(&mut output, &issues).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Skipping card ID 7 during render: failed\n"
        );
    }
}
