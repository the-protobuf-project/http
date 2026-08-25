//! One compiled binding, and the positional walk that matches it.

use super::decode::{CaptureError, decode_segment};
use super::segment::{Capture, Match};
use std::borrow::Cow;

/// One compiled `google.api.http` binding.
///
/// Every field is `&'static` because a route table is emitted as a `static`
/// array by the generator: the whole table is baked into the binary, costs no
/// allocation, and cannot be mutated at runtime.
#[derive(Debug, Clone)]
pub struct Route {
    /// The HTTP method this binding answers, e.g. `"GET"`.
    pub method: &'static str,

    /// The flattened match sequence. No element is a variable — the compiler
    /// expanded those into their sub-segments.
    pub segments: &'static [Match],

    /// The AIP-136 custom verb without its colon, or `""` when the binding
    /// declares none.
    pub verb: &'static str,

    /// The capture spans, in template order.
    pub captures: &'static [Capture],

    /// The original template text, e.g. `"/v1/{name=shelves/*/books/*}"`.
    ///
    /// Carried for diagnostics, tracing spans, and error messages only. It is
    /// never parsed; parsing happens once, in the generator.
    pub template: &'static str,

    /// Index into the generated handler table, which is how a match becomes a
    /// call without dynamic dispatch.
    pub handler: usize,
}

impl Route {
    /// Whether the route ends in a `**`.
    #[inline]
    pub fn has_multi(&self) -> bool {
        matches!(self.segments.last(), Some(Match::Multi))
    }

    /// The number of leading segments that must match one-to-one: the full
    /// length, less the trailing `**` when there is one.
    #[inline]
    fn fixed(&self) -> usize {
        if self.has_multi() {
            self.segments.len() - 1
        } else {
            self.segments.len()
        }
    }

    /// Matches raw, still-encoded path segments against this route.
    ///
    /// `segments` must be split on `/` *before* any percent-decoding, per
    /// README §1.2: decoding first would let a `%2F` invent a segment
    /// boundary and corrupt an AIP-122 resource name.
    ///
    /// A failed match costs no allocation, because captures are sliced out
    /// separately by [`Route::capture`] only once a route has won.
    pub fn matches(&self, segments: &[&str], verb: &str) -> bool {
        if verb != self.verb {
            return false;
        }

        let fixed = self.fixed();
        if self.has_multi() {
            // `**` matches zero or more, so the path may be shorter than the
            // route is long, but never shorter than the fixed prefix.
            if segments.len() < fixed {
                return false;
            }
        } else if segments.len() != fixed {
            return false;
        }

        for (i, m) in self.segments[..fixed].iter().enumerate() {
            match m {
                Match::Literal(lit) => {
                    if segments[i] != *lit {
                        return false;
                    }
                }
                Match::Single => {
                    // A `*` binds exactly one component, and an empty component
                    // — from a doubled or trailing slash — is not one.
                    if segments[i].is_empty() {
                        return false;
                    }
                }
                Match::Multi => unreachable!("`**` cannot appear in the fixed prefix"),
            }
        }
        true
    }

    /// Slices one capture out of a matched path and decodes it.
    ///
    /// Decoding happens here, after the match, per README §1.2 step 4:
    /// every percent-escape is decoded **except `%2F`**, which is left as
    /// written because `/` separates the segments of an AIP-122 resource name.
    /// Decoding it would make `shelves/a%2Fb` indistinguishable from the
    /// genuinely three-segment `shelves/a/b`.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureError`] naming the field when a segment's encoding is
    /// truncated, is not hex, or decodes to invalid UTF-8. Each is a `400`, not
    /// a `404`: the path matched, the value is what is wrong.
    pub fn capture<'a>(
        &self,
        capture: &Capture,
        segments: &[&'a str],
    ) -> Result<Cow<'a, str>, CaptureError> {
        let span = &segments[capture.start..capture.end_index(segments.len())];
        let map_err = |kind| CaptureError {
            field: capture.json,
            kind,
        };

        match span {
            // A `**` binding zero segments yields an empty value, not an error.
            [] => Ok(Cow::Borrowed("")),
            [one] => decode_segment(one).map_err(map_err),
            many => {
                let mut out = String::new();
                for (i, seg) in many.iter().enumerate() {
                    if i > 0 {
                        out.push('/');
                    }
                    out.push_str(&decode_segment(seg).map_err(map_err)?);
                }
                Ok(Cow::Owned(out))
            }
        }
    }
}
