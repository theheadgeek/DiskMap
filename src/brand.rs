//! The Inactive Node mark and copyright line.

use eframe::egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2};
use std::f32::consts::TAU;
use std::time::{SystemTime, UNIX_EPOCH};

/// "Copyright © <this year>, Inactive Node", as the design system's footer reads.
pub fn copyright() -> String {
    format!("Copyright © {}, Inactive Node", current_year())
}

fn current_year() -> i64 {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    year_from_days((seconds / 86_400) as i64)
}

/// The calendar year for a count of days since 1970-01-01
/// (Howard Hinnant's civil-from-days algorithm, trimmed to the year).
fn year_from_days(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153; // 0 = March … 11 = February
    year_of_era + era * 400 + i64::from(month_index >= 10)
}

/// Paints the Inactive Node cube into a square `rect`. This follows the
/// corrected SVG in Nodal's BrandCube.svelte (a 64×64 viewBox), back to front:
/// faces, the hidden far edges, the glowing emblem, then the near edges on top.
pub fn paint_cube(painter: &Painter, rect: Rect) {
    let scale = rect.width() / 64.0;
    let p = |x: f32, y: f32| pos2(rect.left() + x * scale, rect.top() + y * scale);
    let hex = |rgb: u32, alpha: f32| {
        let [_, r, g, b] = rgb.to_be_bytes();
        Color32::from_rgba_unmultiplied(r, g, b, (alpha * 255.0).round() as u8)
    };
    let lines = |points: &[[(f32, f32); 2]], width: f32, color: Color32| {
        for [a, b] in points {
            painter.line_segment([p(a.0, a.1), p(b.0, b.1)], Stroke::new(width * scale, color));
        }
    };

    // Faces: top, left, right.
    for (points, color) in [
        ([(32.0, 3.0), (58.0, 18.0), (32.0, 33.0), (6.0, 18.0)], 0x17131F),
        ([(6.0, 18.0), (32.0, 33.0), (32.0, 61.0), (6.0, 46.0)], 0x0E0B14),
        ([(58.0, 18.0), (58.0, 46.0), (32.0, 61.0), (32.0, 33.0)], 0x0A0810),
    ] {
        let points = points.iter().map(|&(x, y)| p(x, y)).collect();
        painter.add(Shape::convex_polygon(points, hex(color, 1.0), Stroke::NONE));
    }

    // The hidden far corner, dimmer so it reads as inside the box.
    let center = (32.0, 33.0);
    lines(
        &[[center, (32.0, 3.0)], [center, (6.0, 46.0)], [center, (58.0, 46.0)]],
        1.1,
        hex(0x2F1D2B, 0.55),
    );

    // Emblem: a red glow, the ring and the ✕.
    painter.add(glow(p(32.0, 33.0), 16.5 * scale, hex(0xF0343D, 0.62), hex(0xB4141E, 0.22)));
    painter.circle_stroke(p(32.0, 33.0), 14.5 * scale, Stroke::new(2.3 * scale, hex(0xD81F29, 0.7)));
    let cross = hex(0xEE3A42, 1.0);
    for (a, b) in [((24.0, 24.5), (40.0, 40.5)), ((40.0, 24.5), (24.0, 40.5))] {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], Stroke::new(4.3 * scale, cross));
        // Round caps, as the SVG's stroke-linecap="round".
        painter.circle_filled(p(a.0, a.1), 2.15 * scale, cross);
        painter.circle_filled(p(b.0, b.1), 2.15 * scale, cross);
    }

    // Near edges, including the front vertical, drawn over the emblem.
    lines(
        &[[(6.0, 18.0), center], [(58.0, 18.0), center], [center, (32.0, 61.0)]],
        1.1,
        hex(0x4A2942, 0.85),
    );
    let outline: Vec<Pos2> = [(32.0, 3.0), (58.0, 18.0), (58.0, 46.0), (32.0, 61.0), (6.0, 46.0), (6.0, 18.0)]
        .iter()
        .map(|&(x, y)| p(x, y))
        .collect();
    painter.add(Shape::closed_line(outline, Stroke::new(1.3 * scale, hex(0x4A2942, 1.0))));
}

/// A radial glow: `inner` at the centre, `middle` halfway out, transparent at the edge.
/// Mesh colours blend between vertices, which stands in for the SVG's radial gradient.
fn glow(center: Pos2, radius: f32, inner: Color32, middle: Color32) -> Mesh {
    const SEGMENTS: u32 = 48;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, inner);
    for i in 0..SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * TAU;
        let direction = eframe::egui::vec2(angle.cos(), angle.sin());
        mesh.colored_vertex(center + direction * radius * 0.5, middle);
        mesh.colored_vertex(center + direction * radius, Color32::TRANSPARENT);
    }
    for i in 0..SEGMENTS {
        let (mid, edge) = (1 + 2 * i, 2 + 2 * i);
        let (next_mid, next_edge) = (1 + 2 * ((i + 1) % SEGMENTS), 2 + 2 * ((i + 1) % SEGMENTS));
        mesh.add_triangle(0, mid, next_mid);
        mesh.add_triangle(mid, edge, next_edge);
        mesh.add_triangle(mid, next_edge, next_mid);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years_from_days() {
        assert_eq!(year_from_days(0), 1970); // 1970-01-01
        assert_eq!(year_from_days(19_722), 2023); // 2023-12-31
        assert_eq!(year_from_days(19_723), 2024); // 2024-01-01
        assert_eq!(year_from_days(20_736), 2026); // 2026-10-10
    }
}
