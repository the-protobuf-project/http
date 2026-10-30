//! The query parameters, after the system parameters are removed.

use std::collections::HashMap;

/// The `?alt=` system parameter, which selects the response codec.
pub const PARAM_ALT: &str = "alt";

/// The AIP-157 `?fields=` partial-response mask.
pub const PARAM_FIELDS: &str = "fields";

/// The `?prettyPrint=` system parameter, which asks for indented output.
pub const PARAM_PRETTY_PRINT: &str = "prettyPrint";

/// The parameters stripped before binding and never bound to a field.
///
/// Each has a `$`-prefixed alias, for clients that cannot send a bare `alt` —
/// the prefix is reserved precisely so a future system parameter cannot collide
/// with a field name.
const SYSTEM_PARAMS: &[&str] = &[PARAM_ALT, PARAM_FIELDS, PARAM_PRETTY_PRINT];

/// The request's query parameters with the system parameters removed.
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// The remaining parameters. A repeated field is sent as one occurrence per
    /// element, so each name maps to every value it was sent with.
    pub values: HashMap<String, Vec<String>>,
    /// The `?alt=` codec selector.
    pub alt: Option<String>,
    /// The AIP-157 `?fields=` mask.
    pub fields: Option<String>,
    /// Whether `?prettyPrint=` asked for indented output.
    pub pretty_print: bool,
}

impl Query {
    /// The first value of a parameter.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name)?.first().map(String::as_str)
    }

    /// Every value of a parameter, in the order sent.
    #[must_use]
    pub fn all(&self, name: &str) -> &[String] {
        self.values.get(name).map_or(&[], Vec::as_slice)
    }

    /// Whether a parameter was sent at all, which is distinct from its being
    /// sent empty.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// The parameters not in `known`, sorted so the error naming them is
    /// deterministic.
    #[must_use]
    pub fn unknown(&self, known: &[&str]) -> Vec<&str> {
        let mut unknown: Vec<&str> = self
            .values
            .keys()
            .map(String::as_str)
            .filter(|name| !known.contains(name))
            .collect();
        unknown.sort_unstable();
        unknown
    }

    /// Splits a raw query string into system parameters and the rest.
    ///
    /// A pair with a malformed escape is skipped rather than failing the whole
    /// query: the well-formed pairs are still the caller's request, and
    /// rejecting everything over one bad byte would be a worse answer than
    /// binding what was understood. This is what Go's `url.ParseQuery` does.
    ///
    /// # Errors
    ///
    /// The name of a `$`-prefixed parameter that is not a system parameter.
    /// Accepting it would let a future system parameter silently change what an
    /// existing request means.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut query = Query::default();
        for pair in raw.split('&').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let (Some(name), Some(value)) = (unescape(key), unescape(value)) else {
                continue;
            };

            let bare = name.strip_prefix('$').unwrap_or(&name);
            if !SYSTEM_PARAMS.contains(&bare) {
                if name.starts_with('$') {
                    return Err(name);
                }
                query.values.entry(name).or_default().push(value);
                continue;
            }

            // The first occurrence wins, as it does for `url.Values.Get`.
            match bare {
                PARAM_ALT => {
                    query.alt.get_or_insert(value);
                }
                PARAM_FIELDS => {
                    query.fields.get_or_insert(value);
                }
                _ => query.pretty_print = !matches!(value.as_str(), "" | "false" | "0"),
            }
        }
        Ok(query)
    }
}

/// Decodes one query component: `+` is a space, and `%XX` is a byte.
///
/// `None` for a malformed escape or a result that is not UTF-8.
fn unescape(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => {
                let hi = hex(*bytes.get(i + 1)?)?;
                let lo = hex(*bytes.get(i + 2)?)?;
                out.push(hi << 4 | lo);
                i += 2;
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}

const fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
