//! File: rows.rs
//! Purpose: What a screen reader is told about the open tray menu, as plain data: one row per selectable item with its name, its place among the items (position in set, size of set) and its rectangle on the screen. Built from the menu's layout only when a client first asks, so an idle menu pays nothing.
//! Contents: Row — one selectable item; rows — the rows of a menu from its entries, layout and screen origin; row_at — the row under a screen point.

use super::draw::Layout;
use crate::platform::MenuEntry;

/// One selectable item as assistive technology sees it. `entry` is its index among all entries (separators included), the index the menu's hover and choice use. Rectangle fields are screen pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub entry: usize,
    pub name: String,
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// The rows of a menu. Input: the entries, their layout and the screen position of the menu card's top-left corner. Return: one row per item, in order; separators have none.
pub fn rows(entries: &[MenuEntry], layout: &Layout, (x, y): (i32, i32)) -> Vec<Row> {
    entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| match entry {
            MenuEntry::Item { label, .. } => Some(Row {
                entry: i,
                name: label.clone(),
                left: x as f64,
                top: (y + layout.tops[i]) as f64,
                width: layout.width as f64,
                height: layout.heights[i] as f64,
            }),
            MenuEntry::Separator => None,
        })
        .collect()
}

/// The position in `rows` of the row containing the screen point, if any. Input: the rows and the point.
pub fn row_at(rows: &[Row], x: f64, y: f64) -> Option<usize> {
    rows.iter().position(|r| x >= r.left && x < r.left + r.width && y >= r.top && y < r.top + r.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two items split by a separator, 100 px wide.
    fn sample() -> (Vec<MenuEntry>, Layout) {
        let item = |id, label: &str| MenuEntry::Item { id, label: label.into(), icon: None };
        let entries = vec![item(1, "Settings"), MenuEntry::Separator, item(2, "Quit")];
        (entries, Layout { scale: 1.0, width: 100, height: 80, tops: vec![4, 37, 42], heights: vec![33, 5, 33] })
    }

    /// Separators get no row, and each item keeps its entry index, name and screen rectangle.
    #[test]
    fn one_row_per_item_with_screen_rectangle() {
        let (entries, layout) = sample();
        let rows = rows(&entries, &layout, (1000, 500));
        assert_eq!(rows.iter().map(|r| (r.entry, r.name.as_str())).collect::<Vec<_>>(), vec![(0, "Settings"), (2, "Quit")]);
        assert_eq!((rows[1].left, rows[1].top, rows[1].width, rows[1].height), (1000.0, 542.0, 100.0, 33.0));
    }

    /// A point finds the row it is inside and nothing in a gap or outside.
    #[test]
    fn point_finds_its_row() {
        let (entries, layout) = sample();
        let rows = rows(&entries, &layout, (0, 0));
        assert_eq!(row_at(&rows, 50.0, 10.0), Some(0));
        assert_eq!(row_at(&rows, 50.0, 50.0), Some(1));
        assert_eq!(row_at(&rows, 50.0, 38.0), None);
        assert_eq!(row_at(&rows, 150.0, 10.0), None);
    }
}
