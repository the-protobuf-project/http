package transcode

// bind.go turns a matched request into a resolved call: captures, query, body,
// and the two codecs.

import (
	"io"
	"net/http"
	"time"

	"github.com/the-protobuf-project/http/transcode-go/apierr"
	"github.com/the-protobuf-project/http/transcode-go/codec"
	"github.com/the-protobuf-project/http/transcode-go/route"
)

// newCall resolves a matched request into a [Call].
//
// The order is the one README §2 fixes: path captures, then the body, then the
// query parameters — each stage failing rather than overwriting an earlier one.
func (h *Handler) newCall(r *http.Request, resolved *route.Resolution) (*Call, error) {
	method, ok := h.table.Method(resolved.Route.Handler)
	if !ok {
		return nil, apierr.BindingMismatch(resolved.Route.Template, h.domain, "")
	}

	captures, err := resolved.Captures()
	if err != nil {
		// The path matched but a captured segment is malformed, so this is a
		// 400 naming the field rather than a 404.
		return nil, malformedPath(err, h.domain, method.FullName)
	}

	query, reserved := parseQuery(r.URL.RawQuery)
	if reserved != "" {
		return nil, apierr.ReservedQueryParameter(reserved, h.domain, method.FullName)
	}

	body, err := h.readBody(r)
	if err != nil {
		return nil, err
	}

	negotiation := codec.Negotiation{
		ContentType: r.Header.Get("Content-Type"),
		Accept:      r.Header.Get("Accept"),
		Alt:         query.Alt,
		Streaming:   method.ServerStream,
	}
	requestCodec, err := codec.RequestCodec(h.codecs, negotiation, h.domain)
	if err != nil {
		return nil, err
	}
	responseCodec, err := codec.ResponseCodec(h.codecs, negotiation, requestCodec, h.domain)
	if err != nil {
		return nil, err
	}

	return &Call{
		Request:       r,
		cx:            h.newRouteCx(r, method, resolved.Route, captures),
		started:       time.Now(),
		Method:        method,
		Handler:       resolved.Route.Handler,
		Route:         resolved.Route,
		Path:          captures,
		Query:         query,
		Body:          body,
		RequestCodec:  requestCodec,
		ResponseCodec: responseCodec,
		Domain:        h.domain,
	}, nil
}

// readBody reads the request body under the configured limit.
//
// The limit is enforced by reading one byte past it rather than trusting
// Content-Length, which a client controls and a chunked request omits entirely.
func (h *Handler) readBody(r *http.Request) ([]byte, error) {
	if r.Body == nil {
		return nil, nil
	}
	limit := h.options.MaxRequestBody
	body, err := io.ReadAll(io.LimitReader(r.Body, limit+1))
	if err != nil {
		return nil, apierr.MalformedBody(err.Error(), h.domain, "")
	}
	if int64(len(body)) > limit {
		return nil, apierr.PayloadTooLarge(limit, h.domain)
	}
	return body, nil
}
