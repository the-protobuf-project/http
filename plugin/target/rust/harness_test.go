package rust

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/the-protobuf-project/http/plugin/ir"
	"github.com/the-protobuf-project/protokit/factory"
	"google.golang.org/protobuf/compiler/protogen"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/descriptorpb"
	"google.golang.org/protobuf/types/pluginpb"
)

// generate runs the target over the example protos and returns the emitted
// files by name.
//
// It builds the descriptor set with buf rather than vendoring one, so the
// fixture cannot drift from the protos the example actually serves.
func generate(t *testing.T) map[string]string {
	t.Helper()

	protoDir, err := filepath.Abs("../../../examples/protobuf")
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

	source := ir.ProtoSource{Domain: "music.example.com", Version: "test"}
	ctx := factory.Ctx{Plugin: plugin}

	model, err := source.Build(ctx)
	if err != nil {
		t.Fatalf("build model: %v", err)
	}
	if err := (Target{}).Generate(ctx, model, LangRust); err != nil {
		t.Fatalf("generate: %v", err)
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
