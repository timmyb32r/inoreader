use super::*;
use crate::SourceRecordId;
use chrono::{TimeZone, Utc};
use url::Url;

#[test]
fn merge_follows_conservative_state_contract() {
    let a = ArticleState {
        read: true,
        saved: true,
        trashed: true,
        ..Default::default()
    };
    let b = ArticleState {
        read: false,
        later: true,
        protect_unread: true,
        ..Default::default()
    };
    assert_eq!(
        ArticleState::merge([a, b]).unwrap(),
        ArticleState {
            read: false,
            saved: true,
            later: true,
            trashed: false,
            protect_unread: true,
            protect_restored: false
        }
    );
}

fn state(bits: u8) -> ArticleState {
    ArticleState {
        read: bits & 1 != 0,
        saved: bits & 2 != 0,
        later: bits & 4 != 0,
        trashed: bits & 8 != 0,
        protect_unread: bits & 16 != 0,
        protect_restored: bits & 32 != 0,
    }
}

#[test]
fn every_pair_of_state_combinations_obeys_merge_truth_table() {
    for left_bits in 0..64 {
        for right_bits in 0..64 {
            let left = state(left_bits);
            let right = state(right_bits);
            let merged = ArticleState::merge([left, right]).expect("two values");
            assert_eq!(merged.read, left.read && right.read);
            assert_eq!(merged.saved, left.saved || right.saved);
            assert_eq!(merged.later, left.later || right.later);
            assert_eq!(merged.trashed, left.trashed && right.trashed);
            assert_eq!(
                merged.protect_unread,
                left.protect_unread || right.protect_unread
            );
            assert_eq!(
                merged.protect_restored,
                left.protect_restored || right.protect_restored
            );
        }
    }
}

#[test]
fn split_copies_state_and_first_arrival_then_becomes_independent() {
    let original_state = ArticleState {
        read: true,
        saved: true,
        later: true,
        trashed: true,
        protect_unread: true,
        protect_restored: true,
    };
    let first_arrived_at = Utc.with_ymd_and_hms(2026, 9, 25, 1, 2, 3).unwrap();
    let key = DedupKey {
        location: ArticleLocation::from(Url::parse("https://example.test/a?x=1#fragment").unwrap()),
        title: "Original title".into(),
        description: Some("Description".into()),
    };
    let article = Article {
        id: ArticleId::new(),
        key: key.clone(),
        state: original_state,
        first_arrived_at,
        origins: vec![SourceRecordId::new()],
        revision: 7,
    };
    let changed_key = DedupKey {
        title: "Changed title".into(),
        ..key.clone()
    };
    let mut split = article.split_with_keys([
        (ArticleId::new(), key, vec![SourceRecordId::new()]),
        (ArticleId::new(), changed_key, vec![SourceRecordId::new()]),
    ]);
    assert_eq!(split.len(), 2);
    assert!(split.iter().all(|item| item.state == original_state));
    assert!(split
        .iter()
        .all(|item| item.first_arrived_at == first_arrived_at));
    split[0].state.read = false;
    assert!(
        split[1].state.read,
        "split states must be independent values"
    );
}

#[test]
fn exact_dedup_key_preserves_query_fragment_case_whitespace_and_absence() {
    let base = DedupKey {
        location: ArticleLocation::from(
            Url::parse("https://example.test/Story?q=One#part").unwrap(),
        ),
        title: " A title ".into(),
        description: None,
    };
    assert_ne!(
        base,
        DedupKey {
            location: ArticleLocation::from(
                Url::parse("https://example.test/Story?q=One").unwrap()
            ),
            ..base.clone()
        }
    );
    assert_ne!(
        base,
        DedupKey {
            title: "A title".into(),
            ..base.clone()
        }
    );
    assert_ne!(
        base,
        DedupKey {
            description: Some(String::new()),
            ..base.clone()
        }
    );
}

#[test]
fn attaching_the_same_origin_is_idempotent() {
    let origin = SourceRecordId::new();
    let mut article = Article {
        id: ArticleId::new(),
        key: DedupKey {
            location: ArticleLocation::from(Url::parse("https://example.test/a").unwrap()),
            title: "A".into(),
            description: Some("D".into()),
        },
        state: ArticleState::default(),
        first_arrived_at: Utc::now(),
        origins: Vec::new(),
        revision: 4,
    };
    article.attach_origin(origin);
    assert_eq!(article.revision, 5);
    article.attach_origin(origin);
    assert_eq!(article.revision, 5);
    assert_eq!(article.origins, vec![origin]);
}
