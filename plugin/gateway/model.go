// Package gateway wires the service IR into protokit's source/target factory.
//
// The split follows the one protokit draws: a Source builds a model, a Target
// renders it, and a Registry pairs them. The model here is the service IR, so
// every target — Rust, Go, Python, OpenAPI — reads exactly the same thing and
// cannot drift in what it believes the API to be.
package gateway

import (
	"fmt"

	"github.com/the-protobuf-project/protokit/factory"
	"github.com/the-protobuf-project/protokit/service"
)

// Model is what a target renders: the service IR and the options it was built
// with.
type Model struct {
	// IR is the built intermediate representation.
	IR *service.IR

	// Version is the plugin build version, written into every file banner.
	Version string
}

// ProtoSource builds the model from the CodeGeneratorRequest protoc hands the
// plugin.
type ProtoSource struct {
	// Domain is the API's error domain, stamped into every AIP-193 ErrorInfo.
	Domain string

	// Strict is the per-rule severity spec for recoverable problems.
	Strict string

	// Version is the plugin build version.
	Version string
}

// Name identifies the source in a factory.Registry.
func (ProtoSource) Name() string { return "proto" }

// Build reads the descriptors and produces the IR.
func (s ProtoSource) Build(ctx factory.Ctx) (*Model, error) {
	if ctx.Plugin == nil {
		return nil, fmt.Errorf("the proto source requires plugin (protoc) mode")
	}

	ir, err := service.Build(ctx.Plugin, service.Options{
		Domain: s.Domain,
		Strict: s.Strict,
	})
	if err != nil {
		return nil, err
	}
	return &Model{IR: ir, Version: s.Version}, nil
}
