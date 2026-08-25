package music

// call.go reads query parameters into the typed values a handler needs.
//
// Each is what the generated handler will do inline once the Go target emits
// handlers as well as the route table. Encoding lives in codec.go, which is
// protojson.

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/the-protobuf-project/http/netadapter"
	"github.com/the-protobuf-project/http/netadapter/apierr"
)

// queryInt reads a query parameter as an int, defaulting to zero when absent.
func queryInt(call *netadapter.Call, name string) (int, error) {
	raw := call.Query.Get(name)
	if raw == "" {
		return 0, nil
	}
	value, err := strconv.Atoi(raw)
	if err != nil {
		return 0, call.Invalid(apierr.FieldViolation{
			Field:       name,
			Description: fmt.Sprintf("Expected a number, got %q.", raw),
			Reason:      "INVALID_VALUE",
		})
	}
	return value, nil
}

// queryBool reads a query parameter as a bool.
//
// Bare presence counts as true, which is what ?force means to anyone typing it,
// and matches how protojson accepts a boolean in a query string.
func queryBool(call *netadapter.Call, name string) bool {
	if !call.Query.Has(name) {
		return false
	}
	raw := call.Query.Get(name)
	return raw == "" || (raw != "false" && raw != "0")
}

// updateMask reads ?updateMask= as protojson field paths.
//
// An absent mask yields an empty list, which AIP-134 defines as "replace every
// mutable field".
func updateMask(call *netadapter.Call) []string {
	raw := call.Query.Get("updateMask")
	if raw == "" {
		return nil
	}

	var fields []string
	for _, field := range strings.Split(raw, ",") {
		if field = strings.TrimSpace(field); field != "" {
			fields = append(fields, field)
		}
	}
	return fields
}
