package rust

import (
	"embed"
	"fmt"
	"io"
	"text/template"

	"github.com/the-protobuf-project/protokit/header"
)

//go:embed templates/*.tmpl
var templatesFS embed.FS

// templates is parsed once, at init, so a malformed template is a startup
// failure rather than a per-file one.
var templates = template.Must(
	template.New("rust").ParseFS(templatesFS, "templates/*.tmpl"),
)

// render writes one template to a writer.
func render(w io.Writer, name string, data *fileData) error {
	if err := templates.ExecuteTemplate(w, name, data); err != nil {
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
