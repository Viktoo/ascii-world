//! The game's window. One window lives for the whole session and shows every
//! screen: your worlds, the new-world screen, loading, and play. The world is
//! drawn at the window detail setting and scaled up smoothly; a screen of text
//! cells is laid over it, its words set in a sans-serif.
//!
//! `--shot` runs the same screens offscreen and saves a PNG, for checking
//! changes without a display.

mod gfx;
pub(crate) mod taa;
mod text;

pub use taa::jitter;

use crate::app::{App, Frontend, WinGeom};
use crate::render::Frame;
use crate::term::Screen;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use gfx::Compositor;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{CursorGrabMode, Window, WindowId};

/// A text cell's height in logical pixels (its width is half that).
const CELL_H: f64 = 22.0;

/// `--shot`: run unseen for a while and save the window as a PNG.
pub struct Shot {
    pub out: PathBuf,
    pub size: (u32, u32),
    pub scale: f32,
    pub after: f32,
    /// Keys pressed a second before the shot (`esc,down,f1` or single
    /// characters); with a `+` (`+w,+right`), held down from the start.
    pub keys: Vec<String>,
    /// `--keys-at SECS`: press them this long before the shot instead.
    pub keys_at: f32,
}

impl Shot {
    /// `--shot OUT.png [--size WxH] [--scale K] [--after SECS] [--keys a,b]` from the command line.
    pub fn from_args(args: &[String]) -> Result<Option<Shot>> {
        let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
        let Some(out) = flag("--shot") else { return Ok(None) };
        let size = flag("--size").unwrap_or_else(|| "1600x1000".into());
        let (w, h) = size.split_once('x').context("--size expects WxH")?;
        Ok(Some(Shot {
            out: PathBuf::from(out),
            size: (w.parse()?, h.parse()?),
            scale: flag("--scale").and_then(|s| s.parse().ok()).unwrap_or(1.0),
            after: flag("--after").and_then(|s| s.parse().ok()).unwrap_or(8.0),
            keys_at: flag("--keys-at").and_then(|s| s.parse().ok()).unwrap_or(1.0),
            keys: flag("--keys").map(|k| k.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        }))
    }

    fn key_codes(&self) -> Vec<KeyCode> {
        self.keys.iter().filter(|k| !k.starts_with('+')).filter_map(|k| named(k)).collect()
    }

    fn held(&self) -> Vec<KeyCode> {
        self.keys.iter().filter_map(|k| k.strip_prefix('+')).filter_map(named).collect()
    }
}

fn named(k: &str) -> Option<KeyCode> {
    Some(match k.to_lowercase().as_str() {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "space" => KeyCode::Char(' '),
        "backspace" => KeyCode::Backspace,
        f if f.starts_with('f') && f.len() > 1 => KeyCode::F(f[1..].parse().ok()?),
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next()?),
        _ => return None,
    })
}

fn press(k: KeyCode) -> KeyEvent {
    KeyEvent::new_with_kind(k, KeyModifiers::empty(), KeyEventKind::Press)
}

fn geom(scale: f64) -> WinGeom {
    let h = (CELL_H * scale).round() as f32;
    WinGeom { cell_w: (h * 0.5).round(), cell_h: h }
}

fn grid(size: (u32, u32), g: WinGeom) -> (u16, u16, WinGeom) {
    let cols = ((size.0 as f32 / g.cell_w).floor() as u16).max(20);
    let rows = ((size.1 as f32 / g.cell_h).floor() as u16).max(10);
    // Stretch the cells a little so they fill the window exactly.
    (cols, rows, WinGeom { cell_w: size.0 as f32 / cols as f32, cell_h: size.1 as f32 / rows as f32 })
}

/// What happened in the window since last asked.
pub enum Ev {
    Key(KeyEvent),
    Resized,
    Focus(bool),
    Look(f32, f32),
    Click,
    Close,
    Paste(String),
}

