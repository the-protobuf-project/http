package music

// codec.go encodes and decodes messages with protojson.
//
// This is why the example uses generated types rather than hand-written structs
// with JSON tags: the mapping in README §4.1 — 64-bit integers as strings, enums
// by name, Timestamp as RFC 3339, Duration as decimal seconds — is what
// protojson does, and a struct tag can only imitate it. Imitating it is exactly
// how an adapter ends up disagreeing with the clients generated from the same
// protos.

import (
	"github.com/the-protobuf-project/http/transcode-go"
	"github.com/the-protobuf-project/http/transcode-go/apierr"
	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

// marshal is the response mapping.
//
// EmitUnpopulated stays off, so a field at its default is omitted — which is
// what protojson does by default and what an AIP client expects. UseProtoNames
// stays off too: the wire spelling is lowerCamelCase.
var marshal = protojson.MarshalOptions{}

// unmarshal is the request mapping.
//
// DiscardUnknown stays off, so a body naming a field the message does not have
// is rejected. That matches the runtime's treatment of unknown query
// parameters, and for the same reason: a typo in an update call should not be a
// silent no-op.
var unmarshal = protojson.UnmarshalOptions{}

// decode decodes a request body into a message.
//
// An empty body is a valid message with every field at its default, which is
// what a POST with no body means.
func decode[M proto.Message](call *transcode.Call, message M) error {
	if len(call.Body) == 0 {
		return nil
	}
	if err := unmarshal.Unmarshal(call.Body, message); err != nil {
		return apierr.MalformedBody(err.Error(), call.Domain, call.Method.FullName)
	}
	return nil
}

// reply encodes a 200.
func reply(call *transcode.Call, message proto.Message) (*transcode.Reply, error) {
	body, err := marshal.Marshal(message)
	if err != nil {
		return nil, encodeFailed(call)
	}
	return transcode.NewReply(200, body).
		WithHeader("Content-Type", call.ResponseCodec.ContentType()), nil
}

// created encodes a 201 with a Location header. (AIP-133)
//
// The Location is what lets a client follow the response without knowing how
// resource names are formed.
func created(call *transcode.Call, message proto.Message, location string) (*transcode.Reply, error) {
	out, err := reply(call, message)
	if err != nil {
		return nil, err
	}
	out.Status = 201
	return out.WithHeader("Location", location), nil
}

// noContent encodes a 204, for a google.protobuf.Empty response with no
// response_body.
func noContent() (*transcode.Reply, error) {
	return transcode.NewReply(204, nil), nil
}

// encodeFailed is the 500 for a message that could not be encoded, which is a
// service bug rather than anything the caller did.
func encodeFailed(call *transcode.Call) error {
	return apierr.New(apierr.Internal, "The response could not be encoded.").
		WithErrorInfo("ENCODE_FAILED", call.Domain, map[string]string{
			"method": call.Method.FullName,
		})
}
