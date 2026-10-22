//! The app icon, drawn in code at any size: a blue disc with a white clock
//! (hands at ten past ten). Dependency-free, so `xtask` can include this file
//! to write `packaging/icon.ico`.

/// RGBA pixels, `size` × `size`, anti-aliased.
pub fn app_icon_rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let c = s / 2.0;
    let radius = s * 0.47;
    let stroke = (s * 0.075).max(1.2);
    let hour = (c - s * 0.20, c - s * 0.12); // up-left, shorter
    let minute = (c + s * 0.17, c - s * 0.21); // up-right, longer

    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let disc = coverage(radius - ((px - c).powi(2) + (py - c).powi(2)).sqrt());
            let hand = coverage(stroke / 2.0 - segment_distance((px, py), (c, c), hour)).max(
                coverage(stroke / 2.0 - segment_distance((px, py), (c, c), minute)),
            );
            // Blue #1A73E8 under white hands.
            let mix = |blue: f32| blue + (255.0 - blue) * hand;
            rgba.extend_from_slice(&[
                mix(26.0) as u8,
                mix(115.0) as u8,
                mix(232.0) as u8,
                (disc * 255.0) as u8,
            ]);
        }
    }
    rgba
}

/// 0..=1 from a signed distance (positive inside), one pixel of smoothing.
fn coverage(signed_distance: f32) -> f32 {
    (signed_distance + 0.5).clamp(0.0, 1.0)
}

fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / (abx * abx + aby * aby)).clamp(0.0, 1.0);
    let (qx, qy) = (a.0 + abx * t, a.1 + aby * t);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}
