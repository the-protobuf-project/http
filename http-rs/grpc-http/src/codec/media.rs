//! Media type parsing and matching.

/// A parsed media type: a type, a subtype, and the parameters ignored for
/// matching.
///
/// Borrowed from the header it was parsed from, so negotiating costs no
/// allocation on the request path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaType<'a> {
    /// The top-level type, e.g. `application`. `*` for a wildcard.
    pub type_: &'a str,
    /// The subtype, e.g. `json`. `*` for a wildcard.
    pub subtype: &'a str,
}

impl<'a> MediaType<'a> {
    /// Parses one media type, discarding its parameters.
    ///
    /// Parameters other than `charset` carry no meaning for codec selection,
    /// per README §3, and `charset` is only ever UTF-8 here: protojson
    /// is defined as UTF-8 and protobuf is binary.
    ///
    /// Returns `None` when the input is not `type/subtype`.
    pub fn parse(raw: &'a str) -> Option<Self> {
        let value = raw.split(';').next()?.trim();
        let (type_, subtype) = value.split_once('/')?;

        let (type_, subtype) = (type_.trim(), subtype.trim());
        if type_.is_empty() || subtype.is_empty() {
            return None;
        }
        Some(MediaType { type_, subtype })
    }

    /// Whether this media type matches `concrete`, honouring wildcards on
    /// *this* side only.
    ///
    /// The asymmetry is intentional: an `Accept` entry may be `*/*`, but a
    /// codec's registered type never is, so wildcards belong to the request.
    pub fn matches(&self, concrete: &str) -> bool {
        let Some(other) = MediaType::parse(concrete) else {
            return false;
        };
        let type_ok = self.type_ == "*" || self.type_.eq_ignore_ascii_case(other.type_);
        let subtype_ok = self.subtype == "*" || self.subtype.eq_ignore_ascii_case(other.subtype);
        type_ok && subtype_ok
    }

    /// How specific the media type is, for RFC 9110 precedence:
    /// `*/*` is 0, `type/*` is 1, `type/subtype` is 2.
    ///
    /// A more specific `Accept` entry outranks a less specific one at the same
    /// quality, so `Accept: */*, application/json` prefers JSON.
    pub const fn specificity(&self) -> u8 {
        match (self.type_.as_bytes(), self.subtype.as_bytes()) {
            (b"*", _) => 0,
            (_, b"*") => 1,
            _ => 2,
        }
    }

    /// Whether this is the `*/*` wildcard, which accepts anything.
    pub fn is_any(&self) -> bool {
        self.type_ == "*" && self.subtype == "*"
    }
}

impl std::fmt::Display for MediaType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.type_, self.subtype)
    }
}
