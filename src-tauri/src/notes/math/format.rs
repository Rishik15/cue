//! Purpose: Shows a calculated number the way a note displays it: a fixed number of decimals and optional thousands commas.
//! Contents: show — number to text.

/// `5149100` with 2 decimals and commas is "5,149,100.00". A value that rounds to nothing never shows a minus sign.
pub fn show(value: f64, digits: usize, thousands: bool) -> String {
    let text = format!("{:.digits$}", value.abs());
    let (whole, fraction) = text.split_once('.').map_or((text.as_str(), None), |(w, f)| (w, Some(f)));
    let mut out = String::new();
    if value < 0.0 && text.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.push('-');
    }
    for (i, c) in whole.chars().enumerate() {
        if thousands && i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(f) = fraction {
        out.push('.');
        out.push_str(f);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::show;

    #[test]
    fn groups_rounds_and_signs() {
        assert_eq!(show(5_149_100.0, 2, true), "5,149,100.00");
        assert_eq!(show(5_149_100.0, 0, false), "5149100");
        assert_eq!(show(-1234.5, 1, true), "-1,234.5");
        assert_eq!(show(999.0, 2, true), "999.00");
        assert_eq!(show(-0.001, 2, true), "0.00");
        assert_eq!(show(0.5, 7, true), "0.5000000");
    }
}