struct Gpu {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

/// Where screens are shown: the window on the desktop, or offscreen for a shot.
pub struct Win {
    el: Option<EventLoop<()>>,
    h: Handler,
    gpu: Option<Gpu>,
    comp: Compositor,
    size: (u32, u32),
    scale: f64,
    geom: WinGeom,
    grabbed: bool,
    shot: Option<ShotState>,
    /// Set by SIGTERM / SIGINT: every screen closes as if the window had.
    pub stop: Arc<AtomicBool>,
}

struct ShotState {
    shot: Shot,
    start: Instant,
    keys: Vec<KeyCode>,
    /// Keys to press at the start and hold.
    held: Vec<KeyCode>,
    /// Drawn into every time a screen is shown, as a window would be; read at the end.
    target: wgpu::Texture,
    /// What the last screen said.
    text: Option<String>,
}

impl Win {
    /// Open the window (it appears on the first pump).
    pub fn open() -> Result<Win> {
        let mut builder = EventLoop::builder();
        // No stock menu: its Quit ends the process unsaved. Cmd+Q is ours instead.
        #[cfg(target_os = "macos")]
        winit::platform::macos::EventLoopBuilderExtMacOS::with_default_menu(&mut builder, false);
        let mut el = builder.build().context("opening a window")?;
        let mut h = Handler::default();
        let t0 = Instant::now();
        while h.window.is_none() && t0.elapsed() < Duration::from_secs(5) {
            el.pump_app_events(Some(Duration::from_millis(10)), &mut h);
        }
        let window = h.window.clone().context("the window did not open")?;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .context("no GPU adapter for the window")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        let s = window.inner_size();
        let mut config = surface.get_default_config(&adapter, s.width.max(1), s.height.max(1)).context("the window's surface is not supported")?;
        // Colours as the renderer made them: no sRGB conversion.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        config.present_mode = if std::env::var_os("POCKET_NO_VSYNC").is_some() { wgpu::PresentMode::AutoNoVsync } else { wgpu::PresentMode::AutoVsync };
        surface.configure(&device, &config);
        crate::log::info(format!("window: {} ({:?}), {:?}", adapter.get_info().name, adapter.get_info().backend, config.format));
        let comp = Compositor::new(device, queue, config.format);
        let mut win = Win { el: Some(el), h, gpu: Some(Gpu { surface, config }), comp, size: (1, 1), scale: 1.0, geom: geom(1.0), grabbed: false, shot: None, stop: crate::term::signal_flag() };
        win.fit();
        Ok(win)
    }

    /// No window: screens are drawn offscreen and the last one saved as `shot.out`
    /// once `shot.after` seconds have passed (pumps then report Close).
    pub fn offscreen(shot: Shot) -> Result<Win> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).context("no GPU adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        let comp = Compositor::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
        let size = shot.size;
        let scale = shot.scale as f64;
        let (_, _, g) = grid(size, geom(scale));
        let keys = shot.key_codes();
        let held = shot.held();
        let target = comp.offscreen(size);
        Ok(Win { el: None, h: Handler::default(), gpu: None, comp, size, scale, geom: g, grabbed: false, shot: Some(ShotState { shot, start: Instant::now(), keys, held, target, text: None }), stop: crate::term::signal_flag() })
    }

    /// The screen size in cells, and each cell's size in pixels.
    pub fn grid(&self) -> (u16, u16, WinGeom) {
        grid(self.size, geom(self.scale))
    }

    pub fn set_title(&self, title: &str) {
        if let Some(w) = &self.h.window {
            w.set_title(title);
        }
    }

    /// Follow the window's size.
    fn fit(&mut self) {
        let Some(w) = self.h.window.clone() else { return };
        let s = w.inner_size();
        self.size = (s.width.max(1), s.height.max(1));
        self.scale = w.scale_factor();
        if let Some(g) = self.gpu.as_mut() {
            g.config.width = self.size.0;
            g.config.height = self.size.1;
            g.surface.configure(&self.comp.device, &g.config);
        }
        self.comp.text = text::Text::new();
        self.geom = self.grid().2;
    }

