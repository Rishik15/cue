//! Purpose: Migration 3 helper: folder prefixes used to be applied while typing; this writes them into the stored triggers before the column is dropped.
//! Contents: fold — a snippet's triggers text with a prefix put in front of each non-pattern line; fold_into_triggers — applies it to every snippet in a prefixed folder.

use rusqlite::{params, Connection};

use super::expand::pattern::PREFIX;

/// Puts `prefix` before every trigger line except patterns (`re:`) and empty lines.
/// A result over the trigger length limit is kept as is: truncating would change what the user types.
pub fn fold(prefix: &str, triggers: &str) -> String {
    let lines: Vec<String> = triggers
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| if l.starts_with(PREFIX) { l.to_string() } else { format!("{prefix}{l}") })
        .collect();
    lines.join("\n")
}

/// For every snippet whose folder has a prefix, rewrites its triggers with the prefix included.
pub fn fold_into_triggers(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt =
        conn.prepare("SELECT s.id, f.prefix, s.triggers FROM snippets s JOIN folders f ON f.id = s.folder_id WHERE f.prefix != ''")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, prefix, triggers) in rows {
        conn.execute("UPDATE snippets SET triggers = ?1 WHERE id = ?2", params![fold(&prefix, &triggers), id])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn the_prefix_goes_before_plain_triggers_only() {
        assert_eq!(fold(";", "sig\nre:\\d+\n\nhi"), ";sig\nre:\\d+\n;hi");
    }
}
