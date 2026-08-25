package rust

// view.go adapts the shared route-table view into the data a Rust template
// renders. Naming and ordering are the table package's decisions; what is left
// here is Rust spelling.

import (
	"fmt"

	"github.com/the-protobuf-project/http/plugin/ir"
	"github.com/the-protobuf-project/http/plugin/target/table"
)

// newFile builds the view for a model.
func newFile(model *ir.Model) (*fileData, error) {
	entries := table.Entries(model.IR)
	table.Sort(entries)
	methods, index := table.Methods(model.IR)

	data := &fileData{
		Banner: banner(model.Version, table.Sources(model.IR)),
		Domain: model.IR.Domain,
		Codecs: defaultCodecs(),
	}
	for _, method := range methods {
		data.Methods = append(data.Methods, methodData{
			Variant:      method.Name,
			FullName:     method.FullName,
			Service:      method.Service,
			Doc:          method.Doc,
			Mutating:     method.Mutating,
			ServerStream: method.ServerStream,
			Index:        method.Index,
		})
	}

	matches := table.NewDedup()
	captures := table.NewDedup()
	seenMatch := map[string]bool{}
	seenCapture := map[string]bool{}

	for _, entry := range entries {
		route := entry.Binding.Route

		matchKey := table.MatchKey(route)
		matchName := matches.Add(matchKey, matchIdent(route))
		if !seenMatch[matchName] {
			seenMatch[matchName] = true
			data.Matches = append(data.Matches, matchData{
				Ident:    matchName,
				Doc:      table.Shape(route),
				Segments: renderSegments(route),
			})
		}

		captureName := "NONE"
		if len(route.Captures) > 0 {
			captureName = captures.Add(table.CaptureKey(route), captureIdent(route))
			if !seenCapture[captureName] {
				seenCapture[captureName] = true
				data.Captures = append(data.Captures, captureData{
					Ident: captureName,
					Doc:   table.CaptureDoc(route),
					Spans: renderCaptures(route),
				})
			}
		}

		variant, ok := index[entry.Method.FullName]
		if !ok {
			return nil, fmt.Errorf("method %s has bindings but no handler index", entry.Method.FullName)
		}
		data.Routes = append(data.Routes, routeData{
			HTTPMethod:   entry.Binding.HTTPMethod,
			MatchIdent:   matchName,
			CaptureIdent: captureName,
			Verb:         entry.Binding.Verb,
			Template:     entry.Binding.Template.Raw,
			Variant:      data.Methods[variant].Variant,
		})
	}
	return data, nil
}

// defaultCodecs returns the codec registry, JSON first so it is the default.
func defaultCodecs() []codecData {
	return []codecData{{
		Name:       "json",
		MediaTypes: []string{"application/json"},
		Framing:    "JsonArray",
		Index:      0,
	}}
}
