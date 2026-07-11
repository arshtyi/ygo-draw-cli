use std::path::Path;

use anyhow::{Result, bail};
use rand::Rng;
use rand::seq::SliceRandom;

use crate::artworks;
use crate::ids::{CardId, CardScope};

pub fn select(count: usize, scope: CardScope, resource_dir: &Path) -> Result<Vec<CardId>> {
    let cards = artworks::available_cards(resource_dir, scope)?;
    select_with_rng(cards, count, scope, &mut rand::rng())
}

fn select_with_rng(
    cards: Vec<CardId>,
    count: usize,
    scope: CardScope,
    rng: &mut impl Rng,
) -> Result<Vec<CardId>> {
    if count == 0 {
        bail!("random card count must be greater than zero");
    }
    let mut cards = cards;
    if count > cards.len() {
        let available = cards.len();
        bail!(
            "requested {count} random cards, but only {available} cards in {scope} have center images"
        );
    }

    cards.shuffle(rng);
    cards.truncate(count);
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use crate::ids::CardKind;

    fn cards() -> Vec<CardId> {
        vec![
            CardId {
                value: 1,
                kind: CardKind::Ot,
            },
            CardId {
                value: 2,
                kind: CardKind::Ot,
            },
            CardId {
                value: 100_000_001,
                kind: CardKind::Rd,
            },
        ]
    }

    #[test]
    fn selects_requested_number_without_duplicates() {
        let selected = select_with_rng(
            cards(),
            2,
            CardScope::Both,
            &mut StdRng::seed_from_u64(7),
        )
        .unwrap();
        let unique: HashSet<_> = selected.iter().map(|card| card.value).collect();

        assert_eq!(selected.len(), 2);
        assert_eq!(unique.len(), 2);
    }

    #[test]
    fn rejects_zero_and_counts_exceeding_requested_scope() {
        assert!(
            select_with_rng(
                cards(),
                0,
                CardScope::Both,
                &mut StdRng::seed_from_u64(7),
            )
            .is_err()
        );
        let rd_cards = vec![CardId {
            value: 100_000_001,
            kind: CardKind::Rd,
        }];
        assert!(
            select_with_rng(
                rd_cards,
                2,
                CardScope::Rd,
                &mut StdRng::seed_from_u64(7),
            )
            .is_err()
        );
    }
}
