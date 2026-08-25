// Package golang renders the route table the http-go runtime executes.
//
// What it emits is that runtime's whole input: a method table, the flattened
// match sequences, the capture spans, and the route table sorted
// most-specific-first. It is the same table the Rust target emits, in Go
// syntax — both are built from the shared view in plugin/target/table, so
// neither can decide something about the API the other did not.
package golang

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/http/plugin/ir"
	"github.com/the-protobuf-project/protokit/factory"
)

// LangGo is the language key this target answers to.
const LangGo = "go"

// Target renders the IR as Go.
type Target struct {
	// Dir is the output directory, relative to the plugin's output root.
	// Empty means the generated files sit beside the protos.
	Dir string

	// Package is the Go package name for the emitted files. Empty means
	// "routes", which names what the package holds: the compiled route table a
	// runtime scans, plus the method and codec tables it resolves against.
	Package string
}

// Name identifies the target in a factory.Registry.
func (Target) Name() string { return LangGo }

// Languages lists what this target emits.
func (Target) Languages() []string { return []string{LangGo} }

// Generate renders the model.
//
// Three files, matching the Rust target's split: the method and codec tables,
// the shared match sequences and capture spans, and the route table itself.
// Splitting a generated file is not cosmetic here — the route table is the part
// that grows with an API, and keeping it alone means a diff over it shows only
// what changed about routing.
func (t Target) Generate(ctx factory.Ctx, model *ir.Model, lang string) error {
	if ctx.Plugin == nil {
		return fmt.Errorf("the go target requires plugin (protoc) mode")
	}
	if lang != LangGo {
		return fmt.Errorf("unsupported language %q (this target emits %s)", lang, LangGo)
	}
	if len(model.IR.Services) == 0 {
		return nil
	}

	data, err := newFile(model, t.pkg())
	if err != nil {
		return err
	}

	for _, out := range []struct {
		name     string
		template string
	}{
		{"tables.go", "tables.go.tmpl"},
		{"matches.go", "matches.go.tmpl"},
		{"routes.go", "routes.go.tmpl"},
	} {
		file := ctx.Plugin.NewGeneratedFile(t.path(out.name), "")
		if err := render(file, out.template, data); err != nil {
			return fmt.Errorf("%s: %w", out.name, err)
		}
	}
	return nil
}

// pkg is the Go package name to emit.
func (t Target) pkg() string {
	if t.Package != "" {
		return t.Package
	}
	return "routes"
}

// path joins the target's output directory with a file name.
func (t Target) path(name string) string {
	if t.Dir == "" {
		return name
	}
	return strings.TrimSuffix(t.Dir, "/") + "/" + name
}
