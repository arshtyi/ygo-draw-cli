use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod artworks;
mod cleanup;
mod download;
mod ids;
mod progress;
mod random;
mod rendering;
mod resources;
mod summary;

/// Render Yu-Gi-Oh! cards with typst-ygo.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Refresh card data, assets, and typst-ygo before rendering.
    #[arg(long)]
    refresh: bool,

    /// Refresh resources and exit without rendering cards.
    #[arg(long, conflicts_with_all = ["refresh", "random"])]
    refresh_only: bool,

    /// Remove all downloaded resources and rendered output, then exit.
    #[arg(long, conflicts_with_all = ["refresh", "refresh_only", "random"])]
    clean: bool,

    /// Read card IDs from this file, one ID per line.
    #[arg(short, long, default_value = "cards.txt")]
    input: PathBuf,

    /// Render a random selection containing this many cards.
    #[arg(
        short,
        long,
        value_name = "COUNT",
        value_parser = parse_positive_count
    )]
    random: Option<usize>,

    /// Write rendered card images to this directory.
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Store downloaded resources in this directory.
    #[arg(long, default_value = "resources")]
    resource_dir: PathBuf,
}

fn parse_positive_count(value: &str) -> Result<usize, String> {
    let count = value
        .parse::<usize>()
        .map_err(|_| "count must be a positive integer".to_owned())?;
    if count == 0 {
        return Err("count must be greater than zero".to_owned());
    }
    Ok(count)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.clean {
        cleanup::all(&cli.resource_dir, &cli.output)?;
        return Ok(());
    }

    if cli.refresh || cli.refresh_only {
        resources::refresh(&cli.resource_dir)?;
    }
    if cli.refresh_only {
        return Ok(());
    }

    let (batch, selection) = match cli.random {
        Some(count) => (
            ids::IdBatch {
                cards: random::select(count, &cli.resource_dir)?,
                issues: Vec::new(),
            },
            summary::Selection::Random { requested: count },
        ),
        None => {
            let batch = ids::read_file(&cli.input)?;
            let lines = batch.cards.len() + batch.issues.len();
            (batch, summary::Selection::File { lines })
        }
    };
    ids::print_issues(&batch.issues)?;
    let (ot_count, rd_count) = batch.kind_counts();
    let invalid_count = batch.invalid_count();
    let duplicate_count = batch.duplicate_count();
    let artworks = artworks::prepare(&batch.cards, &cli.resource_dir)?;
    artworks::print_issues(&artworks.issues)?;
    let rendered =
        rendering::render_all(&artworks.ready, &cli.resource_dir, &cli.output)?;
    rendering::print_issues(&rendered.issues)?;
    summary::print(&summary::RunSummary {
        selection,
        valid_ids: batch.cards.len(),
        ot_ids: ot_count,
        rd_ids: rd_count,
        invalid_lines: invalid_count,
        duplicate_ids: duplicate_count,
        artwork_failures: artworks.issues.len(),
        render_failures: rendered.issues.len(),
        rendered: rendered.rendered.len(),
        output_dir: cli.output.clone(),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_expected_defaults() {
        let cli = Cli::try_parse_from(["ygo-draw"]).expect("default CLI should parse");

        assert!(!cli.refresh);
        assert!(!cli.refresh_only);
        assert!(!cli.clean);
        assert_eq!(cli.input, PathBuf::from("cards.txt"));
        assert_eq!(cli.random, None);
        assert_eq!(cli.output, PathBuf::from("output"));
        assert_eq!(cli.resource_dir, PathBuf::from("resources"));
    }

    #[test]
    fn parses_initial_workflow_options() {
        let cli = Cli::try_parse_from([
            "ygo-draw",
            "--refresh",
            "--input",
            "ids.txt",
            "--random",
            "12",
            "--output",
            "rendered",
            "--resource-dir",
            "cache",
        ])
        .expect("explicit CLI options should parse");

        assert!(cli.refresh);
        assert!(!cli.refresh_only);
        assert!(!cli.clean);
        assert_eq!(cli.input, PathBuf::from("ids.txt"));
        assert_eq!(cli.random, Some(12));
        assert_eq!(cli.output, PathBuf::from("rendered"));
        assert_eq!(cli.resource_dir, PathBuf::from("cache"));
    }

    #[test]
    fn rejects_zero_random_count() {
        assert!(Cli::try_parse_from(["ygo-draw", "--random", "0"]).is_err());
    }

    #[test]
    fn parses_refresh_only_with_custom_resource_directory() {
        let cli = Cli::try_parse_from([
            "ygo-draw",
            "--refresh-only",
            "--resource-dir",
            "cache",
        ])
        .expect("refresh-only mode should parse");

        assert!(cli.refresh_only);
        assert!(!cli.refresh);
        assert_eq!(cli.resource_dir, PathBuf::from("cache"));
    }

    #[test]
    fn refresh_only_rejects_rendering_modes() {
        assert!(
            Cli::try_parse_from(["ygo-draw", "--refresh-only", "--refresh"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["ygo-draw", "--refresh-only", "--random", "1"])
                .is_err()
        );
    }

    #[test]
    fn parses_clean_mode_with_custom_directories() {
        let cli = Cli::try_parse_from([
            "ygo-draw",
            "--clean",
            "--resource-dir",
            "cache",
            "--output",
            "rendered",
        ])
        .expect("clean mode should parse");

        assert!(cli.clean);
        assert_eq!(cli.resource_dir, PathBuf::from("cache"));
        assert_eq!(cli.output, PathBuf::from("rendered"));
    }

    #[test]
    fn clean_rejects_other_execution_modes() {
        assert!(Cli::try_parse_from(["ygo-draw", "--clean", "--refresh"]).is_err());
        assert!(
            Cli::try_parse_from(["ygo-draw", "--clean", "--refresh-only"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["ygo-draw", "--clean", "--random", "1"]).is_err()
        );
    }
}
