//! Path segmentation and AIP-136 custom-verb separation.

/// Splits a request path into raw segments and peels a custom verb.
///
/// Splitting happens on the undecoded path, per README §1.2 step 2, so a
/// `%2F` cannot create a segment boundary.
///
/// The verb is only separated when `has_verb_routes` says some registered route
/// declares one. A `:` is legal inside a resource id, so stripping a suffix
/// nobody asked for would bind the id to the wrong value — which is exactly the
/// failure mode of feeding `/v1/{name}:cancel` to a general-purpose router,
/// which accepts it and silently folds `:cancel` into `name`.
///
/// Returns the raw segments and the verb without its colon, or `""`.
pub fn split_path(path: &str, has_verb_routes: bool) -> (Vec<&str>, &str) {
    let path = path.strip_prefix('/').unwrap_or(path);
    let mut segments: Vec<&str> = path.split('/').collect();
    let mut verb = "";

    if has_verb_routes
        && let Some(last) = segments.last_mut()
        && let Some(idx) = last.rfind(':')
    {
        let (head, tail) = last.split_at(idx);
        // Both halves must be non-empty: ":x" has no resource id and "x:" has
        // no verb, and neither is a custom method.
        if !head.is_empty() && tail.len() > 1 {
            verb = &tail[1..];
            *last = head;
        }
    }
    (segments, verb)
}
