//! The codec table.

use super::Framing;

/// One codec's static metadata, as the generator emits it.
///
/// The registry stores metadata rather than trait objects because negotiation
/// only ever needs the metadata. The concrete codec is reached by the generated
/// handler matching on [`CodecEntry::index`], which keeps encoding
/// monomorphic — no `dyn`, no downcast, nothing to dispatch on the hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecEntry {
    /// The `?alt=` selector, e.g. `"json"`.
    pub name: &'static str,

    /// The media types this codec answers to, most canonical first. The first
    /// entry becomes the response `Content-Type`.
    pub media_types: &'static [&'static str],

    /// How this codec delimits a server-streaming response.
    pub framing: Framing,

    /// Position in the registry's slice, and the discriminant the generated
    /// handler switches on.
    pub index: usize,
}

impl CodecEntry {
    /// The `Content-Type` a response encoded by this codec carries.
    pub fn content_type(&self) -> &'static str {
        self.media_types
            .first()
            .copied()
            .unwrap_or("application/octet-stream")
    }

    /// Whether this codec answers to the given concrete media type.
    fn answers_to(&self, media_type: &str) -> bool {
        self.media_types
            .iter()
            .any(|m| m.eq_ignore_ascii_case(media_type))
    }
}

/// The set of codecs a gateway was generated with.
///
/// Ordering is significant: the first entry is the default, used when a request
/// expresses no preference at all.
#[derive(Debug, Clone, Copy)]
pub struct CodecRegistry {
    /// The codecs, default first.
    entries: &'static [CodecEntry],
}

impl CodecRegistry {
    /// Builds a registry over a generated codec slice.
    ///
    /// # Panics
    ///
    /// Panics if `entries` is empty. A gateway with no codec cannot answer any
    /// request, and failing at construction is far better than failing on the
    /// first one.
    #[must_use]
    pub const fn new(entries: &'static [CodecEntry]) -> Self {
        assert!(
            !entries.is_empty(),
            "a codec registry needs at least one codec"
        );
        Self { entries }
    }

    /// The default codec, used when a request expresses no preference.
    pub const fn default_codec(&self) -> &'static CodecEntry {
        &self.entries[0]
    }

    /// Every registered codec, in declaration order.
    pub const fn entries(&self) -> &'static [CodecEntry] {
        self.entries
    }

    /// Looks a codec up by its `?alt=` name.
    pub fn by_name(&self, name: &str) -> Option<&'static CodecEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Looks a codec up by a concrete media type, ignoring parameters.
    pub fn by_media_type(&self, media_type: &str) -> Option<&'static CodecEntry> {
        let bare = media_type.split(';').next()?.trim();
        self.entries.iter().find(|e| e.answers_to(bare))
    }

    /// The registered `?alt=` names, for an error message listing what is
    /// supported.
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|e| e.name).collect()
    }

    /// The canonical media type of every registered codec, for the same
    /// purpose.
    pub fn supported_media_types(&self) -> Vec<&'static str> {
        self.entries.iter().map(CodecEntry::content_type).collect()
    }
}
