package apierr

// detail_policy.go holds the details a policy attaches: what a caller must wait
// for, what quota they exceeded, what precondition failed, and where to read
// about it.

import "time"

// RetryInfo says how long to wait before retrying. Projected to Retry-After.
type RetryInfo struct {
	// RetryDelay is how long the caller should wait.
	RetryDelay time.Duration
}

// TypeURL implements [Detail].
func (RetryInfo) TypeURL() string { return typePrefix + "RetryInfo" }

// Fields implements [Detail].
func (r RetryInfo) Fields() map[string]any {
	return map[string]any{"retryDelay": FormatDuration(r.RetryDelay)}
}

// HelpLink is one documentation link.
type HelpLink struct {
	// Description says what the link explains.
	Description string

	// URL is the link itself.
	URL string
}

// Help carries links a caller can follow. Projected to Link headers.
type Help struct {
	// Links are the documentation links, in order.
	Links []HelpLink
}

// TypeURL implements [Detail].
func (Help) TypeURL() string { return typePrefix + "Help" }

// Fields implements [Detail].
func (h Help) Fields() map[string]any {
	links := make([]any, 0, len(h.Links))
	for _, link := range h.Links {
		links = append(links, map[string]any{
			"description": link.Description,
			"url":         link.URL,
		})
	}
	return map[string]any{"links": links}
}

// QuotaViolation is one exhausted quota.
type QuotaViolation struct {
	// Subject is what the quota is counted against — a caller id, an address,
	// a project.
	Subject string

	// Description says which limit was hit, in terms the caller can act on.
	Description string
}

// QuotaFailure reports that a call was refused for being over quota.
//
// Paired with a [RetryInfo] in practice: knowing a limit was hit without
// knowing when to come back leaves a client with nothing better to do than
// retry immediately, which is the behaviour the limit exists to stop.
type QuotaFailure struct {
	// Violations are the exhausted quotas.
	Violations []QuotaViolation
}

// TypeURL implements [Detail].
func (QuotaFailure) TypeURL() string { return typePrefix + "QuotaFailure" }

// Fields implements [Detail].
func (q QuotaFailure) Fields() map[string]any {
	violations := make([]any, 0, len(q.Violations))
	for _, v := range q.Violations {
		violations = append(violations, map[string]any{
			"subject":     v.Subject,
			"description": v.Description,
		})
	}
	return map[string]any{"violations": violations}
}

// PreconditionViolation is one unmet precondition.
type PreconditionViolation struct {
	// Type is the kind of precondition, e.a. "TOS".
	Type string

	// Subject is what it applies to.
	Subject string

	// Description says what must change before a retry can succeed.
	Description string
}

// PreconditionFailure reports that the system is in a state the call cannot
// proceed from.
type PreconditionFailure struct {
	// Violations are the unmet preconditions.
	Violations []PreconditionViolation
}

// TypeURL implements [Detail].
func (PreconditionFailure) TypeURL() string { return typePrefix + "PreconditionFailure" }

// Fields implements [Detail].
func (p PreconditionFailure) Fields() map[string]any {
	violations := make([]any, 0, len(p.Violations))
	for _, v := range p.Violations {
		violations = append(violations, map[string]any{
			"type":        v.Type,
			"subject":     v.Subject,
			"description": v.Description,
		})
	}
	return map[string]any{"violations": violations}
}

// ResourceInfo names the resource a failure was about.
//
// Worth attaching on a NOT_FOUND or a PERMISSION_DENIED: the message says what
// happened, and this says what it happened to, without the caller having to
// parse prose to find out.
type ResourceInfo struct {
	// ResourceType is the AIP-123 type, e.a. "library.example.com/Book".
	ResourceType string

	// ResourceName is the resource name, e.a. "shelves/s1/books/b9".
	ResourceName string

	// Owner is who owns it, when that is meaningful and disclosable.
	Owner string

	// Description says what went wrong with it.
	Description string
}

// TypeURL implements [Detail].
func (ResourceInfo) TypeURL() string { return typePrefix + "ResourceInfo" }

// Fields implements [Detail].
func (r ResourceInfo) Fields() map[string]any {
	return map[string]any{
		"resourceType": r.ResourceType,
		"resourceName": r.ResourceName,
		"owner":        r.Owner,
		"description":  r.Description,
	}
}

// RequestInfo carries the identifier an operator needs to find the failing
// request in a log.
type RequestInfo struct {
	// RequestID is the identifier, which must be safe to show a client.
	RequestID string

	// ServingData is opaque data the service can use to reconstruct the call.
	ServingData string
}

// TypeURL implements [Detail].
func (RequestInfo) TypeURL() string { return typePrefix + "RequestInfo" }

// Fields implements [Detail].
func (r RequestInfo) Fields() map[string]any {
	return map[string]any{"requestId": r.RequestID, "servingData": r.ServingData}
}

// FormatDuration renders a duration the way protojson spells google.protobuf
// .Duration: decimal seconds with a trailing "s", trailing zeros trimmed.
func FormatDuration(d time.Duration) string {
	seconds := d.Seconds()
	out := trimZeros(seconds)
	return out + "s"
}
