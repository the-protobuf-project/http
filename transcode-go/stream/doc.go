// Package stream implements server streaming, and the rule that makes it
// honest.
//
// A streaming response has a problem a unary one does not: HTTP commits its
// status line before the body, so a stream that fails midway has already
// claimed success. grpc-gateway resolves this by writing 200 when the stream
// opens and appending an error chunk if things go wrong, which means a client
// reading only the status cannot distinguish a failed stream from a complete
// one.
//
// README §6.2 resolves it differently, in two parts.
//
// Defer the commit. The status line is not written when the stream opens. It
// waits for either the first message or termination, so a failure before any
// output — authorization, validation, quota, not-found, which is the
// overwhelming majority — produces a real status and an ordinary error body.
// This costs only the latency of the first message, which the client is waiting
// for anyway.
//
// Truncate when the commit is spent. Once a message has gone out the status
// cannot be unspent, so the stream emits an in-band error frame, sets
// grpc-status trailers, and then terminates the body abnormally. That last step
// is what makes the failure observable: truncation is the only signal HTTP has
// left, and a transcoder that closes cleanly instead is lying about the outcome.
//
// [Writer] is the state machine; [Termination] is what a transport must act on.
package stream
