// Package target assembles the targets protoc-gen-http ships.
//
// One registry rather than a per-language entry point, because the sources and
// the targets are chosen from the same command line: `lang=` picks the target,
// and an unknown value should be able to name the ones that exist.
package target

import (
	"github.com/the-protobuf-project/http/plugin/ir"
	"github.com/the-protobuf-project/http/plugin/target/golang"
	"github.com/the-protobuf-project/http/plugin/target/rust"
	"github.com/the-protobuf-project/protokit/factory"
)

// Registry returns the sources and targets this plugin ships.
//
// Every target renders the same model — the service IR — so adding one is a
// rendering problem and never a question of what the API means. That is the
// property the cross-target agreement test in this package holds them to.
func Registry(source ir.ProtoSource, dir, pkg string) *factory.Registry[*ir.Model] {
	registry := factory.NewRegistry[*ir.Model]()
	registry.AddSource(source)
	registry.AddTarget(rust.Target{Dir: dir})
	registry.AddTarget(golang.Target{Dir: dir, Package: pkg})
	return registry
}
