//! `Accept` header parsing and preference ordering.

use super::media::MediaType;

/// One entry of an `Accept` header: a media range and its quality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcceptEntry<'a> {
    /// The media range, which may contain wildcards.
    pub media: MediaType<'a>,

    /// The quality value, scaled to thousandths so entries sort as integers.
    ///
    /// RFC 9110 allows at most three decimal places, so this is exact rather
    /// than a rounding of the float the header actually spells.
    pub quality: u16,
}

impl AcceptEntry<'_> {
    /// Whether the client has explicitly refused this range.
    ///
    /// `q=0` is a refusal, not a low preference: a codec matched only by a
    /// zero-quality entry must not be selected.
    pub const fn is_refusal(&self) -> bool {
        self.quality == 0
    }
}

/// Parses an `Accept` header into entries ordered by preference, most preferred
/// first.
///
/// Ordering follows RFC 9110: quality descending, then specificity descending,
/// so `Accept: */*, application/json` prefers JSON even though the wildcard
/// came first. Ties keep header order, which is the only tiebreak a client can
/// actually control.
///
/// Unparseable entries are skipped rather than failing the request — a header
/// with one malformed range and one good one should still work.
pub fn parse_accept(header: &str) -> Vec<AcceptEntry<'_>> {
    let mut entries: Vec<AcceptEntry<'_>> = header
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            let media = MediaType::parse(part)?;
            Some(AcceptEntry {
                media,
                quality: parse_quality(part),
            })
        })
        .collect();

    // sort_by is stable, so equal entries keep the order the client sent.
    entries.sort_by(|a, b| {
        b.quality
            .cmp(&a.quality)
            .then_with(|| b.media.specificity().cmp(&a.media.specificity()))
    });
    entries
}

/// Extracts the `q=` parameter from one `Accept` entry, defaulting to 1.0.
///
/// Returns thousandths: `q=0.9` is 900, an absent parameter is 1000.
fn parse_quality(entry: &str) -> u16 {
    for param in entry.split(';').skip(1) {
        let Some((key, value)) = param.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("q") {
            continue;
        }
        return parse_q_value(value.trim());
    }
    1000
}

/// Parses a quality value into thousandths.
///
/// Done by hand rather than through `f32` so `q=0.001` and `q=1.0` are exact,
/// and so a malformed value degrades to "fully acceptable" rather than to a
/// silent refusal — the safer direction when the header is ambiguous.
fn parse_q_value(raw: &str) -> u16 {
    let (whole, frac) = raw.split_once('.').unwrap_or((raw, ""));

    let base = match whole.trim() {
        "0" => 0u16,
        "1" => return 1000, // any fraction after "1" cannot raise it further
        _ => return 1000,
    };

    // At most three digits are significant; anything beyond is truncated.
    let mut thousandths = 0u16;
    for (i, c) in frac.chars().take(3).enumerate() {
        let Some(digit) = c.to_digit(10) else {
            return 1000;
        };
        let scale = match i {
            0 => 100,
            1 => 10,
            _ => 1,
        };
        thousandths += digit as u16 * scale;
    }
    base + thousandths
}
