package golang

// render.go parses the templates and stamps the file banner.

import (
	"bytes"
	"embed"
	"fmt"
	"go/format"
	"io"
	"text/template"

	"github.com/the-protobuf-project/protokit/header"
)

//go:embed templates/*.tmpl
var templatesFS embed.FS

// templates is parsed once, at init, so a malformed template is a startup
// failure rather than a per-file one.
var templates = template.Must(
	template.New("go").ParseFS(templatesFS, "templates/*.tmpl"),
)

// render writes one template to a writer, gofmt-formatted.
//
// Formatting happens here rather than in a build step because Go can do it
// in-process, as protoc-gen-go does: a generated file that is not canonical
// shows up as churn the first time anyone runs gofmt over the tree.
//
// A formatting failure returns the unformatted source in the error rather than
// only the parse error, because the parse error alone names a line in a file
// nobody has seen.
func render(w io.Writer, name string, data *fileData) error {
	var raw bytes.Buffer
	if err := templates.ExecuteTemplate(&raw, name, data); err != nil {
		return fmt.Errorf("render %s: %w", name, err)
	}

	formatted, err := format.Source(raw.Bytes())
	if err != nil {
		return fmt.Errorf("render %s: emitted invalid Go: %w\n%s", name, err, raw.String())
	}
	if _, err := w.Write(formatted); err != nil {
		return fmt.Errorf("render %s: %w", name, err)
	}
	return nil
}

// banner returns the generated-file header, using protokit's renderer so every
// generator in the org stamps files the same way.
func banner(version, source string) string {
	return header.Render("//", header.Info{
		PluginVersion: version,
		Source:        source,
		Notes: []string{
			"This file is the runtime's whole input: a route table it executes.",
			"It parses no templates and reads no descriptors.",
		},
	})
}
