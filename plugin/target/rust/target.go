// Package rust renders the route table grpc-http executes.
//
// What it emits is the runtime's whole input: a method enum, the flattened
// match sequences, the capture spans, and the route table sorted
// most-specific-first. Nothing in it requires the runtime to understand
// protobuf, which is what lets the Rust, Go and Python runtimes agree — none of
// them interprets a template, they all execute the same compiled table.
package rust

import (
	"fmt"
	"strings"

	"github.com/the-protobuf-project/http/plugin/ir"
	"github.com/the-protobuf-project/protokit/factory"
)

// LangRust is the language key this target answers to.
const LangRust = "rust"

// Target renders the IR as Rust.
type Target struct {
	// Dir is the output directory, relative to the plugin's output root.
	// Empty means the generated files sit beside the protos.
	Dir string
}

// Name identifies the target in a factory.Registry.
func (Target) Name() string { return LangRust }

// Languages lists what this target emits.
func (Target) Languages() []string { return []string{LangRust} }

// Generate renders the model.
//
// Three files rather than one, mirroring how the runtime crate is laid out and
// keeping each under the project's line limit: the module facade, the shared
// match sequences, and the route table itself.
func (t Target) Generate(ctx factory.Ctx, model *ir.Model, lang string) error {
	if ctx.Plugin == nil {
		return fmt.Errorf("the rust target requires plugin (protoc) mode")
	}
	if lang != LangRust {
		return fmt.Errorf("unsupported language %q (this target emits %s)", lang, LangRust)
	}
	if len(model.IR.Services) == 0 {
		return nil
	}

	data, err := newFile(model)
	if err != nil {
		return err
	}

	for _, out := range []struct {
		name     string
		template string
	}{
		{"mod.rs", "mod.rs.tmpl"},
		{"matches.rs", "matches.rs.tmpl"},
		{"routes.rs", "routes.rs.tmpl"},
	} {
		file := ctx.Plugin.NewGeneratedFile(t.path(out.name), "")
		if err := render(file, out.template, data); err != nil {
			return fmt.Errorf("%s: %w", out.name, err)
		}
	}
	return nil
}

// path joins the target's output directory with a file name.
func (t Target) path(name string) string {
	if t.Dir == "" {
		return name
	}
	return strings.TrimSuffix(t.Dir, "/") + "/" + name
}
