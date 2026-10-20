//! Development aid: drive the window from a script (`SKISS_SCRIPT=steps.txt`).
//!
//! Real input events are fed through eframe's raw input hook, so scripted runs exercise exactly
//! the code paths a person's mouse and keyboard do. One step per line:
//!
//! ```text
//! wait 0.5
//! click @anna-berg            # center of a card or area (by id)
//! dclick @people:40,12        # 40 x 12 points from its top-left corner
//! drag @erik:%100,50 @anna    # from the middle of Erik's right edge
//! click 300 200               # window points
//! drag @erik-lund 900 600     # press, move over a few frames, release
//! shiftdrag 100 100 400 400
//! type Hello world
//! key cmd+b                   # modifiers: cmd, shift, alt, ctrl
//! scroll 0 -120               # pan
//! pinch 1.5                   # zoom around the pointer
//! shot /tmp/out.png
//! quit
//! ```

use std::collections::VecDeque;
use std::path::PathBuf;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};

#[derive(Debug, Clone)]
enum Target {
    Point(Pos2),
    /// `offset` in points from the top-left, or in percent of the size when `percent`.
    Node {
        id: String,
        offset: Option<(f32, f32)>,
        percent: bool,
    },
}

#[derive(Debug, Clone)]
enum Step {
    Wait(f64),
    Move(Target),
    Click(Target, Modifiers),
    DoubleClick(Target),
    Drag(Target, Target, Modifiers),
    Type(String),
    Key(Key, Modifiers),
    Scroll(f32, f32),
    Pinch(f32),
    Shot(PathBuf),
    Quit,
}

pub struct Script {
    steps: VecDeque<Step>,
    frames: VecDeque<Vec<Event>>,
    /// Modifier keys held during the queued frames.
    held: VecDeque<Modifiers>,
    wait_until: f64,
    pointer: Pos2,
}

fn parse_mods(s: &str) -> (Modifiers, String) {
    let mut m = Modifiers::NONE;
    let parts: Vec<&str> = s.split('+').collect();
    for p in &parts[..parts.len() - 1] {
        match p.to_lowercase().as_str() {
            "cmd" | "command" => {
                m.command = true;
                m.mac_cmd = cfg!(target_os = "macos");
                m.ctrl |= !cfg!(target_os = "macos");
            }
            "shift" => m.shift = true,
            "alt" | "opt" => m.alt = true,
            "ctrl" => m.ctrl = true,
            _ => {}
        }
    }
    (m, parts[parts.len() - 1].to_string())
}

fn parse_target(words: &[&str]) -> Option<(Target, usize)> {
    let first = words.first()?;
    if let Some(rest) = first.strip_prefix('@') {
        let (id, offset, percent) = match rest.split_once(':') {
            Some((id, off)) => {
                let percent = off.starts_with('%');
                let (x, y) = off.trim_start_matches('%').split_once(',')?;
                (
                    id.to_string(),
                    Some((x.parse().ok()?, y.parse().ok()?)),
                    percent,
                )
            }
            None => (rest.to_string(), None, false),
        };
        return Some((
            Target::Node {
                id,
                offset,
                percent,
            },
            1,
        ));
    }
    let x: f32 = words.first()?.parse().ok()?;
    let y: f32 = words.get(1)?.parse().ok()?;
    Some((Target::Point(pos2(x, y)), 2))
}

impl Script {
    pub fn load() -> Option<Script> {
        let path = std::env::var("SKISS_SCRIPT").ok()?;
        let text = std::fs::read_to_string(path).ok()?;
        let mut steps = VecDeque::new();
        for line in text.lines() {
            let line = line.split(" #").next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
            let words: Vec<&str> = rest.split_whitespace().collect();
            let Some(step) = Self::parse_step(cmd, rest, &words) else {
                eprintln!("script: cannot read '{line}'");
                continue;
            };
            steps.push_back(step);
        }
        Some(Script {
            steps,
            frames: VecDeque::new(),
            held: VecDeque::new(),
            wait_until: 1.5,
            pointer: pos2(10.0, 10.0),
        })
    }

    fn parse_step(cmd: &str, rest: &str, words: &[&str]) -> Option<Step> {
        Some(match cmd {
            "wait" => Step::Wait(rest.trim().parse().unwrap_or(0.3)),
            "move" => Step::Move(parse_target(words)?.0),
            "click" | "shiftclick" | "cmdclick" => {
                let m = match cmd {
                    "shiftclick" => Modifiers::SHIFT,
                    "cmdclick" => Modifiers::COMMAND,
                    _ => Modifiers::NONE,
                };
                Step::Click(parse_target(words)?.0, m)
            }
            "dclick" => Step::DoubleClick(parse_target(words)?.0),
            "drag" | "shiftdrag" => {
                let (a, n) = parse_target(words)?;
                let (b, _) = parse_target(&words[n..])?;
                let m = if cmd == "shiftdrag" {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                };
                Step::Drag(a, b, m)
            }
            "type" => Step::Type(rest.to_string()),
            "key" => {
                let (m, name) = parse_mods(rest.trim());
                let lower = name.to_lowercase();
                let named = match lower.as_str() {
                    "left" | "arrowleft" => Some(Key::ArrowLeft),
                    "right" | "arrowright" => Some(Key::ArrowRight),
                    "up" | "arrowup" => Some(Key::ArrowUp),
                    "down" | "arrowdown" => Some(Key::ArrowDown),
                    "esc" | "escape" => Some(Key::Escape),
                    "enter" | "return" => Some(Key::Enter),
                    "backspace" => Some(Key::Backspace),
                    "delete" => Some(Key::Delete),
                    "tab" => Some(Key::Tab),
                    _ => None,
                };
                let key = named
                    .or_else(|| Key::from_name(&name))
                    .or_else(|| Key::from_name(&name.to_uppercase()))
                    .or_else(|| {
                        let mut c = name.chars();
                        let f = c.next()?.to_uppercase().collect::<String>() + c.as_str();
                        Key::from_name(&f)
                    })?;
                Step::Key(key, m)
            }
            "scroll" => Step::Scroll(words.first()?.parse().ok()?, words.get(1)?.parse().ok()?),
            "pinch" => Step::Pinch(words.first()?.parse().ok()?),
            "shot" => Step::Shot(PathBuf::from(rest.trim())),
            "quit" => Step::Quit,
            _ => return None,
        })
    }

