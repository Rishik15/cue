//! Purpose: The small value makers behind placeholders: text modifiers, a random UUID, a random choice or number.
//! Contents: modify — one `| modifier` applied to text; uuid — a version 4 id; random — a pick from a list or a range.

pub fn modify(text: &str, modifier: &str) -> Option<String> {
    Some(match modifier.to_lowercase().as_str() {
        "upper" | "uppercase" => text.to_uppercase(),
        "lower" | "lowercase" => text.to_lowercase(),
        "trim" => text.trim().to_string(),
        "title" => text.split(' ').map(capital).collect::<Vec<_>>().join(" "),
        _ => return None,
    })
}

fn capital(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
}

/// A random version 4 UUID.
pub fn uuid() -> String {
    let mut b = [0u8; 16];
    // A failing system generator gives zeros, which still make a well-formed (if useless) id; it does not happen in practice.
    let _ = getrandom::fill(&mut b);
    b[6] = b[6] & 0x0f | 0x40;
    b[8] = b[8] & 0x3f | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// One of the comma-separated choices (`yes, no, maybe`), or a whole number in a range (`1-6`).
pub fn random(spec: &str) -> Option<String> {
    let pick = |count: u64| getrandom::u64().ok().map(|n| n % count);
    if let Some((lo, hi)) = spec.split_once('-').and_then(|(a, b)| Some((a.trim().parse::<i64>().ok()?, b.trim().parse::<i64>().ok()?))) {
        let count = hi.checked_sub(lo)?.checked_add(1).filter(|c| *c > 0)?;
        return Some((lo + pick(count as u64)? as i64).to_string());
    }
    let choices: Vec<&str> = spec.split(',').map(str::trim).filter(|c| !c.is_empty()).collect();
    (!choices.is_empty()).then(|| choices[pick(choices.len() as u64).unwrap_or(0) as usize].to_string())
}
