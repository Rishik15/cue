//! File: snippets/expand/values.rs
//! Purpose: The small value makers behind placeholders: text modifiers, a random UUID, a random choice or number. Pure: randomness comes in as a function.
//! Contents: modify — one `| modifier` applied to text; uuid — a version 4 id; random — a pick from a list or a range (both None when the entropy fails).

/// Applies one `| modifier` to `text`; None when the modifier is unknown.
pub fn modify(text: &str, modifier: &str) -> Option<String> {
    Some(match modifier.to_lowercase().as_str() {
        "upper" | "uppercase" => text.to_uppercase(),
        "lower" | "lowercase" => text.to_lowercase(),
        "trim" => text.trim().to_string(),
        "title" => text.split(' ').map(capital).collect::<Vec<_>>().join(" "),
        _ => return None,
    })
}

/// A word with its first letter upper and the rest lower case.
fn capital(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
}

/// A random version 4 UUID from `entropy`; None when it fails, so no id is invented.
pub fn uuid(entropy: &dyn Fn() -> Option<u64>) -> Option<String> {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&entropy()?.to_le_bytes());
    b[8..].copy_from_slice(&entropy()?.to_le_bytes());
    b[6] = b[6] & 0x0f | 0x40;
    b[8] = b[8] & 0x3f | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Some(format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]))
}

/// One of the comma-separated choices (`yes, no, maybe`), or a whole number in a range (`1-6`), picked with `entropy`; None when the spec is not one of these or `entropy` fails.
pub fn random(spec: &str, entropy: &dyn Fn() -> Option<u64>) -> Option<String> {
    let pick = |count: u64| entropy().map(|n| n % count);
    if let Some((lo, hi)) = spec.split_once('-').and_then(|(a, b)| Some((a.trim().parse::<i64>().ok()?, b.trim().parse::<i64>().ok()?))) {
        let count = hi.checked_sub(lo)?.checked_add(1).filter(|c| *c > 0)?;
        return Some((lo + pick(count as u64)? as i64).to_string());
    }
    let choices: Vec<&str> = spec.split(',').map(str::trim).filter(|c| !c.is_empty()).collect();
    if choices.is_empty() {
        return None;
    }
    Some(choices[pick(choices.len() as u64)? as usize].to_string())
}
