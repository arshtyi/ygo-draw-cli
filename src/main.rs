use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod resources;

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
    #[arg(short, long, value_name = "COUNT")]
    random: Option<usize>,

    /// Write rendered card images to this directory.
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Store downloaded resources in this directory.
    #[arg(long, default_value = "resources")]
    resource_dir: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.refresh {
        resources::refresh(&cli.resource_dir)?;
    }

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
}
