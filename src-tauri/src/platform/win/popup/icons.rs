//! File: icons.rs
//! Purpose: Menu icons as 8-bit alpha masks (Lucide, rendered at build time), one per size, so any display scale gets a
//! crisp 1:1 bitmap that is tinted at draw time to match the light or dark menu.
//! Contents: mask — the mask and its pixel size nearest to the wanted size.

/// Pixel sizes the masks were rendered at: 15 dip at 100 %, 125 %, 150 %, 200 %, 250 %.
const SIZES: [u32; 5] = [15, 19, 23, 30, 38];

macro_rules! masks {
    ($name:literal) => {
        (
            $name,
            [
                include_bytes!(concat!("../../../../menu-icons/", $name, "-15.a8")).as_slice(),
                include_bytes!(concat!("../../../../menu-icons/", $name, "-19.a8")).as_slice(),
                include_bytes!(concat!("../../../../menu-icons/", $name, "-23.a8")).as_slice(),
                include_bytes!(concat!("../../../../menu-icons/", $name, "-30.a8")).as_slice(),
                include_bytes!(concat!("../../../../menu-icons/", $name, "-38.a8")).as_slice(),
            ],
        )
    };
}

const MASKS: [(&str, [&[u8]; 5]); 2] = [masks!("settings"), masks!("power")];

/// The alpha mask of icon `name` at the bundled size nearest `wanted_px`. Return: the mask bytes and its side in px, None for an unknown name.
pub fn mask(name: &str, wanted_px: f32) -> Option<(&'static [u8], u32)> {
    let i = (0..SIZES.len()).min_by_key(|&i| (SIZES[i] as f32 - wanted_px).abs() as u32)?;
    Some((MASKS.iter().find(|(n, _)| *n == name)?.1[i], SIZES[i]))
}
