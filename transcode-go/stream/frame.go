package stream

// frame.go writes messages into a framing.

import (
	"encoding/binary"

	"github.com/the-protobuf-project/http/transcode-go/codec"
)

// Framer renders messages and errors into one framing's byte layout.
//
// Separate from [Writer] because the framing is pure byte arrangement while the
// writer is a state machine about what may be written when. Keeping them apart
// is what lets a new framing be added without touching the rule in README §6.2.
type Framer struct {
	// framing is the layout being written.
	framing codec.Framing

	// started records whether anything has been written, which decides between
	// the opening bytes and the separator.
	started bool
}

// NewFramer returns a framer for one framing.
func NewFramer(framing codec.Framing) *Framer { return &Framer{framing: framing} }

// Framing returns the layout being written.
func (f *Framer) Framing() codec.Framing { return f.framing }

// Message returns the bytes for one encoded message, including any leading
// delimiter.
//
// For [codec.JSONArray] the first call emits "[" and later calls emit ",", so
// the response is valid JSON at every point a reader could stop.
func (f *Framer) Message(encoded []byte) []byte {
	out := make([]byte, 0, len(encoded)+16)

	switch f.framing {
	case codec.JSONArray:
		if f.started {
			out = append(out, ',')
		} else {
			out = append(out, '[')
		}
		out = append(out, encoded...)
	case codec.SSE:
		out = append(out, "event: message\ndata: "...)
		out = append(out, encoded...)
		out = append(out, "\n\n"...)
	case codec.LineDelimited:
		out = append(out, encoded...)
		out = append(out, '\n')
	case codec.LengthPrefixed:
		// Four-byte big-endian length, matching gRPC's framing minus the
		// compression flag.
		out = binary.BigEndian.AppendUint32(out, uint32(len(encoded)))
		out = append(out, encoded...)
	}

	f.started = true
	return out
}

// Close returns the bytes that close a stream that completed cleanly.
func (f *Framer) Close() []byte {
	if f.framing == codec.JSONArray && !f.started {
		// An empty JSON array still has to be well-formed.
		return []byte("[]")
	}
	return f.framing.Close()
}

// Error returns the terminal error frame for a stream that failed after
// committing.
//
// The encoded bytes are the AIP-193 envelope, already serialized by the
// negotiated codec. The frame goes out before the body is truncated, so a
// client that does read the body learns why.
func (f *Framer) Error(encoded []byte) []byte {
	out := make([]byte, 0, len(encoded)+24)

	switch f.framing {
	case codec.JSONArray:
		if f.started {
			out = append(out, ',')
		} else {
			out = append(out, '[')
		}
		out = append(out, encoded...)
		out = append(out, ']')
	case codec.SSE:
		// A distinct event name, so a browser handler can bind to it rather
		// than having to inspect every message.
		out = append(out, "event: error\ndata: "...)
		out = append(out, encoded...)
		out = append(out, "\n\n"...)
	case codec.LineDelimited:
		out = append(out, encoded...)
		out = append(out, '\n')
	case codec.LengthPrefixed:
		out = binary.BigEndian.AppendUint32(out, uint32(len(encoded)))
		out = append(out, encoded...)
	}

	f.started = true
	return out
}

// Keepalive returns a keepalive comment, for framings that have one.
//
// SSE connections are reaped by intermediaries when idle, and a stream that is
// merely waiting looks identical to one that has died.
func (f *Framer) Keepalive() []byte {
	if f.framing == codec.SSE {
		return []byte(": keepalive\n\n")
	}
	return nil
}
