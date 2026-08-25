package golang

// idents.go turns the shared table's neutral words and segments into the Go
// identifiers and expressions the templates emit.
//
// Only the spelling lives here. Which routes may share a sequence, and what a
// sequence is named after, are decided once in the table package so the two
// targets cannot disagree about it.

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/http/plugin/target/table"
	"github.com/the-protobuf-project/protokit/naming"
	"github.com/the-protobuf-project/protokit/service/httprule"
)

// matchIdent names a match sequence after the path shape it walks, e.g.
// matchArtistsAnyTracks.
//
// Unexported, lowercase-first: the emitted package's exported surface is the
// route table, the method constants and the codec registry. A match sequence is
// an implementation detail of the table, and exporting it would invite someone
// to build a second table out of the pieces of this one.
func matchIdent(route *httprule.Route) string {
	return "match" + camel(table.MatchWords(route))
}

// captureIdent names a capture set after its fields and its span, e.g.
// captureName1To5.
//
// The span is part of the name because two routes binding the same field from
// different positions are different sets: {name=artists/*} and
// {name=artists/*/tracks/*} both bind "name".
func captureIdent(route *httprule.Route) string {
	words := append(table.CaptureWords(route), spanWords(route)...)
	return "capture" + camel(words)
}

// spanWords renders the first capture's span as words, with "to" between them
// so the Go identifier reads as a range rather than as two numbers.
func spanWords(route *httprule.Route) []string {
	span := table.SpanWords(route)
	if len(span) != 2 {
		return span
	}
	return []string{span[0], "to", span[1]}
}

// methodConst names a method's handler-index constant, e.g. MethodGetArtist.
//
// Exported, because a service implementation switches on it.
func methodConst(name string) string { return "Method" + naming.PascalGo(name) }

// camel renders words as one PascalCase identifier fragment.
func camel(words []string) string {
	var out strings.Builder
	for _, word := range words {
		out.WriteString(naming.PascalGo(word))
	}
	return out.String()
}

// renderSegments renders a route's match sequence as Go expressions.
func renderSegments(route *httprule.Route) []string {
	out := make([]string, 0, len(route.Segments))
	for _, segment := range route.Segments {
		switch segment.Kind {
		case httprule.KindLiteral:
			out = append(out, fmt.Sprintf("route.Literal(%q)", segment.Literal))
		case httprule.KindSingle:
			out = append(out, "route.Single()")
		case httprule.KindMulti:
			out = append(out, "route.Multi()")
		}
	}
	return out
}

// renderCaptures renders a route's capture spans as Go expressions.
//
// A span that runs to the end of the path is emitted as route.ToEnd rather than
// as -1: the table is read by people, and a bare -1 index in a generated file is
// the kind of thing a reader has to go and look up.
func renderCaptures(route *httprule.Route) []string {
	out := make([]string, 0, len(route.Captures))
	for _, capture := range route.Captures {
		fields := make([]string, len(capture.Field))
		for i, part := range capture.Field {
			fields[i] = fmt.Sprintf("%q", part)
		}
		end := fmt.Sprintf("%d", capture.End)
		if capture.End == httprule.ToEnd {
			end = "route.ToEnd"
		}
		out = append(out, fmt.Sprintf(
			"{Field: []string{%s}, JSON: %q, Start: %d, End: %s}",
			strings.Join(fields, ", "), capture.Name(), capture.Start, end,
		))
	}
	return out
}