    /// Wait up to `d` for input; everything that happened since the last call.
    pub fn pump(&mut self, d: Duration) -> Vec<Ev> {
        if let Some(s) = self.shot.as_mut() {
            let left = Duration::from_secs_f32(s.shot.after).saturating_sub(s.start.elapsed());
            std::thread::sleep(d.min(Duration::from_millis(2)).min(left));
            let mut out = std::mem::take(&mut self.h.events);
            out.extend(s.held.drain(..).map(|k| Ev::Key(press(k))));
            if !s.keys.is_empty() && left <= Duration::from_secs_f32(s.shot.keys_at) {
                for k in s.keys.drain(..) {
                    out.push(Ev::Key(press(k)));
                    out.push(Ev::Key(KeyEvent::new_with_kind(k, KeyModifiers::empty(), KeyEventKind::Release)));
                }
            } else if left.is_zero() {
                out.push(Ev::Close);
            }
            return out;
        }
        if let Some(el) = self.el.as_mut() {
            // Events kept back by an earlier wait come first; don't sleep past them.
            let d = if self.h.events.is_empty() { d } else { Duration::ZERO };
            el.pump_app_events(Some(d), &mut self.h);
        }
        let mut evs = std::mem::take(&mut self.h.events);
        if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            evs.push(Ev::Close);
        }
        if evs.iter().any(|e| matches!(e, Ev::Resized)) {
            self.fit();
        }
        if evs.iter().any(|e| matches!(e, Ev::Focus(false))) {
            self.grab(false);
        }
        evs
    }

    /// Show a screen: its top `view_rows` rows over `frame` (where its cells let the world through).
    pub fn present(&mut self, screen: &Screen, view_rows: u16, frame: Option<&Frame>) {
        let cell = (self.geom.cell_w, self.geom.cell_h);
        if let Some(s) = self.shot.as_mut() {
            // Offscreen: drawn like a window, read back at the end.
            s.text = Some(screen.plain_text());
            let view = s.target.create_view(&Default::default());
            self.comp.draw(&view, self.size, screen, view_rows, frame, cell);
            return;
        }
        let Some(g) = self.gpu.as_mut() else { return };
        let tex = match g.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                g.surface.configure(&self.comp.device, &g.config);
                return;
            }
            _ => return,
        };
        let view = tex.texture.create_view(&Default::default());
        self.comp.draw(&view, self.size, screen, view_rows, frame, cell);
        if let Some(w) = self.h.window.as_ref() {
            w.pre_present_notify();
        }
        self.comp.queue.present(tex);
    }

    /// Save the shot, if this is an offscreen window.
    pub fn finish(&mut self) -> Result<()> {
        if let Some(s) = self.shot.as_mut() {
            let text = s.text.take().context("nothing was shown before the shot")?;
            let rgb = self.comp.read_back(&s.target);
            std::fs::write(&s.shot.out, crate::png::encode(self.size.0, self.size.1, &rgb))?;
            // What it says, beside it, for reading without looking.
            std::fs::write(s.shot.out.with_extension("txt"), text)?;
        }
        Ok(())
    }

    pub fn is_shot(&self) -> bool {
        self.shot.is_some()
    }

    fn grab(&mut self, on: bool) {
        let Some(w) = self.h.window.as_ref() else { return };
        if on == self.grabbed {
            return;
        }
        if on {
            let ok = w.set_cursor_grab(CursorGrabMode::Locked).or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined)).is_ok();
            if !ok {
                return;
            }
        } else {
            let _ = w.set_cursor_grab(CursorGrabMode::None);
        }
        w.set_cursor_visible(!on);
        self.grabbed = on;
    }
}

/// Play `app` in the window until it quits.
pub fn play(win: &mut Win, app: &mut App, stop: &AtomicBool) -> Result<()> {
    let (cols, rows, g) = win.grid();
    app.set_window(g);
    app.resize(cols, rows);
    let mut front = PlayFront { win };
    app.run_on(&mut front, stop, None)?;
    front.win.grab(false);
    Ok(())
}

struct PlayFront<'a> {
    win: &'a mut Win,
}

impl PlayFront<'_> {
    fn handle(&mut self, app: &mut App, evs: Vec<Ev>) {
        for ev in evs {
            match ev {
                Ev::Key(k) => app.on_key(k),
                Ev::Resized => {
                    let (cols, rows, g) = self.win.grid();
                    app.set_window(g);
                    app.resize(cols, rows);
                }
                Ev::Focus(false) => app.focus_lost(),
                Ev::Focus(true) => {}
                Ev::Look(dx, dy) if self.win.grabbed => app.mouse_look(dx, dy),
                Ev::Look(..) => {}
                Ev::Click => {
                    if app.wants_mouse() {
                        self.win.grab(true);
                    }
                }
                Ev::Close => app.quit_now(),
                Ev::Paste(t) => app.paste(&t),
            }
        }
        // Mouse look only while walking: menus and typing get the pointer back.
        if self.win.grabbed && !app.wants_mouse() {
            self.win.grab(false);
        }
    }
}

