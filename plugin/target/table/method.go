package table

// method.go indexes the methods a route table dispatches to, and renders the
// documentation every target repeats.

import (
	"fmt"
	"sort"
	"strings"

	"github.com/the-protobuf-project/protokit/service"
)

// MethodRef is one entry of the generated method table.
//
// It is what a matched route resolves to: the handler index, plus the names a
// runtime needs for tracing, error metadata, and policy selection. Nothing here
// requires understanding protobuf, which is the property that lets the runtimes
// stay schema-free.
type MethodRef struct {
	// Name is the RPC's own name, e.g. "GetArtist". Targets case it to suit.
	Name string

	// FullName is the fully-qualified proto method name, which is what a
	// tracing span and an ErrorInfo's metadata report.
	FullName string

	// Service is the fully-qualified proto service name.
	Service string

	// Doc is the primary binding rendered as one line of prose.
	Doc string

	// Pattern is the method's AIP classification, spelled as the runtimes spell
	// it: "Get", "BatchCreate", "Custom". A target prefixes it to reach its own
	// constant, so the two runtimes cannot end up classifying a method
	// differently.
	Pattern string

	// Mutating is whether the method changes state, derived from its AIP
	// pattern rather than from its name — so a policy written against it covers
	// a method added later.
	Mutating bool

	// ServerStream is whether the method streams its response, which decides
	// whether a runtime may write the status line when the response opens.
	//
	// There is no client-stream counterpart: the service IR rejects a
	// google.api.http rule on a client-streaming method outright, since HTTP
	// has no honest mapping for one.
	ServerStream bool

	// Index is the handler index. Assigned in declaration order, never in route
	// order: the table reorders by specificity, and a handler index that moved
	// with it would renumber every handler when a template changed.
	Index int
}

// Methods returns the methods a route table can dispatch to, in handler-index
// order, and a lookup from full proto name to that index.
//
// A method with no bindings is skipped: it is legal, and simply means the RPC
// is not exposed over HTTP.
func Methods(ir *service.IR) ([]MethodRef, map[string]int) {
	var methods []MethodRef
	index := map[string]int{}

	for _, svc := range ir.Services {
		for _, method := range svc.Methods {
			if len(method.Bindings) == 0 {
				continue
			}
			index[method.FullName] = len(methods)
			methods = append(methods, MethodRef{
				Name:         method.Name,
				FullName:     method.FullName,
				Service:      svc.FullName,
				Doc:          BindingDoc(method.Bindings[0]),
				Pattern:      method.Pattern.String(),
				Mutating:     method.Mutating,
				ServerStream: method.ServerStream,
				Index:        len(methods),
			})
		}
	}
	return methods, index
}

// BindingDoc renders a binding as a one-line doc comment: the HTTP method, the
// template, and what the body binds.
func BindingDoc(binding *service.Binding) string {
	doc := binding.HTTPMethod + " " + binding.Template.Raw
	if binding.Body != nil {
		if binding.Body.Wildcard {
			doc += ` with body: "*"`
		} else {
			doc += fmt.Sprintf(" with body: %q", binding.Body.Field.JSON)
		}
	}
	return doc
}

// Sources returns the protos a build read, comma-joined, for the file banner.
//
// Sorted rather than left in file order so the banner does not churn when the
// order protoc hands over the files changes.
func Sources(ir *service.IR) string {
	seen := map[string]bool{}
	var files []string
	for _, svc := range ir.Services {
		if !seen[svc.File] {
			seen[svc.File] = true
			files = append(files, svc.File)
		}
	}
	sort.Strings(files)
	return strings.Join(files, ", ")
}
