//! Springs for cards and areas, and the camera.
//!
//! Every visible thing has a target (from the layout) and a current state that follows it with a
//! critically damped spring, so layout changes glide instead of jumping. When everything has
//! settled, nothing asks for another frame: a still board costs no CPU.

use std::collections::HashMap;

use egui::{Pos2, Rect, Vec2, pos2, vec2};

#[derive(Clone, Copy, Debug)]
struct Spring {
    x: f32,
    v: f32,
}

impl Spring {
    fn new(x: f32) -> Self {
        Spring { x, v: 0.0 }
    }

    /// Critically damped step towards `target` (stiffness `k`).
    fn step(&mut self, target: f32, k: f32, dt: f32) {
        let c = 2.0 * k.sqrt();
        // Semi-implicit Euler with sub-steps keeps it stable at low frame rates.
        let steps = ((dt / 0.008).ceil() as usize).clamp(1, 8);
        let h = dt / steps as f32;
        for _ in 0..steps {
            let a = -k * (self.x - target) - c * self.v;
            self.v += a * h;
            self.x += self.v * h;
        }
    }

    fn settled(&self, target: f32, eps: f32) -> bool {
        (self.x - target).abs() < eps && self.v.abs() < eps * 4.0
    }

    fn snap(&mut self, target: f32) {
        self.x = target;
        self.v = 0.0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Anim {
    min_x: Spring,
    min_y: Spring,
    w: Spring,
    h: Spring,
    /// 0 -> 1 when appearing.
    pub appear: f32,
    appear_v: f32,
    /// Seconds to wait before appearing (staggered entrance).
    pub delay: f32,
    pub seen: bool,
}

impl Anim {
    fn new(rect: Rect, from: Option<Rect>, delay: f32) -> Self {
        let start = from.unwrap_or(rect);
        Anim {
            min_x: Spring::new(start.min.x),
            min_y: Spring::new(start.min.y),
            w: Spring::new(start.width()),
            h: Spring::new(start.height()),
            appear: 0.0,
            appear_v: 0.0,
            delay,
            seen: true,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(
            pos2(self.min_x.x, self.min_y.x),
            vec2(self.w.x.max(1.0), self.h.x.max(1.0)),
        )
    }

    /// Pop-in scale: slightly overshooting spring from 0.85 to 1.
    pub fn scale(&self) -> f32 {
        0.85 + 0.15 * self.appear
    }

    pub fn alpha(&self) -> f32 {
        (self.appear * 1.4).clamp(0.0, 1.0)
    }
}

#[derive(Default)]
pub struct Motion {
    pub anims: HashMap<String, Anim>,
    /// Where newly created items should fly in from (world point), keyed by nothing: the next new one.
    pub spawn_from: Option<Pos2>,
    /// Entrance stagger for the first frame.
    pub first: bool,
}

impl Motion {
    pub fn new() -> Self {
        Motion {
            anims: HashMap::new(),
            spawn_from: None,
            first: true,
        }
    }

    /// Advances every spring towards the targets. Returns true while anything still moves.
    pub fn step(
        &mut self,
        targets: &[(String, Rect)],
        dt: f32,
        overrides: &HashMap<String, Rect>,
    ) -> bool {
        let dt = dt.clamp(0.0, 0.05);
        let k = 260.0;
        let mut moving = false;
        for a in self.anims.values_mut() {
            a.seen = false;
        }
        let first = self.first;
        let mut new_count = 0usize;
        for (i, (id, target)) in targets.iter().enumerate() {
            let target = overrides.get(id).copied().unwrap_or(*target);
            let anim = self.anims.entry(id.clone()).or_insert_with(|| {
                new_count += 1;
                let delay = if first {
                    (i as f32 * 0.018).min(0.55)
                } else {
                    0.0
                };
                let from = self
                    .spawn_from
                    .filter(|_| !first)
                    .map(|p| Rect::from_center_size(p, target.size() * 0.4));
                let mut a = Anim::new(target, from, delay);
                if from.is_none() {
                    a.min_x.snap(target.min.x);
                    a.min_y.snap(target.min.y);
                }
                a
            });
            anim.seen = true;
            if overrides.contains_key(id) {
                // Dragged: follow the pointer exactly.
                anim.min_x.snap(target.min.x);
                anim.min_y.snap(target.min.y);
                anim.w.snap(target.width());
                anim.h.snap(target.height());
            } else {
                anim.min_x.step(target.min.x, k, dt);
                anim.min_y.step(target.min.y, k, dt);
                anim.w.step(target.width(), k, dt);
                anim.h.step(target.height(), k, dt);
            }
            if anim.delay > 0.0 {
                anim.delay -= dt;
                moving = true;
            } else if anim.appear < 0.999 || anim.appear_v.abs() > 0.001 {
                // Slightly bouncy pop.
                let kk: f32 = 300.0;
                let c = 2.0 * kk.sqrt() * 0.62;
                let steps = ((dt / 0.008).ceil() as usize).clamp(1, 8);
                let h = dt / steps as f32;
                for _ in 0..steps {
                    let acc = -kk * (anim.appear - 1.0) - c * anim.appear_v;
                    anim.appear_v += acc * h;
                    anim.appear += anim.appear_v * h;
                }
                if (anim.appear - 1.0).abs() < 0.001 && anim.appear_v.abs() < 0.001 {
                    anim.appear = 1.0;
                    anim.appear_v = 0.0;
                } else {
                    moving = true;
                }
            }
            let eps = 0.05;
            if !(anim.min_x.settled(target.min.x, eps)
                && anim.min_y.settled(target.min.y, eps)
                && anim.w.settled(target.width(), eps)
                && anim.h.settled(target.height(), eps))
            {
                moving = true;
            } else {
                anim.min_x.snap(target.min.x);
                anim.min_y.snap(target.min.y);
                anim.w.snap(target.width());
                anim.h.snap(target.height());
            }
        }
        self.anims.retain(|_, a| a.seen);
        if new_count > 0 {
            self.spawn_from = None;
        }
        self.first = false;
        moving
    }

    /// Keeps the animation state when an id changes (a renamed card).
    pub fn rename(&mut self, old: &str, new: &str) {
        if old != new
            && let Some(a) = self.anims.remove(old)
        {
            self.anims.insert(new.to_string(), a);
        }
    }

    pub fn get(&self, id: &str) -> Option<&Anim> {
        self.anims.get(id)
    }

    pub fn rect(&self, id: &str) -> Option<Rect> {
        self.anims.get(id).map(Anim::rect)
    }
}

/// The view onto the infinite canvas.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// World point at the center of the screen.
    pub center: Pos2,
    pub zoom: f32,
    target_center: Pos2,
    target_zoom: f32,
    /// While flying: keep this world point under this screen point.
    gliding: bool,
}

pub const MIN_ZOOM: f32 = 0.02;
pub const MAX_ZOOM: f32 = 16.0;

impl Camera {
    pub fn new(center: Pos2, zoom: f32) -> Self {
        Camera {
            center,
            zoom,
            target_center: center,
            target_zoom: zoom,
            gliding: false,
        }
    }

    pub fn to_screen(self, screen: Rect, p: Pos2) -> Pos2 {
        screen.center() + (p - self.center) * self.zoom
    }

    pub fn to_world(self, screen: Rect, p: Pos2) -> Pos2 {
        self.center + (p - screen.center()) / self.zoom
    }

    pub fn rect_to_screen(&self, screen: Rect, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(screen, r.min), self.to_screen(screen, r.max))
    }

    /// Immediate pan by a screen delta (trackpad scroll, drag).
    pub fn pan(&mut self, delta: Vec2) {
        self.center -= delta / self.zoom;
        self.target_center = self.center;
        self.target_zoom = self.zoom;
        self.gliding = false;
    }

    /// Immediate zoom around a screen point (pinch, ctrl+scroll).
    pub fn zoom_at(&mut self, screen: Rect, at: Pos2, factor: f32) {
        let world = self.to_world(screen, at);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = world - (at - screen.center()) / self.zoom;
        self.target_center = self.center;
        self.target_zoom = self.zoom;
        self.gliding = false;
    }

    /// Smoothly zooms around a screen point (keyboard).
    pub fn glide_zoom(&mut self, screen: Rect, at: Pos2, factor: f32) {
        let world = self.to_world(screen, at);
        let z = (self.target_zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.target_center = world - (at - screen.center()) / z;
        self.target_zoom = z;
        self.gliding = true;
    }

    /// Smoothly frames a world rect.
    pub fn fly_to(&mut self, screen: Rect, world: Rect, margin: f32, max_zoom: f32) {
        let avail = (screen.size() - Vec2::splat(margin * 2.0)).max(Vec2::splat(50.0));
        let z = (avail.x / world.width().max(1.0))
            .min(avail.y / world.height().max(1.0))
            .clamp(MIN_ZOOM, max_zoom);
        self.target_center = world.center();
        self.target_zoom = z;
        self.gliding = true;
    }

    pub fn jump_to(&mut self, screen: Rect, world: Rect, margin: f32, max_zoom: f32) {
        self.fly_to(screen, world, margin, max_zoom);
        self.center = self.target_center;
        self.zoom = self.target_zoom;
        self.gliding = false;
    }

    /// Moves the camera so `world` is visible (without zooming), if it is not.
    pub fn reveal(&mut self, screen: Rect, world: Rect, margin: f32) {
        let r = self.rect_to_screen(screen, world);
        let inner = screen.shrink(margin);
        let mut d = Vec2::ZERO;
        if r.width() < inner.width() {
            if r.min.x < inner.min.x {
                d.x = inner.min.x - r.min.x;
            } else if r.max.x > inner.max.x {
                d.x = inner.max.x - r.max.x;
            }
        }
        if r.height() < inner.height() {
            if r.min.y < inner.min.y {
                d.y = inner.min.y - r.min.y;
            } else if r.max.y > inner.max.y {
                d.y = inner.max.y - r.max.y;
            }
        }
        if d != Vec2::ZERO {
            self.target_center = self.center - d / self.zoom;
            self.target_zoom = self.zoom;
            self.gliding = true;
        }
    }

    /// Advances a glide. Returns true while moving.
    pub fn step(&mut self, dt: f32) -> bool {
        if !self.gliding {
            return false;
        }
        let t = 1.0 - (-dt.clamp(0.0, 0.05) * 9.0).exp();
        // Interpolate zoom in log space so it feels constant-speed.
        let lz = self.zoom.ln() + (self.target_zoom.ln() - self.zoom.ln()) * t;
        self.zoom = lz.exp();
        self.center += (self.target_center - self.center) * t;
        let done = (self.zoom / self.target_zoom - 1.0).abs() < 0.001
            && (self.target_center - self.center).length() * self.zoom < 0.3;
        if done {
            self.zoom = self.target_zoom;
            self.center = self.target_center;
            self.gliding = false;
        }
        !done
    }
}
