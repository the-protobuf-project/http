// Command protoc-gen-http generates HTTP/JSON gateways from google.api.http
// annotations.
//
// It reads the AIP vocabulary once, in Go, and emits a route table each runtime
// executes. No runtime parses a path template or reads a descriptor, which is
// what keeps the Rust, Go, and Python gateways from disagreeing about what a
// request means.
//
// Usage in buf.gen.yaml:
//
//	plugins:
//	  - local: protoc-gen-http
//	    out: gen
//	    opt:
//	      - lang=rust
//	      - domain=music.example.com
//
// lang=go emits the same table in Go, against the http-go runtime. Both are
// built from one view of the IR, so the two cannot describe different APIs.
package main

import (
	"flag"
	"fmt"
	"os"
	"runtime/debug"
	"strings"

	"github.com/the-protobuf-project/http/plugin/gateway"
	"github.com/the-protobuf-project/http/plugin/target"
	"github.com/the-protobuf-project/protokit/factory"
	"github.com/the-protobuf-project/protokit/header"
	"google.golang.org/protobuf/compiler/protogen"
)

// version is set at build time via -ldflags "-X main.version=v0.1.0", and falls
// back to the module version when the binary was installed from a tag.
var version = ""

func resolveVersion() string {
	if version != "" {
		return version
	}
	if info, ok := debug.ReadBuildInfo(); ok && info.Main.Version != "" && info.Main.Version != "(devel)" {
		return info.Main.Version
	}
	return "dev"
}

// modulePath is this plugin's Go module path, used to derive the repository the
// banner credits. Overridable at build time for the same reason version is.
var modulePath = ""

// projectCredit returns the banner's credit line.
//
// Derived from the module path rather than written out. header.SetTool derives
// the repository by stripping "protoc-gen-" from the binary name, which gives
// "http" — the right answer today, but only because the repository happens to be
// named after this binary. The two names are free to move apart, and when they
// do the derivation silently starts crediting a repository that does not exist.
//
// The module path is the one thing that must already be correct for the code to
// compile at all, so deriving from it means a rename of either name is a single
// edit in go.mod with nothing left to notice.
func projectCredit() string {
	path := modulePath
	if path == "" {
		if info, ok := debug.ReadBuildInfo(); ok {
			path = info.Main.Path
		}
	}
	// The plugin is a nested module, so trim the suffix to reach the repository.
	repo := strings.TrimSuffix(path, "/plugin")
	if repo == "" {
		// Nothing to credit is better than crediting the wrong thing.
		return ""
	}
	name := repo[strings.LastIndex(repo, "/")+1:]
	return fmt.Sprintf("%s — https://%s", name, repo)
}

func main() {
	if len(os.Args) == 2 && os.Args[1] == "--version" {
		fmt.Printf("protoc-gen-http %s\n", resolveVersion())
		return
	}

	var flags flag.FlagSet
	lang := flags.String("lang", "rust", "Target language: rust or go.")
	domain := flags.String("domain", "", "API error domain for AIP-193 ErrorInfo. Required.")
	dir := flags.String("dir", "", "Output subdirectory for the generated files.")
	strict := flags.String("strict", "", `Per-rule severity, e.g. "route:error,aip:warn".`)
	pkg := flags.String("package", "", "Go package name for lang=go. Defaults to \"gateway\".")

	protogen.Options{ParamFunc: flags.Set}.Run(func(plugin *protogen.Plugin) error {
		if *domain == "" {
			return fmt.Errorf(
				"required option \"domain\" is missing — add opt: [domain=your.api.host] " +
					"to your buf.gen.yaml plugin entry. It is stamped into every error " +
					"response and cannot be derived from the protos",
			)
		}
		header.SetTool("protoc-gen-http")
		header.SetProject(projectCredit())

		source := gateway.ProtoSource{
			Domain:  *domain,
			Strict:  *strict,
			Version: resolveVersion(),
		}
		registry := target.Registry(source, *dir, *pkg)
		ctx := factory.Ctx{Plugin: plugin}

		model, err := registry.Sources["proto"].Build(ctx)
		if err != nil {
			return err
		}

		langTarget, ok := registry.Targets[*lang]
		if !ok {
			return fmt.Errorf("unknown language %q (have: %s)", *lang, registry.TargetNames())
		}
		if err := langTarget.Generate(ctx, model, *lang); err != nil {
			return fmt.Errorf("target %s: %w", langTarget.Name(), err)
		}

		// Diagnostics are advisory: a shadowed route is legal, but a route that
		// can never be reached looks exactly like one written wrong.
		for _, diag := range model.IR.Diags {
			fmt.Fprintf(os.Stderr, "protoc-gen-http: [%s] %s: %s\n", diag.Rule, diag.Subject, diag.Message)
		}
		return nil
	})
}
