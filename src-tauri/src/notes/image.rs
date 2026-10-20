//! Purpose: Text from a picture dropped on a note, read with the recognition built into Windows (the same engine as Grab Text).
//! Contents: read — checks the dropped path is a picture file of a sensible size, then reads its text; the check is the boundary, since the path comes from the page.

use std::path::Path;

/// Picture kinds Windows can decode that a person drops: the page filters the same list for its drop hint.
const KINDS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp"];
const MAX_BYTES: u64 = 30 * 1024 * 1024;

pub fn read(path: &str) -> Result<String, String> {
    let path = Path::new(path);
    let is_picture = path.extension().and_then(|e| e.to_str()).is_some_and(|e| KINDS.contains(&e.to_ascii_lowercase().as_str()));
    if !is_picture {
        return Err("Drop a picture (png, jpg, bmp, gif, tiff or webp).".into());
    }
    let size = std::fs::metadata(path).map_err(|e| format!("Could not open the image: {e}"))?.len();
    if size > MAX_BYTES {
        return Err("That picture is too large to read.".into());
    }
    let text = crate::platform::recognize_image_file(path)?;
    if text.trim().is_empty() {
        return Err("No text found in that picture.".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn only_picture_files_are_read() {
        assert!(read("C:\\notes.txt").unwrap_err().contains("Drop a picture"));
        assert!(read("no-such-folder/shot.PNG").unwrap_err().contains("Could not open"));
    }
}
