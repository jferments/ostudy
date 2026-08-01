fn duplicate_checkable(front: &str, back: &str) -> bool {
    !front.trim().is_empty() && !back.trim().is_empty()
}

fn normalized_pair(front: &str, back: &str) -> (String, String) {
    (
        collapse_whitespace(front).to_lowercase(),
        collapse_whitespace(back).to_lowercase(),
    )
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn duplicate_hash(front: &str, back: &str) -> String {
    // Stable FNV-1a. Hash collisions are harmless because every candidate is
    // compared against the normalized full text before it is treated as a
    // duplicate.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in front
        .as_bytes()
        .iter()
        .copied()
        .chain(std::iter::once(0xff))
        .chain(back.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn split_tags(value: &str) -> Vec<String> {
    value
        .split(TAG_SEPARATOR)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn default_due_filter() -> String {
    "all".to_string()
}

fn default_sort() -> String {
    "updated_desc".to_string()
}

const fn default_limit() -> i64 {
    100
}

const fn default_true() -> bool {
    true
}
