//! Painting helpers: the paper grid, pencil lines, frames and arrows.

use egui::epaint::{CubicBezierShape, PathShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use super::layout::wobble_of;
use super::theme::Theme;

/// Dots on the paper. The spacing adapts so the grid never gets dense when zoomed out.
pub fn grid(p: &Painter, screen: Rect, center: Pos2, zoom: f32, theme: &Theme) {
    let mut spacing = 24.0;
    while spacing * zoom < 16.0 {
        spacing *= 4.0;
    }
    let s = spacing * zoom;
    // Fade dots in as a finer grid becomes visible.
    let fade = ((s - 16.0) / 24.0).clamp(0.25, 1.0);
    let color = theme.grid.gamma_multiply(fade);
    let origin = screen.center() - center.to_vec2() * zoom;
    let x0 = ((screen.min.x - origin.x) / s).floor() as i64;
    let x1 = ((screen.max.x - origin.x) / s).ceil() as i64;
    let y0 = ((screen.min.y - origin.y) / s).floor() as i64;
    let y1 = ((screen.max.y - origin.y) / s).ceil() as i64;
    if (x1 - x0) * (y1 - y0) > 20_000 {
        return;
    }
    let r = (1.1 * zoom.clamp(0.6, 1.4)).max(0.8);
    let mut mesh = egui::Mesh::default();
    for ix in x0..=x1 {
        for iy in y0..=y1 {
            let c = pos2(origin.x + ix as f32 * s, origin.y + iy as f32 * s);
            mesh.add_colored_rect(Rect::from_center_size(c, Vec2::splat(r * 2.0)), color);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// A deterministic small offset for hand-drawn lines.
fn jitter(seed: &str, i: usize, amount: f32) -> Vec2 {
    let a = wobble_of(&format!("{seed}{i}"));
    let b = wobble_of(&format!("{i}{seed}x"));
    vec2(a, b) * amount
}

/// A line that looks drawn with a pencil when the theme wobbles.
pub fn pencil_line(
    p: &Painter,
    pts: &[Pos2],
    stroke: Stroke,
    theme: &Theme,
    seed: &str,
    zoom: f32,
) {
    if theme.wobble <= 0.0 || pts.len() < 2 {
        p.add(Shape::line(pts.to_vec(), stroke));
        return;
    }
    let w = theme.wobble * zoom.clamp(0.3, 2.0);
    let a: Vec<Pos2> = pts
        .iter()
        .enumerate()
        .map(|(i, q)| *q + jitter(seed, i / 6, w * 0.5))
        .collect();
    p.add(Shape::line(a, stroke));
    let b: Vec<Pos2> = pts
        .iter()
        .enumerate()
        .map(|(i, q)| *q + jitter(seed, i / 6 + 99, w * 0.8))
        .collect();
    p.add(Shape::line(
        b,
        Stroke::new(stroke.width * 0.55, stroke.color.gamma_multiply(0.55)),
    ));
}

/// Points around a rounded rectangle (optionally rotated about its center).
pub fn rounded_rect_points(r: Rect, radius: f32, angle: f32) -> Vec<Pos2> {
    let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let mut pts = Vec::new();
    let corners = [
        (pos2(r.max.x - rad, r.min.y + rad), -90.0f32),
        (pos2(r.max.x - rad, r.max.y - rad), 0.0),
        (pos2(r.min.x + rad, r.max.y - rad), 90.0),
        (pos2(r.min.x + rad, r.min.y + rad), 180.0),
    ];
    for (c, start) in corners {
        for k in 0..=6 {
            let a = (start + k as f32 * 15.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    if angle != 0.0 {
        let center = r.center();
        let (s, co) = angle.sin_cos();
        for q in &mut pts {
            let d = *q - center;
            *q = center + vec2(d.x * co - d.y * s, d.x * s + d.y * co);
        }
    }
    pts
}

/// A dashed rounded frame (Sketchbook areas).
pub fn dashed_frame(p: &Painter, r: Rect, radius: f32, stroke: Stroke, dash: f32, gap: f32) {
    let mut pts = rounded_rect_points(r, radius, 0.0);
    pts.push(pts[0]);
    p.extend(Shape::dashed_line(&pts, stroke, dash, gap));
}

/// The outline of a hand-drawn card: two slightly different passes.
#[allow(clippy::too_many_arguments)]
pub fn sketch_outline(
    p: &Painter,
    r: Rect,
    radius: f32,
    angle: f32,
    stroke: Stroke,
    seed: &str,
    theme: &Theme,
    zoom: f32,
) {
    let mut pts = rounded_rect_points(r, radius, angle);
    pts.push(pts[0]);
    pencil_line(p, &pts, stroke, theme, seed, zoom);
}

/// Where a line from `r`'s center towards `toward` leaves `r`, and the side's outward normal.
pub fn exit_point(r: Rect, toward: Pos2) -> (Pos2, Vec2) {
    let c = r.center();
    let d = toward - c;
    // Keep away from rounded corners, but never invert the range on tiny rects.
    let inset = 8.0f32.min(r.height() / 2.0).max(0.0);
    let cy =
        c.y.clamp(r.min.y + inset, (r.max.y - inset).max(r.min.y + inset));
    if d.x.abs() * r.height() > d.y.abs() * r.width() {
        if d.x > 0.0 {
            (pos2(r.max.x, cy), Vec2::X)
        } else {
            (pos2(r.min.x, cy), -Vec2::X)
        }
    } else if d.y > 0.0 {
        (pos2(c.x, r.max.y), Vec2::Y)
    } else {
        (pos2(c.x, r.min.y), -Vec2::Y)
    }
}

/// Which side of `r` faces `toward`: 0 right, 1 left, 2 bottom, 3 top.
pub fn side_of(r: Rect, toward: Pos2) -> u8 {
    let d = toward - r.center();
    if d.x.abs() * r.height() > d.y.abs() * r.width() {
        if d.x > 0.0 { 0 } else { 1 }
    } else if d.y > 0.0 {
        2
    } else {
        3
    }
}

/// A point on a side of `r` (`frac` 0..1 along it) and the side's outward normal.
pub fn port(r: Rect, side: u8, frac: f32) -> (Pos2, Vec2) {
    let m = 10.0f32.min(r.width() * 0.2).min(r.height() * 0.2);
    let lerp = |a: f32, b: f32| a + m + (b - a - 2.0 * m).max(0.0) * frac;
    match side {
        0 => (pos2(r.max.x, lerp(r.min.y, r.max.y)), Vec2::X),
        1 => (pos2(r.min.x, lerp(r.min.y, r.max.y)), -Vec2::X),
        2 => (pos2(lerp(r.min.x, r.max.x), r.max.y), Vec2::Y),
        _ => (pos2(lerp(r.min.x, r.max.x), r.min.y), -Vec2::Y),
    }
}

/// A curve between two ports. Returns the points, the label anchor and the end direction.
pub fn curve(p0: Pos2, n0: Vec2, p3: Pos2, n3: Vec2) -> (Vec<Pos2>, Pos2, Vec2) {
    let dist = (p3 - p0).length();
    let k = (dist * 0.4).clamp(20.0, 220.0);
    let bez = CubicBezierShape::from_points_stroke(
        [p0, p0 + n0 * k, p3 + n3 * k, p3],
        false,
        Color32::TRANSPARENT,
        PathStroke::NONE,
    );
    (bez.flatten(Some(0.4)), bez.sample(0.5), -n3)
}

/// A connection curve between two rects. Returns the curve points, and the label anchor.
pub fn connection_curve(a: Rect, b: Rect) -> (Vec<Pos2>, Pos2, Vec2) {
    let (p0, n0) = exit_point(a, b.center());
    let (p3, n3) = exit_point(b, a.center());
    let dist = (p3 - p0).length();
    let k = (dist * 0.4).clamp(20.0, 220.0);
    let bez = CubicBezierShape::from_points_stroke(
        [p0, p0 + n0 * k, p3 + n3 * k, p3],
        false,
        Color32::TRANSPARENT,
        PathStroke::NONE,
    );
    let pts = bez.flatten(Some(0.4));
    let mid = bez.sample(0.5);
    let tangent = (bez.sample(0.98) - bez.sample(0.9)).normalized();
    let _ = tangent;
    let end_dir = (p3 - (p3 + n3 * k)).normalized();
    (pts, mid, end_dir)
}

/// An open arrowhead at `tip` pointing along `dir`.
pub fn arrowhead(p: &Painter, tip: Pos2, dir: Vec2, size: f32, stroke: Stroke, filled: bool) {
    let dir = if dir.length() > 0.0 {
        dir.normalized()
    } else {
        Vec2::X
    };
    let back = tip - dir * size;
    let side = vec2(-dir.y, dir.x) * size * 0.55;
    if filled {
        p.add(Shape::convex_polygon(
            vec![tip, back + side, back - side],
            stroke.color,
            Stroke::NONE,
        ));
    } else {
        p.add(Shape::line(vec![back + side, tip, back - side], stroke));
    }
}

/// A soft glow around a rounded rect (Ember selection).
pub fn glow(p: &Painter, r: Rect, radius: f32, color: Color32, strength: f32) {
    for i in 1..=6 {
        let grow = i as f32 * 2.5 * strength;
        let a = (0.22 / i as f32) * strength.min(1.0);
        p.add(Shape::Path(PathShape::closed_line(
            rounded_rect_points(r.expand(grow), radius + grow, 0.0),
            Stroke::new(2.5 * strength, color.gamma_multiply(a)),
        )));
    }
}

/// Rotates `q` around `center`.
pub fn rotate(q: Pos2, center: Pos2, angle: f32) -> Pos2 {
    if angle == 0.0 {
        return q;
    }
    let (s, c) = angle.sin_cos();
    let d = q - center;
    center + vec2(d.x * c - d.y * s, d.x * s + d.y * c)
}
