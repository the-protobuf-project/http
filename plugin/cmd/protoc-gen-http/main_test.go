package main

import "testing"

func TestProjectCreditFollowsTheModulePath(t *testing.T) {
	// A repository rename is a single edit in go.mod, and the banner follows.
	// Hardcoding the URL would leave a dead link behind with nothing to catch
	// it, since nothing in the build resolves it.
	original := modulePath
	t.Cleanup(func() { modulePath = original })

	cases := []struct{ path, want string }{
		{
			// Today: the repository is named after the binary, so this agrees
			// with what header.SetTool would have derived on its own.
			"github.com/the-protobuf-project/http/plugin",
			"http — https://github.com/the-protobuf-project/http",
		},
		{
			// After a rename, with no other edit. This is the case SetTool gets
			// wrong: it would still credit ".../http".
			"github.com/the-protobuf-project/aip-http/plugin",
			"aip-http — https://github.com/the-protobuf-project/aip-http",
		},
		{
			// A non-nested layout still resolves.
			"github.com/the-protobuf-project/http",
			"http — https://github.com/the-protobuf-project/http",
		},
	}
	for _, tc := range cases {
		modulePath = tc.path
		if got := projectCredit(); got != tc.want {
			t.Errorf("projectCredit() for %q = %q, want %q", tc.path, got, tc.want)
		}
	}
}

func TestProjectCreditIsEmptyWhenUnknown(t *testing.T) {
	// Crediting nothing beats crediting a repository that does not exist,
	// which is exactly what the tool-name-derived default produced.
	original := modulePath
	t.Cleanup(func() { modulePath = original })

	modulePath = "/plugin"
	if got := projectCredit(); got != "" {
		t.Errorf("projectCredit() = %q, want empty", got)
	}
}
