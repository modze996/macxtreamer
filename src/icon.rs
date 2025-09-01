use eframe::egui::viewport::IconData;

// Generate a simple play-button app icon (blue circular gradient + white triangle)
pub fn generate_icon(size: u32) -> IconData {
    let w = size;
    let h = size;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let cx = (w as f32) * 0.5;
    let cy = (h as f32) * 0.5;
    let radius = (w.min(h) as f32) * 0.45;

    // Play triangle points (slightly to the left to center visually)
    let p1 = (w as f32 * 0.40, h as f32 * 0.32);
    let p2 = (w as f32 * 0.40, h as f32 * 0.68);
    let p3 = (w as f32 * 0.70, h as f32 * 0.50);

    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;

            // Background: circular gradient
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let in_circle = dist <= radius;
            // gradient t from center (0) to edge (1)
            let t = (dist / radius).min(1.0);
            // Blue gradient: inner brighter, outer darker
            let r = lerp(28.0, 10.0, t);
            let g = lerp(140.0, 60.0, t);
            let b = lerp(240.0, 120.0, t);

            rgba[idx + 0] = if in_circle { r as u8 } else { 0 };
            rgba[idx + 1] = if in_circle { g as u8 } else { 0 };
            rgba[idx + 2] = if in_circle { b as u8 } else { 0 };
            rgba[idx + 3] = if in_circle { 255 } else { 0 };

            // Overlay: white play triangle (barycentric point-in-triangle)
            if point_in_triangle(x as f32 + 0.5, y as f32 + 0.5, p1, p2, p3) {
                rgba[idx + 0] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    }

    IconData { rgba, width: w, height: h }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

#[inline]
fn cross(ax: f32, ay: f32, bx: f32, by: f32) -> f32 { ax * by - ay * bx }

fn point_in_triangle(px: f32, py: f32, p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> bool {
    let (x1, y1) = p1;
    let (x2, y2) = p2;
    let (x3, y3) = p3;
    let c1 = cross(x2 - x1, y2 - y1, px - x1, py - y1);
    let c2 = cross(x3 - x2, y3 - y2, px - x2, py - y2);
    let c3 = cross(x1 - x3, y1 - y3, px - x3, py - y3);
    let has_neg = (c1 < 0.0) || (c2 < 0.0) || (c3 < 0.0);
    let has_pos = (c1 > 0.0) || (c2 > 0.0) || (c3 > 0.0);
    !(has_neg && has_pos)
}
