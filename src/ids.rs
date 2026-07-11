use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CardKind {
    Ot,
    Rd,
}

impl CardKind {
    pub fn directory_name(self) -> &'static str {
        match self {
            Self::Ot => "ot",
            Self::Rd => "rd",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum CardScope {
    Ot,
    Rd,
    #[default]
    Both,
}

impl CardScope {
    pub fn includes(self, kind: CardKind) -> bool {
        matches!(
            (self, kind),
            (Self::Ot, CardKind::Ot)
                | (Self::Rd, CardKind::Rd)
                | (Self::Both, _)
        )
    }
}

impl fmt::Display for CardScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ot => "OT",
            Self::Rd => "RD",
            Self::Both => "OT and RD",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    Duplicate { first_line: usize },
}

impl fmt::Display for IdIssueReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("empty line"),
            Self::NotDecimal => formatter.write_str("ID must contain decimal digits only"),
            Self::TooLarge => formatter.write_str("ID is too large"),
            Self::Duplicate { first_line } => {
                write!(formatter, "duplicate of line {first_line}")
            }
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

    pub fn duplicate_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| matches!(&issue.reason, IdIssueReason::Duplicate { .. }))
            .count()
    }

    pub fn invalid_count(&self) -> usize {
        self.issues.len() - self.duplicate_count()
    }
}

impl From<Vec<CardId>> for IdBatch {
    fn from(cards: Vec<CardId>) -> Self {
        Self {
            cards,
            issues: Vec::new(),
        }
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
    let mut seen = HashMap::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        let line_number = index + 1;
        match parse_line(&line) {
            Ok(card) => {
                if let Some(&first_line) = seen.get(&card) {
                    batch.issues.push(IdIssue {
                        line: line_number,
                        value: line,
                        reason: IdIssueReason::Duplicate { first_line },
                    });
                } else {
                    seen.insert(card, line_number);
                    batch.cards.push(card);
                }
            }
            Err(reason) => batch.issues.push(IdIssue {
                line: line_number,
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
        match &issue.reason {
            IdIssueReason::Duplicate { .. } => writeln!(
                writer,
                "Skipping duplicate ID at line {}: {:?} ({})",
                issue.line, issue.value, issue.reason
            )?,
            _ => writeln!(
                writer,
                "Skipping invalid ID at line {}: {:?} ({})",
                issue.line, issue.value, issue.reason
            )?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn card_scope_includes_requested_kinds() {
        assert_eq!(CardScope::default(), CardScope::Both);
        assert!(CardScope::Ot.includes(CardKind::Ot));
        assert!(!CardScope::Ot.includes(CardKind::Rd));
        assert!(!CardScope::Rd.includes(CardKind::Ot));
        assert!(CardScope::Rd.includes(CardKind::Rd));
        assert!(CardScope::Both.includes(CardKind::Ot));
        assert!(CardScope::Both.includes(CardKind::Rd));
    }

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

    #[test]
    fn skips_duplicates_and_records_the_first_line() {
        let input = "483\n120100001\n 483 \n120100001\n";

        let batch = read(Cursor::new(input)).unwrap();

        assert_eq!(batch.cards.len(), 2);
        assert_eq!(batch.duplicate_count(), 2);
        assert_eq!(batch.invalid_count(), 0);
        assert_eq!(
            batch.issues,
            vec![
                IdIssue {
                    line: 3,
                    value: " 483 ".to_owned(),
                    reason: IdIssueReason::Duplicate { first_line: 1 },
                },
                IdIssue {
                    line: 4,
                    value: "120100001".to_owned(),
                    reason: IdIssueReason::Duplicate { first_line: 2 },
                },
            ]
        );
    }

    #[test]
    fn reports_duplicate_line() {
        let issues = [IdIssue {
            line: 4,
            value: "483".to_owned(),
            reason: IdIssueReason::Duplicate { first_line: 1 },
        }];
        let mut output = Vec::new();

        write_issues(&mut output, &issues).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Skipping duplicate ID at line 4: \"483\" (duplicate of line 1)\n"
        );
    }
}