    /// Feeds the next scripted events into this frame's input.
    pub fn feed(
        &mut self,
        ctx: &egui::Context,
        raw: &mut egui::RawInput,
        resolve: &dyn Fn(&str) -> Option<Rect>,
    ) {
        ctx.request_repaint();
        if let Some(events) = self.frames.pop_front() {
            let m = self.held.pop_front().unwrap_or_default();
            raw.events.push(Event::ModifiersChanged(m));
            raw.events.extend(events);
            return;
        }
        raw.events.push(Event::ModifiersChanged(Modifiers::NONE));
        self.held.clear();
        let now = raw.time.unwrap_or(0.0);
        if now < self.wait_until {
            return;
        }
        let Some(step) = self.steps.pop_front() else {
            return;
        };
        let pos = |t: &Target| -> Pos2 {
            match t {
                Target::Point(p) => *p,
                Target::Node {
                    id,
                    offset,
                    percent,
                } => match resolve(id) {
                    Some(r) => match offset {
                        Some((x, y)) if *percent => {
                            r.min + vec2(r.width() * x / 100.0, r.height() * y / 100.0)
                        }
                        Some((x, y)) => r.min + vec2(*x, *y),
                        None => r.center(),
                    },
                    None => {
                        eprintln!("script: no node '{id}'");
                        pos2(-100.0, -100.0)
                    }
                },
            }
        };
        let press = |p: Pos2, pressed: bool, m: Modifiers| Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: m,
        };
        match step {
            Step::Wait(s) => self.wait_until = now + s,
            Step::Move(t) => {
                let p = pos(&t);
                self.pointer = p;
                self.frames.push_back(vec![Event::PointerMoved(p)]);
            }
            Step::Click(t, m) => {
                let p = pos(&t);
                self.held.extend([m, m, m]);
                self.pointer = p;
                self.frames.push_back(vec![Event::PointerMoved(p)]);
                self.frames.push_back(vec![press(p, true, m)]);
                self.frames.push_back(vec![press(p, false, m)]);
                self.wait_until = now + 0.35;
            }
            Step::DoubleClick(t) => {
                let p = pos(&t);
                self.pointer = p;
                self.frames.push_back(vec![Event::PointerMoved(p)]);
                self.frames.push_back(vec![press(p, true, Modifiers::NONE)]);
                self.frames
                    .push_back(vec![press(p, false, Modifiers::NONE)]);
                self.frames.push_back(vec![press(p, true, Modifiers::NONE)]);
                self.frames
                    .push_back(vec![press(p, false, Modifiers::NONE)]);
                self.wait_until = now + 0.4;
            }
            Step::Drag(a, b, m) => {
                let (pa, pb) = (pos(&a), pos(&b));
                self.held.extend(std::iter::repeat_n(m, 17));
                self.frames.push_back(vec![Event::PointerMoved(pa)]);
                self.frames.push_back(vec![press(pa, true, m)]);
                let n = 14;
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    self.frames
                        .push_back(vec![Event::PointerMoved(pa + (pb - pa) * t)]);
                }
                self.frames.push_back(vec![press(pb, false, m)]);
                self.pointer = pb;
                self.wait_until = now + 0.4;
            }
            Step::Type(text) => {
                for c in text.chars() {
                    self.frames.push_back(vec![Event::Text(c.to_string())]);
                }
            }
            Step::Key(key, m) => {
                let ev = |pressed| Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: m,
                };
                // Clipboard shortcuts arrive as their own events, like on a real keyboard.
                if m.command && key == Key::C {
                    self.frames.push_back(vec![Event::Copy]);
                } else if m.command && key == Key::X {
                    self.frames.push_back(vec![Event::Cut]);
                } else {
                    self.frames.push_back(vec![ev(true)]);
                    self.frames.push_back(vec![ev(false)]);
                    self.held.extend([m, m]);
                }
            }
            Step::Scroll(x, y) => {
                for _ in 0..6 {
                    self.frames.push_back(vec![Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(x, y) / 6.0,
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Trackpad,
                        modifiers: Modifiers::NONE,
                    }]);
                }
            }
            Step::Pinch(f) => {
                for _ in 0..8 {
                    self.frames.push_back(vec![Event::Zoom(f.powf(1.0 / 8.0))]);
                }
            }
            Step::Shot(path) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                    egui::ScreenshotCallback::new(move |img| {
                        let [w, h] = img.size;
                        let mut rgba = Vec::with_capacity(w * h * 4);
                        for px in &img.pixels {
                            rgba.extend_from_slice(&px.to_array());
                        }
                        if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                            let _ = buf.save(&path);
                        }
                    }),
                ));
                self.wait_until = now + 0.5;
            }
            Step::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }
}
