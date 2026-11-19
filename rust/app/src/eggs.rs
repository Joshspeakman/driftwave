//! A theme's easter eggs (ported from the C# app's SceneEffect): typed words (outside text fields), the Konami code,
//! clicks on scene items and idle time set them off. They reveal hidden signs, say something in a bubble, or play a
//! little scene: Snake playing itself, two paddles keeping a ball in play, or a solitaire win's bouncing cards.
//!
//! Everything fades in and out over a fraction of a second or more: nothing flashes. The little scenes are a short
//! list of sprites on a board over the page, so only the board redraws while they play.

use super::{log, App, MainWindow, Scenery, SceneText, Sprite};
use crate::scene::{coord, Geometry, Rect};
use driftwave_core::theme::{Color, Theme};
use serde_json::{Map, Value};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::collections::HashSet;
use std::rc::Rc;
use slint::winit_030::winit;
use std::time::Instant;

#[derive(Clone, Debug)]
struct Egg {
    trigger: String,
    show: Option<String>,
    say: Option<String>,
    run: Option<String>,
    seconds: f64,
    near: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Key {
    Up,
    Down,
    Left,
    Right,
    Letter(char),
    Other,
}

const KONAMI: [Key; 10] = [Key::Up, Key::Up, Key::Down, Key::Down, Key::Left, Key::Right, Key::Left, Key::Right, Key::Letter('b'), Key::Letter('a')];

enum Game {
    Snake { cols: i32, rows: i32, body: Vec<(i32, i32)>, food: (i32, i32), next: f64, seed: u32 },
    Pong { x: f32, y: f32, vx: f32, vy: f32 },
    Cascade { cards: Vec<(f32, f32, f32, f32, usize)>, trail_at: f64 },
}

struct Play {
    game: Game,
    start: f64,
    seconds: f64,
    board: Rect,
    last: f64,
}

#[derive(Default)]
pub struct Eggs {
    list: Vec<Egg>,
    /// Clickable scene items (logical px) and their ids.
    clickable: Vec<(Rect, String)>,
    typed: String,
    konami: usize,
    last_input: Option<Instant>,
    idle_fired: HashSet<usize>,
    /// Hidden signs being revealed: their index in the signs, when, and for how long.
    reveals: Vec<(usize, f64, f64)>,
    bubble: Option<(f64, f64)>,
    play: Option<Play>,
    started: Option<Instant>,
    /// Theme colours the little scenes use.
    ink: slint::Color,
    surface: slint::Color,
    sprites: Rc<VecModel<Sprite>>,
    trail: Rc<VecModel<Sprite>>,
}

fn slint_colour(c: Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

fn str_of<'a>(o: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

fn num(o: &Map<String, Value>, k: &str, d: f64) -> f64 {
    o.get(k).and_then(Value::as_f64).unwrap_or(d)
}

/// A fade: in over `fin` s, out over the last `fout` s.
fn fade(t: f64, seconds: f64, fin: f64, fout: f64) -> f32 {
    (t / fin).min((seconds - t) / fout).clamp(0.0, 1.0) as f32
}

fn sprite(x: f32, y: f32, w: f32, h: f32, fill: slint::Color) -> Sprite {
    Sprite { x, y, w, h, fill, text: "".into(), ink: fill, card: false }
}

impl App {
    /// The theme's eggs, and its clickable items (placed for the Full layout's geometry when it's painted).
    pub(crate) fn apply_eggs(&mut self, theme: Option<&Theme>, ink: Color, surface: Color) {
        let scene = theme.and_then(|t| t.effects.get("scene"));
        let eggs = &mut self.eggs;
        eggs.list = scene
            .and_then(|s| s.get("eggs"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_object)
                    .map(|e| Egg {
                        trigger: str_of(e, "trigger").unwrap_or("").to_string(),
                        show: str_of(e, "show").map(String::from),
                        say: str_of(e, "say").map(String::from),
                        run: str_of(e, "run").map(String::from),
                        seconds: num(e, "seconds", 6.0).max(1.0),
                        near: str_of(e, "near").map(String::from),
                    })
                    .collect()
            })
            .unwrap_or_default();
        eggs.clickable.clear();
        eggs.reveals.clear();
        eggs.bubble = None;
        eggs.play = None;
        eggs.typed.clear();
        eggs.konami = 0;
        eggs.idle_fired.clear();
        eggs.ink = slint_colour(Color { a: 255, ..ink });
        eggs.surface = slint_colour(surface);
        eggs.started.get_or_insert_with(Instant::now);
        if let Some(ui) = self.ui.upgrade() {
            let s = ui.global::<Scenery>();
            s.set_bubble_opacity(0.0);
            s.set_game_on(false);
            s.set_sprites(ModelRc::from(eggs.sprites.clone()));
            s.set_trail(ModelRc::from(eggs.trail.clone()));
        }
    }

    /// Where the theme's clickable items are, for this window's geometry.
    pub(crate) fn place_eggs(&mut self, theme_layers: &[Map<String, Value>], g: &Geometry) {
        self.eggs.clickable = theme_layers
            .iter()
            .filter(|l| l.get("clickable").and_then(Value::as_bool) == Some(true))
            .filter_map(|l| {
                let id = str_of(l, "id")?.to_string();
                let r = g.region(str_of(l, "in"));
                let (x, y) = (r.x + coord(l.get("x"), r.w, 0.0), r.y + coord(l.get("y"), r.h, 0.0));
                let (w, h) = match str_of(l, "type") {
                    Some("pixels") => {
                        let rows = l.get("rows").and_then(Value::as_array).map(|a| a.to_vec()).unwrap_or_default();
                        let cols = rows.iter().filter_map(Value::as_str).map(|s| s.chars().count()).max().unwrap_or(1);
                        let cell = num(l, "scale", 4.0) as f32;
                        (cols as f32 * cell, rows.len() as f32 * cell)
                    }
                    Some("text") => {
                        let size = num(l, "size", 14.0) as f32;
                        (str_of(l, "text").unwrap_or("").chars().count() as f32 * size * 0.62, size * 1.3)
                    }
                    Some("image") if l.get("w").is_none() => (r.w, coord(l.get("h"), r.h, r.h)),
                    Some("line") => {
                        let rows = l.get("repeat").and_then(Value::as_object).map(|o| num(o, "n", 1.0) * num(o, "dy", 0.0)).unwrap_or(0.0) as f32;
                        (coord(l.get("x2"), r.w, 0.0).abs(), coord(l.get("y2"), r.h, 0.0).abs() + rows)
                    }
                    _ => (coord(l.get("w"), r.w, 32.0), coord(l.get("h"), r.h, 32.0)),
                };
                Some((Rect { x, y, w: w.max(16.0), h: h.max(16.0) }, id))
            })
            .collect();
    }

    fn egg_now(&mut self) -> f64 {
        self.eggs.started.get_or_insert_with(Instant::now).elapsed().as_secs_f64()
    }

    /// Any input: the idle eggs start counting again.
    pub(crate) fn egg_input(&mut self) {
        self.eggs.last_input = Some(Instant::now());
        self.eggs.idle_fired.clear();
    }

    /// A key pressed anywhere in the window (`typing`: a text field has the focus).
    pub(crate) fn egg_key(&mut self, key: &winit::keyboard::Key, typing: bool) {
        use winit::keyboard::{Key as K, NamedKey};
        self.egg_input();
        if self.eggs.list.is_empty() {
            return;
        }
        let k = match key {
            K::Named(NamedKey::ArrowUp) => Key::Up,
            K::Named(NamedKey::ArrowDown) => Key::Down,
            K::Named(NamedKey::ArrowLeft) => Key::Left,
            K::Named(NamedKey::ArrowRight) => Key::Right,
            K::Character(s) => s.chars().next().filter(char::is_ascii_alphabetic).map(|c| Key::Letter(c.to_ascii_lowercase())).unwrap_or(Key::Other),
            _ => Key::Other,
        };
        let e = &mut self.eggs;
        e.konami = if k == KONAMI[e.konami] { e.konami + 1 } else if k == KONAMI[0] { 1 } else { 0 };
        if e.konami == KONAMI.len() {
            e.konami = 0;
            self.egg_fire("konami", None);
            return;
        }
        // Typed words count only outside text fields (a search for "snake" shouldn't set one off).
        if typing {
            return;
        }
        if let Key::Letter(c) = k {
            let e = &mut self.eggs;
            e.typed.push(c);
            if e.typed.len() > 24 {
                e.typed.remove(0);
            }
            let hit = e.list.iter().position(|egg| egg.trigger.strip_prefix("type:").is_some_and(|w| e.typed.ends_with(w)));
            if let Some(i) = hit {
                e.typed.clear();
                let egg = e.list[i].clone();
                self.egg_run(&egg, None);
            }
        }
    }

    /// A click at the pointer: a clickable scene item there sets its eggs off (the UI still gets the click).
    pub(crate) fn egg_click(&mut self, at: (f32, f32)) {
        self.egg_input();
        let hit = self.eggs.clickable.iter().find(|(r, _)| at.0 >= r.x && at.0 <= r.x + r.w && at.1 >= r.y && at.1 <= r.y + r.h).cloned();
        if let Some((r, id)) = hit {
            self.egg_fire(&format!("click:{id}"), Some((r.x + r.w / 2.0, r.y)));
        }
    }

    fn egg_fire(&mut self, trigger: &str, at: Option<(f32, f32)>) {
        for egg in self.eggs.list.clone().iter().filter(|e| e.trigger == trigger) {
            self.egg_run(egg, at);
        }
    }

    fn egg_box(&self, id: &str) -> Option<Rect> {
        self.eggs.clickable.iter().find(|(_, i)| i == id).map(|(r, _)| *r)
    }

    /// Test runs (-egg <trigger>): sets an egg off as if its trigger had happened (a click lands on its item).
    pub(crate) fn egg_test(&mut self, trigger: &str) {
        let at = trigger.strip_prefix("click:").and_then(|id| self.egg_box(id)).map(|r| (r.x + r.w / 2.0, r.y));
        self.egg_fire(trigger, at);
    }

    fn egg_run(&mut self, egg: &Egg, at: Option<(f32, f32)>) {
        log(&format!("egg {}", egg.trigger));
        let Some(ui) = self.ui.upgrade() else { return };
        let now = self.egg_now();
        let s = ui.global::<Scenery>();
        let content = self.egg_content(&ui);
        if let Some(id) = &egg.show {
            for (i, t) in self.look_texts().iter().enumerate() {
                if t.id.as_str() == id {
                    self.eggs.reveals.retain(|(j, _, _)| *j != i);
                    self.eggs.reveals.push((i, now, egg.seconds));
                }
            }
        }
        if let Some(say) = &egg.say {
            let near = at.or_else(|| egg.near.as_deref().and_then(|n| self.egg_box(n)).map(|r| (r.x + r.w / 2.0, r.y))).unwrap_or((content.x + content.w - 160.0, content.y + content.h - 120.0));
            s.set_bubble_text(say.as_str().into());
            s.set_bubble_ax(near.0);
            s.set_bubble_ay(near.1);
            self.eggs.bubble = Some((now, egg.seconds));
        }
        if let Some(run) = &egg.run {
            let area = egg.near.as_deref().and_then(|n| self.egg_box(n)).unwrap_or(content);
            let play = match run.as_str() {
                "snake" => {
                    let (cols, rows) = (((area.w.min(360.0)) / 10.0).max(8.0) as i32, ((area.h.min(220.0)) / 10.0).max(6.0) as i32);
                    let (bw, bh) = (cols as f32 * 10.0, rows as f32 * 10.0);
                    let body = (0..4).map(|i| (cols / 2 - i, rows / 2)).collect();
                    let mut g = Game::Snake { cols, rows, body, food: (0, 0), next: now, seed: 0x9E37_79B9 };
                    snake_food(&mut g);
                    Some((g, Rect { x: area.x + (area.w - bw) / 2.0, y: area.y + (area.h - bh) / 2.0, w: bw, h: bh }, self.eggs.surface.with_alpha(0.9)))
                }
                "pong" => {
                    let (bw, bh) = ((area.w - 40.0).min(420.0), (area.h - 40.0).min(240.0));
                    Some((Game::Pong { x: bw / 2.0, y: bh / 2.0, vx: 190.0, vy: 120.0 }, Rect { x: area.x + (area.w - bw) / 2.0, y: area.y + (area.h - bh) / 2.0, w: bw, h: bh }, slint::Color::from_argb_u8(230, 0, 0, 0)))
                }
                "cascade" => Some((Game::Cascade { cards: Vec::new(), trail_at: 0.0 }, content, slint::Color::from_argb_u8(0, 0, 0, 0))),
                _ => None,
            };
            if let Some((game, board, back)) = play {
                self.eggs.trail.set_vec(Vec::new());
                self.eggs.sprites.set_vec(Vec::new());
                s.set_game_x(board.x);
                s.set_game_y(board.y);
                s.set_game_w(board.w);
                s.set_game_h(board.h);
                s.set_game_back(back);
                s.set_game_on(true);
                self.eggs.play = Some(Play { game, start: now, seconds: egg.seconds, board, last: now });
            }
        }
    }

    fn look_texts(&self) -> Vec<SceneText> {
        self.ui.upgrade().map(|ui| ui.global::<Scenery>().get_texts().iter().collect()).unwrap_or_default()
    }

    /// The content area (between the pane and the player), in logical px.
    fn egg_content(&self, ui: &MainWindow) -> Rect {
        let scale = ui.window().scale_factor();
        let size = ui.window().size();
        let player = if ui.get_show_player() { 312.0 } else { 0.0 };
        let (w, h) = (size.width as f32 / scale, size.height as f32 / scale);
        Rect { x: 220.0, y: 48.0, w: (w - 220.0 - player).max(100.0), h: (h - 48.0).max(100.0) }
    }

    /// Moves the eggs along: idle checks, reveals, the bubble and the little scenes. Called every Full tick.
    pub(crate) fn tick_eggs(&mut self, ui: &MainWindow) {
        if self.eggs.list.is_empty() {
            return;
        }
        let now = self.egg_now();
        // Idle eggs: once per quiet spell.
        let quiet = self.eggs.last_input.get_or_insert_with(Instant::now).elapsed().as_secs_f64();
        let idle: Vec<(usize, Egg)> = self
            .eggs
            .list
            .iter()
            .enumerate()
            .filter(|(i, e)| e.trigger.strip_prefix("idle:").and_then(|s| s.parse::<f64>().ok()).is_some_and(|s| quiet > s) && !self.eggs.idle_fired.contains(i))
            .map(|(i, e)| (i, e.clone()))
            .collect();
        for (i, egg) in idle {
            self.eggs.idle_fired.insert(i);
            self.egg_run(&egg, None);
        }
        let s = ui.global::<Scenery>();
        // Reveals: in over 0.6 s, out over 0.8 s.
        if !self.eggs.reveals.is_empty() {
            let texts = s.get_texts();
            for &(i, start, secs) in &self.eggs.reveals {
                if let Some(mut t) = texts.row_data(i) {
                    t.opacity = fade(now - start, secs, 0.6, 0.8);
                    texts.set_row_data(i, t);
                }
            }
            self.eggs.reveals.retain(|&(_, start, secs)| now - start < secs);
        }
        if let Some((start, secs)) = self.eggs.bubble {
            let t = now - start;
            s.set_bubble_opacity(fade(t, secs, 0.35, 0.6));
            s.set_bubble_rise((8.0 * (1.0 - (t / 0.35).min(1.0))) as f32);
            if t >= secs {
                self.eggs.bubble = None;
                s.set_bubble_opacity(0.0);
            }
        }
        if self.eggs.play.is_some() {
            self.step_play(ui, now);
        }
    }

    fn step_play(&mut self, ui: &MainWindow, now: f64) {
        let s = ui.global::<Scenery>();
        let ink = self.eggs.ink;
        let Some(p) = self.eggs.play.as_mut() else { return };
        let t = now - p.start;
        if t >= p.seconds {
            self.eggs.play = None;
            s.set_game_on(false);
            self.eggs.sprites.set_vec(Vec::new());
            self.eggs.trail.set_vec(Vec::new());
            return;
        }
        let dt = (now - p.last).min(0.1) as f32;
        p.last = now;
        let (bw, bh) = (p.board.w, p.board.h);
        match &mut p.game {
            Game::Snake { .. } => {
                s.set_game_opacity(fade(t, p.seconds, 0.4, 0.6));
                let mut moved = false;
                if let Game::Snake { next, .. } = &p.game {
                    moved = now >= *next;
                }
                if moved {
                    snake_step(&mut p.game);
                    if let Game::Snake { next, body, food, .. } = &mut p.game {
                        *next = now + 0.12;
                        let mut v: Vec<Sprite> = body.iter().map(|&(x, y)| sprite(x as f32 * 10.0, y as f32 * 10.0, 9.0, 9.0, ink)).collect();
                        v.push(sprite(food.0 as f32 * 10.0 + 2.0, food.1 as f32 * 10.0 + 2.0, 5.0, 5.0, ink));
                        self.eggs.sprites.set_vec(v);
                    }
                }
            }
            Game::Pong { x, y, vx, vy } => {
                s.set_game_opacity(fade(t, p.seconds, 0.4, 0.6));
                *x += *vx * dt;
                *y += *vy * dt;
                if *y < 0.0 || *y > bh - 8.0 {
                    *vy = -*vy;
                    *y = y.clamp(0.0, bh - 8.0);
                }
                if *x < 18.0 || *x > bw - 26.0 {
                    *vx = -*vx;
                    *x = x.clamp(18.0, bw - 26.0);
                }
                let mut v = vec![
                    sprite(8.0, (*y - 18.0).clamp(0.0, bh - 44.0), 8.0, 44.0, ink),
                    sprite(bw - 16.0, (*y - 18.0 + (now as f32).sin() * 6.0).clamp(0.0, bh - 44.0), 8.0, 44.0, ink),
                    sprite(*x, *y, 8.0, 8.0, ink),
                ];
                // The net: a dashed line down the middle.
                let mut ny = 0.0;
                while ny < bh {
                    v.push(sprite(bw / 2.0 - 1.0, ny, 2.0, 3.0, ink.with_alpha(0.5)));
                    ny += 6.0;
                }
                self.eggs.sprites.set_vec(v);
            }
            Game::Cascade { cards, trail_at } => {
                s.set_game_opacity(((p.seconds - t) / 0.8).clamp(0.0, 1.0) as f32);
                const SUITS: [&str; 4] = ["♠", "♥", "♦", "♣"];
                if cards.len() < 4 && t > cards.len() as f64 * 1.2 {
                    let n = cards.len();
                    let speed = 80.0 + ((n as f32 * 0.37 + 0.2).fract()) * 90.0;
                    cards.push((bw - 70.0 - n as f32 * 50.0, 10.0, -speed, 0.0, n % 4));
                }
                let trail = now - *trail_at > 0.08 && self.eggs.trail.row_count() < 220;
                if trail {
                    *trail_at = now;
                }
                let card = |x: f32, y: f32, suit: usize| Sprite {
                    x,
                    y,
                    w: 42.0,
                    h: 58.0,
                    fill: slint::Color::from_rgb_u8(255, 255, 255),
                    text: SUITS[suit].into(),
                    ink: if suit == 1 || suit == 2 { slint::Color::from_rgb_u8(220, 0, 0) } else { slint::Color::from_rgb_u8(0, 0, 0) },
                    card: true,
                };
                let mut v = Vec::new();
                for c in cards.iter_mut() {
                    c.3 += 900.0 * dt;
                    c.0 += c.2 * dt;
                    c.1 += c.3 * dt;
                    if c.1 > bh - 58.0 {
                        c.1 = bh - 58.0;
                        c.3 = -c.3 * 0.78;
                    }
                    if trail && c.0 > -40.0 {
                        self.eggs.trail.push(card(c.0, c.1, c.4));
                    }
                    v.push(card(c.0, c.1, c.4));
                }
                self.eggs.sprites.set_vec(v);
            }
        }
    }
}

fn snake_rand(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}

fn snake_food(g: &mut Game) {
    if let Game::Snake { cols, rows, body, food, seed, .. } = g {
        loop {
            let f = ((snake_rand(seed) % *cols as u32) as i32, (snake_rand(seed) % *rows as u32) as i32);
            if !body.contains(&f) {
                *food = f;
                break;
            }
        }
    }
}

/// One step: head for the food; never back onto itself; wrap round the edges.
fn snake_step(g: &mut Game) {
    let mut ate = false;
    if let Game::Snake { cols, rows, body, food, .. } = g {
        let head = body[0];
        let dirs = [((food.0 - head.0).signum(), 0), (0, (food.1 - head.1).signum()), (1, 0), (0, 1), (-1, 0), (0, -1)];
        for (dx, dy) in dirs {
            if dx == 0 && dy == 0 {
                continue;
            }
            let n = ((head.0 + dx + *cols) % *cols, (head.1 + dy + *rows) % *rows);
            if body[..body.len() - 1].contains(&n) {
                continue;
            }
            body.insert(0, n);
            if n == *food {
                ate = true;
            } else {
                body.pop();
            }
            break;
        }
    }
    if ate {
        snake_food(g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_finds_its_food_and_grows() {
        let mut g = Game::Snake { cols: 20, rows: 12, body: (0..4).map(|i| (10 - i, 6)).collect(), food: (0, 0), next: 0.0, seed: 7 };
        snake_food(&mut g);
        for _ in 0..400 {
            snake_step(&mut g);
        }
        let Game::Snake { body, cols, rows, .. } = &g else { unreachable!() };
        assert!(body.len() > 4, "it ate");
        let mut seen = HashSet::new();
        assert!(body.iter().all(|&(x, y)| x >= 0 && y >= 0 && x < *cols && y < *rows && seen.insert((x, y))), "no overlaps, on the board");
    }

    #[test]
    fn fades_never_jump() {
        let mut last = 0.0;
        for i in 0..=600 {
            let v = fade(i as f64 / 100.0, 6.0, 0.6, 0.8);
            assert!((v - last).abs() <= 0.02 + 1e-6, "smooth: {last} -> {v}");
            last = v;
        }
    }
}
