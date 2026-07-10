use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod artworks;
mod ids;
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

    if cli.refresh {
        resources::refresh(&cli.resource_dir)?;
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
        invalid_lines: batch.issues.len(),
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
        assert_eq!(cli.input, PathBuf::from("ids.txt"));
        assert_eq!(cli.random, Some(12));
        assert_eq!(cli.output, PathBuf::from("rendered"));
        assert_eq!(cli.resource_dir, PathBuf::from("cache"));
    }

    #[test]
    fn rejects_zero_random_count() {
        assert!(Cli::try_parse_from(["ygo-draw", "--random", "0"]).is_err());
    }
}
