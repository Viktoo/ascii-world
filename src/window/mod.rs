//! `pocket window`: the game in a window of its own. The world is drawn at the
//! window detail setting and scaled up smoothly; the same screen of cells the
//! terminal shows is laid over it, its words set in a sans-serif. Everything
//! else (the sim, the menus, the keys) is the terminal game's own.

mod gfx;
mod text;

use crate::app::{App, Frontend, WinGeom};
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

/// `pocket window --shot`: run the game unseen for a while and save the window as a PNG.
pub struct Shot {
    pub out: PathBuf,
    pub size: (u32, u32),
    pub scale: f32,
    pub after: f32,
    /// Keys pressed a second before the shot (`esc,down,f1` or single characters).
    pub keys: Vec<String>,
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
        f if f.starts_with('f') && f.len() > 1 => KeyCode::F(f[1..].parse().ok()?),
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next()?),
        _ => return None,
    })
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

/// Play `app` in a window until it quits (or, with `shot`, render one offscreen).
pub fn run(app: &mut App, stop: &AtomicBool, shot: Option<Shot>) -> Result<()> {
    if let Some(s) = shot {
        return run_shot(app, stop, s);
    }
    let mut builder = EventLoop::builder();
    // No stock menu: its Quit ends the process unsaved. Cmd+Q is ours instead.
    #[cfg(target_os = "macos")]
    winit::platform::macos::EventLoopBuilderExtMacOS::with_default_menu(&mut builder, false);
    let el = builder.build().context("opening a window")?;
    let mut front = WinFront { el, h: Handler::default(), gpu: None, size: (0, 0), geom: geom(1.0), grabbed: false };
    // The window appears on the first pump.
    let t0 = Instant::now();
    while front.h.window.is_none() && t0.elapsed() < Duration::from_secs(5) {
        front.el.pump_app_events(Some(Duration::from_millis(10)), &mut front.h);
    }
    let window = front.h.window.clone().context("the window did not open")?;
    front.connect(window)?;
    front.fit(app);
    app.run_on(&mut front, stop, None)?;
    Ok(())
}

fn run_shot(app: &mut App, stop: &AtomicBool, s: Shot) -> Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).context("no GPU adapter")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let mut comp = Compositor::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
    let (cols, rows, g) = grid(s.size, geom(s.scale as f64));
    app.set_window(g);
    app.resize(cols, rows);
    let mut front = ShotFront;
    let end = Instant::now() + Duration::from_secs_f32(s.after);
    let mut keys: Vec<KeyCode> = s.keys.iter().filter_map(|k| named(k)).collect();
    let mut script = |app: &mut App, _: f32| {
        if !keys.is_empty() && Instant::now() + Duration::from_secs(1) >= end {
            for k in keys.drain(..) {
                app.on_key(KeyEvent::new_with_kind(k, KeyModifiers::empty(), KeyEventKind::Press));
                app.on_key(KeyEvent::new_with_kind(k, KeyModifiers::empty(), KeyEventKind::Release));
            }
        }
        Instant::now() < end
    };
    app.run_on(&mut front, stop, Some(&mut script))?;
    let rgb = comp.snapshot(s.size, &app.screen, app.view_rows(), app.last_frame(), (g.cell_w, g.cell_h));
    std::fs::write(&s.out, crate::png::encode(s.size.0, s.size.1, &rgb))?;
    println!("{}", app.stats_line());
    Ok(())
}

struct ShotFront;

impl Frontend for ShotFront {
    fn input(&mut self, _: &mut App) -> std::io::Result<()> {
        Ok(())
    }
    fn present(&mut self, _: &mut App) -> std::io::Result<()> {
        Ok(())
    }
    fn wait(&mut self, d: Duration) {
        std::thread::sleep(d.min(Duration::from_millis(2)));
    }
}

struct Gpu {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    comp: Compositor,
}

struct WinFront {
    el: EventLoop<()>,
    h: Handler,
    gpu: Option<Gpu>,
    size: (u32, u32),
    geom: WinGeom,
    grabbed: bool,
}

impl WinFront {
    fn connect(&mut self, window: Arc<Window>) -> Result<()> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .context("no GPU adapter for the window")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        let size = window.inner_size();
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).context("the window's surface is not supported")?;
        // Colours as the terminal shows them: no sRGB conversion.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        config.present_mode = if std::env::var_os("POCKET_NO_VSYNC").is_some() { wgpu::PresentMode::AutoNoVsync } else { wgpu::PresentMode::AutoVsync };
        surface.configure(&device, &config);
        let comp = Compositor::new(device, queue, config.format);
        crate::log::info(format!("window: {} ({:?}), {:?}", adapter.get_info().name, adapter.get_info().backend, config.format));
        self.gpu = Some(Gpu { surface, config, comp });
        Ok(())
    }

    /// Size the app's screen to the window.
    fn fit(&mut self, app: &mut App) {
        let Some(w) = self.h.window.clone() else { return };
        let s = w.inner_size();
        self.size = (s.width.max(1), s.height.max(1));
        if let Some(g) = self.gpu.as_mut() {
            g.config.width = self.size.0;
            g.config.height = self.size.1;
            g.comp.text = text::Text::new();
            g.surface.configure(&g.comp.device, &g.config);
        }
        let (cols, rows, geom) = grid(self.size, geom(w.scale_factor()));
        self.geom = geom;
        app.set_window(geom);
        app.resize(cols, rows);
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

impl Frontend for WinFront {
    fn input(&mut self, app: &mut App) -> std::io::Result<()> {
        self.el.pump_app_events(Some(Duration::ZERO), &mut self.h);
        for ev in std::mem::take(&mut self.h.events) {
            match ev {
                Ev::Key(k) => app.on_key(k),
                Ev::Resized => self.fit(app),
                Ev::Focus(false) => {
                    app.focus_lost();
                    self.grab(false);
                }
                Ev::Focus(true) => {}
                Ev::Look(dx, dy) if self.grabbed => app.mouse_look(dx, dy),
                Ev::Look(..) => {}
                Ev::Click => {
                    if app.wants_mouse() {
                        self.grab(true);
                    }
                }
                Ev::Close => app.quit_now(),
            }
        }
        // Mouse look only while walking: menus and typing get the pointer back.
        if self.grabbed && !app.wants_mouse() {
            self.grab(false);
        }
        Ok(())
    }

    fn present(&mut self, app: &mut App) -> std::io::Result<()> {
        let Some(g) = self.gpu.as_mut() else { return Ok(()) };
        let tex = match g.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                g.surface.configure(&g.comp.device, &g.config);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let view = tex.texture.create_view(&Default::default());
        g.comp.draw(&view, self.size, &app.screen, app.view_rows(), app.last_frame(), (self.geom.cell_w, self.geom.cell_h));
        if let Some(w) = self.h.window.as_ref() {
            w.pre_present_notify();
        }
        g.comp.queue.present(tex);
        Ok(())
    }

    fn wait(&mut self, d: Duration) {
        self.el.pump_app_events(Some(d), &mut self.h);
    }
}

enum Ev {
    Key(KeyEvent),
    Resized,
    Focus(bool),
    Look(f32, f32),
    Click,
    Close,
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
                if quit && event.state == ElementState::Pressed {
                    self.events.push(Ev::Close);
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
