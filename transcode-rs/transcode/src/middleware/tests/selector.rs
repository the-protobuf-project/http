//! Selector matching.

use crate::middleware::{MethodPattern, Selector};

/// Shorthand: does this selector cover this method?
fn covers(selector: &Selector, method: &str, pattern: MethodPattern) -> bool {
    selector.matches("music.v1.ArtistService", method, pattern)
}

#[test]
fn mutating_covers_every_write_pattern() {
    let selector = Selector::Mutating;
    for pattern in [
        MethodPattern::Create,
        MethodPattern::Update,
        MethodPattern::Delete,
        MethodPattern::Undelete,
        MethodPattern::BatchCreate,
        MethodPattern::BatchUpdate,
        MethodPattern::BatchDelete,
        // A custom method is assumed mutating: the conservative reading.
        MethodPattern::Custom,
    ] {
        assert!(covers(&selector, "m", pattern), "{pattern:?}");
    }
}

#[test]
fn mutating_excludes_reads() {
    let selector = Selector::Mutating;
    for pattern in [
        MethodPattern::Get,
        MethodPattern::List,
        MethodPattern::BatchGet,
    ] {
        assert!(!covers(&selector, "m", pattern), "{pattern:?}");
    }
}

#[test]
fn read_only_is_the_exact_complement_of_mutating() {
    // Every pattern is covered by exactly one of the two, which is what makes
    // a pair of policies over them exhaustive.
    for pattern in [
        MethodPattern::Get,
        MethodPattern::List,
        MethodPattern::Create,
        MethodPattern::Update,
        MethodPattern::Delete,
        MethodPattern::Undelete,
        MethodPattern::BatchGet,
        MethodPattern::BatchCreate,
        MethodPattern::BatchUpdate,
        MethodPattern::BatchDelete,
        MethodPattern::Custom,
    ] {
        assert_ne!(
            covers(&Selector::Mutating, "m", pattern),
            covers(&Selector::ReadOnly, "m", pattern),
            "{pattern:?}"
        );
    }
}

#[test]
fn service_and_method_select_by_name() {
    assert!(covers(
        &Selector::Service("music.v1.ArtistService"),
        "any",
        MethodPattern::Get
    ));
    assert!(!covers(
        &Selector::Service("music.v1.TrackService"),
        "any",
        MethodPattern::Get
    ));
    assert!(covers(
        &Selector::Method("GetArtist"),
        "GetArtist",
        MethodPattern::Get
    ));
}

#[test]
fn combinators_compose() {
    // Every mutation except deletes.
    let selector = Selector::All_(vec![
        Selector::Mutating,
        Selector::Pattern(MethodPattern::Delete).except(),
    ]);
    assert!(covers(&selector, "m", MethodPattern::Create));
    assert!(!covers(&selector, "m", MethodPattern::Delete));
    assert!(!covers(&selector, "m", MethodPattern::Get));

    let any = Selector::Any(vec![
        Selector::Pattern(MethodPattern::Get),
        Selector::Pattern(MethodPattern::List),
    ]);
    assert!(covers(&any, "m", MethodPattern::Get));
    assert!(!covers(&any, "m", MethodPattern::Create));
}
