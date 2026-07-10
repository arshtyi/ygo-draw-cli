use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardKind {
    Ot,
    Rd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardId {
    pub value: u64,
    pub kind: CardKind,
}

#[derive(Debug, Eq, PartialEq)]
pub struct IdIssue {
    pub line: usize,
    pub value: String,
    pub reason: IdIssueReason,
}

#[derive(Debug, Eq, PartialEq)]
pub enum IdIssueReason {
    Empty,
    NotDecimal,
    TooLarge,
}

impl fmt::Display for IdIssueReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("empty line"),
            Self::NotDecimal => formatter.write_str("ID must contain decimal digits only"),
            Self::TooLarge => formatter.write_str("ID is too large"),
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct IdBatch {
    pub cards: Vec<CardId>,
    pub issues: Vec<IdIssue>,
}

impl IdBatch {
    pub fn kind_counts(&self) -> (usize, usize) {
        self.cards
            .iter()
            .fold((0, 0), |(ot, rd), card| match card.kind {
                CardKind::Ot => (ot + 1, rd),
                CardKind::Rd => (ot, rd + 1),
            })
    }
}

pub fn read_file(path: &Path) -> Result<IdBatch> {
    let file = File::open(path)
        .with_context(|| format!("failed to open ID file {}", path.display()))?;
    let reader = BufReader::new(file);
    read(reader).with_context(|| format!("failed to read ID file {}", path.display()))
}

pub fn print_issues(issues: &[IdIssue]) -> Result<()> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    write_issues(&mut stderr, issues).context("failed to print invalid card IDs")
}

fn read(reader: impl BufRead) -> io::Result<IdBatch> {
    let mut batch = IdBatch::default();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        match parse_line(&line) {
            Ok(card) => batch.cards.push(card),
            Err(reason) => batch.issues.push(IdIssue {
                line: index + 1,
                value: line,
                reason,
            }),
        }
    }
    Ok(batch)
}

fn parse_line(line: &str) -> Result<CardId, IdIssueReason> {
    let value = line.trim();
    if value.is_empty() {
        return Err(IdIssueReason::Empty);
    }
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IdIssueReason::NotDecimal);
    }

    let numeric = value
        .parse::<u64>()
        .map_err(|_| IdIssueReason::TooLarge)?;
    let kind = if value.len() <= 8 {
        CardKind::Ot
    } else {
        CardKind::Rd
    };
    Ok(CardId {
        value: numeric,
        kind,
    })
}

fn write_issues(mut writer: impl Write, issues: &[IdIssue]) -> io::Result<()> {
    for issue in issues {
        writeln!(
            writer,
            "Skipping invalid ID at line {}: {:?} ({})",
            issue.line, issue.value, issue.reason
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn classifies_eight_digits_as_ot_and_nine_as_rd() {
        assert_eq!(
            parse_line("12345678"),
            Ok(CardId {
                value: 12_345_678,
                kind: CardKind::Ot,
            })
        );
        assert_eq!(
            parse_line("123456789"),
            Ok(CardId {
                value: 123_456_789,
                kind: CardKind::Rd,
            })
        );
    }

    #[test]
    fn trims_surrounding_whitespace_before_classification() {
        assert_eq!(
            parse_line("  12345678\r"),
            Ok(CardId {
                value: 12_345_678,
                kind: CardKind::Ot,
            })
        );
    }

    #[test]
    fn records_invalid_lines_and_keeps_valid_ids() {
        let input = "12345678\n\n12x\n123456789\n18446744073709551616\n";
        let batch = read(Cursor::new(input)).unwrap();

        assert_eq!(batch.cards.len(), 2);
        assert_eq!(batch.kind_counts(), (1, 1));
        assert_eq!(
            batch.issues,
            vec![
                IdIssue {
                    line: 2,
                    value: String::new(),
                    reason: IdIssueReason::Empty,
                },
                IdIssue {
                    line: 3,
                    value: "12x".to_owned(),
                    reason: IdIssueReason::NotDecimal,
                },
                IdIssue {
                    line: 5,
                    value: "18446744073709551616".to_owned(),
                    reason: IdIssueReason::TooLarge,
                },
            ]
        );
    }

    #[test]
    fn reports_issue_line_value_and_reason() {
        let issues = [IdIssue {
            line: 7,
            value: "abc".to_owned(),
            reason: IdIssueReason::NotDecimal,
        }];
        let mut output = Vec::new();

        write_issues(&mut output, &issues).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Skipping invalid ID at line 7: \"abc\" (ID must contain decimal digits only)\n"
        );
    }
}