impl Frontend for PlayFront<'_> {
    fn input(&mut self, app: &mut App) -> std::io::Result<()> {
        let evs = self.win.pump(Duration::ZERO);
        self.handle(app, evs);
        Ok(())
    }

    fn present(&mut self, app: &mut App) -> std::io::Result<()> {
        self.win.present(&app.screen, app.view_rows(), app.last_frame());
        Ok(())
    }

    fn wait(&mut self, d: Duration) {
        // Events that arrive while waiting are kept for the next `input`.
        let evs = self.win.pump(d);
        self.win.h.events.extend(evs);
    }
}

#[derive(Default)]
struct Handler {
    window: Option<Arc<Window>>,
    events: Vec<Ev>,
    mods: ModifiersState,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Pocket").with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        match el.create_window(attrs) {
            Ok(w) => self.window = Some(Arc::new(w)),
            Err(e) => crate::log::error(format!("window: {e}")),
        }
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.events.push(Ev::Close),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.events.push(Ev::Resized),
            WindowEvent::Focused(f) => self.events.push(Ev::Focus(f)),
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.events.push(Ev::Click),
            WindowEvent::KeyboardInput { event, .. } => {
                // Cmd+Q / Cmd+W (Mac), Alt+F4 elsewhere: save and quit.
                let quit = match &event.logical_key {
                    Key::Character(c) => self.mods.super_key() && matches!(c.as_str(), "q" | "w"),
                    Key::Named(NamedKey::F4) => self.mods.alt_key(),
                    _ => false,
                };
                // Cmd+V (Mac) or Ctrl+V: paste.
                let paste = matches!(&event.logical_key, Key::Character(c) if c.as_str().eq_ignore_ascii_case("v")) && (self.mods.super_key() || self.mods.control_key());
                if quit && event.state == ElementState::Pressed {
                    self.events.push(Ev::Close);
                } else if paste {
                    if event.state == ElementState::Pressed {
                        if let Some(t) = arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok()) {
                            self.events.push(Ev::Paste(t));
                        }
                    }
                } else if let Some(k) = key(&event, self.mods) {
                    self.events.push(Ev::Key(k));
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.events.push(Ev::Look(delta.0 as f32, delta.1 as f32));
        }
    }
}

/// A window key as the terminal would have reported it (with releases).
fn key(e: &winit::event::KeyEvent, mods: ModifiersState) -> Option<KeyEvent> {
    // Cmd shortcuts are the system's, not the game's keys.
    if mods.super_key() {
        return None;
    }
    let code = match &e.logical_key {
        Key::Named(n) => match n {
            NamedKey::ArrowUp => KeyCode::Up,
            NamedKey::ArrowDown => KeyCode::Down,
            NamedKey::ArrowLeft => KeyCode::Left,
            NamedKey::ArrowRight => KeyCode::Right,
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Escape => KeyCode::Esc,
            NamedKey::Backspace => KeyCode::Backspace,
            NamedKey::Delete => KeyCode::Delete,
            NamedKey::Tab => KeyCode::Tab,
            NamedKey::Home => KeyCode::Home,
            NamedKey::End => KeyCode::End,
            NamedKey::PageUp => KeyCode::PageUp,
            NamedKey::PageDown => KeyCode::PageDown,
            NamedKey::Space => KeyCode::Char(' '),
            NamedKey::F1 => KeyCode::F(1),
            NamedKey::F2 => KeyCode::F(2),
            NamedKey::F3 => KeyCode::F(3),
            NamedKey::F4 => KeyCode::F(4),
            NamedKey::F5 => KeyCode::F(5),
            NamedKey::F6 => KeyCode::F(6),
            NamedKey::F7 => KeyCode::F(7),
            NamedKey::F8 => KeyCode::F(8),
            NamedKey::F9 => KeyCode::F(9),
            NamedKey::F10 => KeyCode::F(10),
            NamedKey::F11 => KeyCode::F(11),
            NamedKey::F12 => KeyCode::F(12),
            _ => return None,
        },
        Key::Character(s) => KeyCode::Char(s.chars().next()?),
        _ => return None,
    };
    let mut m = KeyModifiers::empty();
    if mods.shift_key() {
        m |= KeyModifiers::SHIFT;
    }
    if mods.control_key() {
        m |= KeyModifiers::CONTROL;
    }
    if mods.alt_key() {
        m |= KeyModifiers::ALT;
    }
    let kind = match (e.state, e.repeat) {
        (ElementState::Released, _) => KeyEventKind::Release,
        (_, true) => KeyEventKind::Repeat,
        _ => KeyEventKind::Press,
    };
    Some(KeyEvent::new_with_kind(code, m, kind))
}
