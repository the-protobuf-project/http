package rust

// idents.go turns the shared table's neutral words and segments into the Rust
// identifiers and expressions the templates emit.
//
// Only the spelling lives here. Which routes may share a sequence, and what a
// sequence is named after, are decided once in the table package so the three
// targets cannot disagree about it.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/http/plugin/target/table"
	"github.com/the-protobuf-project/protokit/naming"
	"github.com/the-protobuf-project/protokit/service/httprule"
)

// matchIdent names a match sequence after the path shape it walks, e.g.
// M_ARTISTS_ANY_TRACKS.
func matchIdent(route *httprule.Route) string {
	return "M_" + screaming(table.MatchWords(route))
}

// captureIdent names a capture set after its fields and its span, e.g.
// CAP_NAME_1_5.
//
// The span is part of the name because two routes binding the same field from
// different positions are different sets: `{name=artists/*}` and
// `{name=artists/*/tracks/*}` both bind "name".
func captureIdent(route *httprule.Route) string {
	words := append(table.CaptureWords(route), table.SpanWords(route)...)
	return "CAP_" + screaming(words)
}

// screaming renders words as one SCREAMING_SNAKE identifier fragment.
func screaming(words []string) string {
	parts := make([]string, len(words))
	for i, word := range words {
		parts[i] = naming.ScreamingSnake(word)
	}
	return strings.Join(parts, "_")
}

// renderSegments renders a route's match sequence as Rust expressions.
func renderSegments(route *httprule.Route) []string {
	out := make([]string, 0, len(route.Segments))
	for _, segment := range route.Segments {
		switch segment.Kind {
		case httprule.KindLiteral:
			out = append(out, fmt.Sprintf("Match::Literal(%q)", segment.Literal))
		case httprule.KindSingle:
			out = append(out, "Match::Single")
		case httprule.KindMulti:
			out = append(out, "Match::Multi")
		}
	}
	return out
}

// renderCaptures renders a route's capture spans as Rust expressions.
//
// A span that runs to the end of the path is emitted as the TO_END constant
// rather than as -1: the table is read by people, and a bare -1 index in a
// generated file is the kind of thing a reader has to go and look up.
func renderCaptures(route *httprule.Route) []string {
	out := make([]string, 0, len(route.Captures))
	for _, capture := range route.Captures {
		fields := make([]string, len(capture.Field))
		for i, part := range capture.Field {
			fields[i] = fmt.Sprintf("%q", part)
		}
		end := fmt.Sprintf("%d", capture.End)
		if capture.End == httprule.ToEnd {
			end = "TO_END"
		}
		out = append(out, fmt.Sprintf(
			"Capture { field: &[%s], json: %q, start: %d, end: %s }",
			strings.Join(fields, ", "), capture.Name(), capture.Start, end,
		))
	}
	return out
}
