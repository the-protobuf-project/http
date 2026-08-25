//! The two value types a compiled template is made of.

/// One element of a compiled template.
///
/// There is no `Variable` variant: the compiler expands variables into their
/// sub-segments and records the spans in [`Route::captures`], which is what
/// turns matching into a flat positional walk.
///
/// [`Route::captures`]: super::Route::captures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match {
    /// A fixed path component, compared byte-exact against the still-encoded
    /// request segment.
    Literal(&'static str),
    /// `*` — exactly one non-empty component.
    Single,
    /// `**` — zero or more components. Only ever the final element of a route.
    Multi,
}

impl Match {
    /// Orders segment kinds from most to least specific: a literal outranks a
    /// `*`, which outranks a `**`.
    ///
    /// This mirrors `httprule.Kind.rank` in the generator, which is where
    /// precedence is actually applied — a route table arrives already sorted.
    /// It exists here for assertions and diagnostics, not for the matching
    /// path.
    pub const fn rank(&self) -> u8 {
        match self {
            Match::Literal(_) => 0,
            Match::Single => 1,
            Match::Multi => 2,
        }
    }

    /// Whether this segment matches any single component regardless of content.
    pub const fn is_wildcard(&self) -> bool {
        matches!(self, Match::Single | Match::Multi)
    }
}

/// Marks a [`Capture`] span that runs to the end of the path.
///
/// Spans are stored as `i32` rather than `Option<usize>` so the generated table
/// is a plain array of integers in every target language.
pub const TO_END: i32 = -1;

/// Where one template variable's value lives in a matched path.
///
/// Indices count from the start of the path, which is well defined precisely
/// because `**` may only appear last: every span except one ending in `**` sits
/// at a fixed position no matter how long the request path turns out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// The request-message field path this span binds, in proto field names.
    ///
    /// `{book.name=*}` yields `["book", "name"]`. The generator emits typed
    /// setters against this, so the runtime never resolves it.
    pub field: &'static [&'static str],

    /// The protojson spelling of [`Capture::field`], e.g. `"book.displayName"`.
    ///
    /// This is the name a `BadRequest.FieldViolation` reports and the name
    /// `OpenAPI` documents, so a caller sees one spelling everywhere.
    pub json: &'static str,

    /// First segment index of the span, inclusive.
    pub start: usize,

    /// One past the last segment index, or [`TO_END`] when the span ends in a
    /// `**` and so extends to the path's final segment.
    pub end: i32,
}

impl Capture {
    /// Resolves the span's exclusive end against a concrete path length.
    pub const fn end_index(&self, path_len: usize) -> usize {
        if self.end == TO_END {
            path_len
        } else {
            self.end as usize
        }
    }
}
