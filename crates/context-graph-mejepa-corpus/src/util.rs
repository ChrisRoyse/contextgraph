/// Return the byte offset for appending generated text while preserving one
/// canonical trailing newline.
pub(crate) fn append_insertion_offset(source: &str) -> usize {
    if source.ends_with('\n') {
        source.len() - 1
    } else {
        source.len()
    }
}
