//! Query parsing: system parameters, escapes, and the reserved prefix.

use crate::gateway::Query;

#[test]
fn system_parameters_are_stripped() {
    let query = Query::parse("alt=json&fields=name&prettyPrint=true&pageSize=2").unwrap();

    assert_eq!(query.alt.as_deref(), Some("json"));
    assert_eq!(query.fields.as_deref(), Some("name"));
    assert!(query.pretty_print);
    assert_eq!(query.unknown(&["pageSize"]), Vec::<&str>::new());
}

#[test]
fn the_dollar_alias_names_a_system_parameter() {
    let query = Query::parse("$alt=sse").unwrap();
    assert_eq!(query.alt.as_deref(), Some("sse"));
}

#[test]
fn an_unknown_dollar_parameter_is_reserved() {
    assert_eq!(Query::parse("$future=1").unwrap_err(), "$future");
}

#[test]
fn a_repeated_field_keeps_every_value_in_order() {
    let query = Query::parse("tag=a&tag=b").unwrap();
    assert_eq!(query.all("tag"), ["a", "b"]);
    assert_eq!(query.get("tag"), Some("a"));
}

#[test]
fn escapes_and_plus_are_decoded() {
    let query = Query::parse("filter=name%3D%22a+b%22").unwrap();
    assert_eq!(query.get("filter"), Some("name=\"a b\""));
}

#[test]
fn a_key_without_a_value_is_present_and_empty() {
    let query = Query::parse("force").unwrap();
    assert!(query.has("force"));
    assert_eq!(query.get("force"), Some(""));
}

#[test]
fn a_malformed_escape_skips_only_its_pair() {
    let query = Query::parse("bad=%zz&good=1").unwrap();
    assert!(!query.has("bad"));
    assert_eq!(query.get("good"), Some("1"));
}

#[test]
fn unknown_names_are_sorted() {
    let query = Query::parse("b=1&a=1&known=1").unwrap();
    assert_eq!(query.unknown(&["known"]), ["a", "b"]);
}

#[test]
fn pretty_print_empty_false_and_zero_are_off() {
    // As in Go: a bare `?prettyPrint` carries no value, so it does not ask.
    assert!(!Query::parse("prettyPrint").unwrap().pretty_print);
    assert!(!Query::parse("prettyPrint=false").unwrap().pretty_print);
    assert!(!Query::parse("prettyPrint=0").unwrap().pretty_print);
    assert!(Query::parse("prettyPrint=1").unwrap().pretty_print);
}
