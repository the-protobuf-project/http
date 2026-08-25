package target_test

// harness_test.go builds the IR once, from the real example protos, and renders
// it through every target.
//
// From the protos rather than from a vendored descriptor set, so the fixture
// cannot drift from the API the examples actually serve — a golden file that
// silently stops describing the protos is worse than no golden file.

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/the-protobuf-project/http/plugin/ir"
	"github.com/the-protobuf-project/http/plugin/target"
	"github.com/the-protobuf-project/protokit/factory"
	"google.golang.org/protobuf/compiler/protogen"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/descriptorpb"
	"google.golang.org/protobuf/types/pluginpb"
)

// testDomain is the error domain the fixture builds with.
const testDomain = "music.example.com"

// generate renders the example protos through one target and returns the files
// by name.
func generate(t *testing.T, lang string) map[string]string {
	t.Helper()

	plugin := newPlugin(t)
	source := ir.ProtoSource{Domain: testDomain, Version: "test"}
	ctx := factory.Ctx{Plugin: plugin}

	model, err := source.Build(ctx)
	if err != nil {
		t.Fatalf("build model: %v", err)
	}

	registry := target.Registry(source, "", "gateway")
	selected, ok := registry.Targets[lang]
	if !ok {
		t.Fatalf("no target for %q; have %s", lang, registry.TargetNames())
	}
	if err := selected.Generate(ctx, model, lang); err != nil {
		t.Fatalf("generate %s: %v", lang, err)
	}

	response := plugin.Response()
	if response.Error != nil {
		t.Fatalf("plugin error: %s", response.GetError())
	}

	files := map[string]string{}
	for _, file := range response.File {
		files[file.GetName()] = file.GetContent()
	}
	return files
}

// newPlugin compiles the example protos into a protogen.Plugin.
func newPlugin(t *testing.T) *protogen.Plugin {
	t.Helper()

	protoDir, err := filepath.Abs("../../examples/protobuf")
	if err != nil {
		t.Fatalf("resolve proto dir: %v", err)
	}
	if _, err := os.Stat(protoDir); err != nil {
		t.Skipf("example protos not present: %v", err)
	}
	if _, err := exec.LookPath("buf"); err != nil {
		t.Skip("buf is not installed")
	}

	out := filepath.Join(t.TempDir(), "music.binpb")
	cmd := exec.Command("buf", "build", "-o", out)
	cmd.Dir = protoDir
	if combined, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("buf build: %v\n%s", err, combined)
	}

	raw, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("read descriptor set: %v", err)
	}
	var set descriptorpb.FileDescriptorSet
	if err := proto.Unmarshal(raw, &set); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	var toGenerate []string
	for _, file := range set.File {
		if strings.HasPrefix(file.GetName(), "music/") {
			toGenerate = append(toGenerate, file.GetName())
		}
	}

	plugin, err := protogen.Options{}.New(&pluginpb.CodeGeneratorRequest{
		FileToGenerate: toGenerate,
		ProtoFile:      set.File,
	})
	if err != nil {
		t.Fatalf("protogen: %v", err)
	}
	return plugin
}

// languages is every target the plugin ships, for a test that must cover all of
// them rather than a list that drifts.
func languages(t *testing.T) []string {
	t.Helper()

	registry := target.Registry(ir.ProtoSource{Domain: testDomain}, "", "gateway")
	var langs []string
	for name := range registry.Targets {
		langs = append(langs, name)
	}
	if len(langs) < 2 {
		t.Fatalf("expected at least two targets, got %v", langs)
	}
	return langs
}
