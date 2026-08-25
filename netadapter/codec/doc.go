// Package codec holds the codec table and content negotiation.
//
// Path captures and query parameters arrive as strings and are parsed by the
// generated typed setters, so the codec boundary is narrower than it first
// appears: it covers the request body, the response body, and how a stream is
// delimited. Nothing else.
//
// A codec is described here by metadata alone — an [Entry] — because
// negotiation never needs to encode anything. The concrete encoder is reached
// by the generated handler switching on [Entry.Index], which keeps encoding
// direct: no interface dispatch and no reflection on the request path.
//
// See README §3 for the negotiation order this implements.
package codec
