use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::ids::CardScope;

#[derive(Debug, Eq, PartialEq)]
pub enum Selection {
    File { lines: usize },
    Random {
        requested: usize,
        scope: CardScope,
    },
    All { scope: CardScope },
}

#[derive(Debug, Eq, PartialEq)]
pub struct RunSummary {
    pub selection: Selection,
    pub valid_ids: usize,
    pub ot_ids: usize,
    pub rd_ids: usize,
    pub invalid_lines: usize,
    pub duplicate_ids: usize,
    pub artwork_failures: usize,
    pub render_failures: usize,
    pub rendered: usize,
    pub output_dir: PathBuf,
}

impl RunSummary {
    pub fn skipped(&self) -> usize {
        self.invalid_lines
            + self.duplicate_ids
            + self.artwork_failures
            + self.render_failures
    }
}

impl fmt::Display for RunSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "Summary")?;
        match &self.selection {
            Selection::File { lines } => {
                writeln!(formatter, "  Selection: file ({lines} input lines)")?;
            }
            Selection::Random { requested, scope } => {
                writeln!(
                    formatter,
                    "  Selection: random ({requested} requested, {})",
                    scope
                )?;
            }
            Selection::All { scope } => {
                writeln!(formatter, "  Selection: all ({scope})")?;
            }
        }
        writeln!(
            formatter,
            "  Valid IDs: {} (OT: {}, RD: {})",
            self.valid_ids, self.ot_ids, self.rd_ids
        )?;
        writeln!(formatter, "  Invalid lines: {}", self.invalid_lines)?;
        writeln!(formatter, "  Duplicate IDs: {}", self.duplicate_ids)?;
        writeln!(
            formatter,
            "  Center image failures: {}",
            self.artwork_failures
        )?;
        writeln!(formatter, "  Render failures: {}", self.render_failures)?;
        writeln!(formatter, "  Rendered: {}", self.rendered)?;
        writeln!(formatter, "  Skipped: {}", self.skipped())?;
        write!(
            formatter,
            "  Output directory: {}",
            self.output_dir.display()
        )
    }
}

pub fn print(summary: &RunSummary) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{summary}").context("failed to print run summary")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> RunSummary {
        RunSummary {
            selection: Selection::File { lines: 8 },
            valid_ids: 5,
            ot_ids: 3,
            rd_ids: 2,
            invalid_lines: 2,
            duplicate_ids: 1,
            artwork_failures: 1,
            render_failures: 1,
            rendered: 3,
            output_dir: PathBuf::from("rendered"),
        }
    }

    #[test]
    fn totals_all_skip_stages() {
        assert_eq!(summary().skipped(), 5);
    }

    #[test]
    fn formats_complete_summary() {
        assert_eq!(
            summary().to_string(),
            "Summary\n\
             \x20\x20Selection: file (8 input lines)\n\
             \x20\x20Valid IDs: 5 (OT: 3, RD: 2)\n\
             \x20\x20Invalid lines: 2\n\
             \x20\x20Duplicate IDs: 1\n\
             \x20\x20Center image failures: 1\n\
             \x20\x20Render failures: 1\n\
             \x20\x20Rendered: 3\n\
             \x20\x20Skipped: 5\n\
             \x20\x20Output directory: rendered"
        );
    }

    #[test]
    fn formats_random_selection() {
        let mut summary = summary();
        summary.selection = Selection::Random {
            requested: 5,
            scope: CardScope::Rd,
        };

        assert!(
            summary
                .to_string()
                .contains("Selection: random (5 requested, RD)")
        );
    }

    #[test]
    fn formats_all_selection() {
        let mut summary = summary();
        summary.selection = Selection::All {
            scope: CardScope::Both,
        };

        assert!(summary.to_string().contains("Selection: all (OT and RD)"));
    }
}
