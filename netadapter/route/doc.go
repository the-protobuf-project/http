// Package route executes a compiled route table.
//
// There is no parser here. A google.api.http template is parsed and compiled by
// protoc-gen-http at build time; what reaches the runtime is the flattened
// result — a positional sequence of [Match] segments plus the [Capture] spans
// that slice values out of a matched path. Matching is a positional walk with
// no backtracking, because the compiler guarantees a "**" can only appear last.
//
// The consequence worth stating: two runtimes generated from the same IR cannot
// disagree about what a template means, because neither one interprets
// templates. Both execute the same table, and both are held to the Go reference
// implementation in protokit/service/httprule that produced it.
//
// See README §1 for the normative rules this implements.
package route
