use sdl2::event::Event;
use sdl2::keyboard::Scancode;
use sdl2::pixels::Color as SdlColor;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window as SdlWindow;
use sdl2::EventPump;
use sdl2::Sdl;

use crate::tokens::{role, Color, SCR_H, SCR_W};
use harbor_platform::Key;

pub struct Window {
    _sdl: Sdl,
    canvas: Canvas<SdlWindow>,
    events: EventPump,
}

pub struct Frame<'a> {
    canvas: &'a mut Canvas<SdlWindow>,
}

impl Window {
    pub fn open(title: &str) -> Result<Self, String> {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let mut builder = video.window(title, SCR_W, SCR_H);
        builder.allow_highdpi();
        if std::env::var("SDL_VIDEODRIVER").ok().as_deref() != Some("dummy") {
            builder.opengl();
        }
        let win = builder.build().map_err(|e| e.to_string())?;
        let mut canvas = match win.into_canvas().present_vsync().build() {
            Ok(c) => c,
            Err(_) => {
                let win = {
                    let mut b = video.window(title, SCR_W, SCR_H);
                    b.allow_highdpi();
                    b.build().map_err(|e| e.to_string())?
                };
                win.into_canvas()
                    .software()
                    .build()
                    .map_err(|e| e.to_string())?
            }
        };
        canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        let events = sdl.event_pump()?;
        Ok(Self {
            _sdl: sdl,
            canvas,
            events,
        })
    }

    pub fn poll_keys(&mut self) -> (bool, Vec<Key>) {
        let mut quit = false;
        let mut keys = Vec::new();
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => quit = true,
                Event::KeyDown {
                    scancode: Some(sc),
                    repeat: false,
                    ..
                } => {
                    if sc == Scancode::Q {
                        quit = true;
                    }
                    keys.push(harbor_platform::from_sdl_scancode(sc as i32));
                }
                _ => {}
            }
        }
        (quit, keys)
    }

    pub fn begin(&mut self) -> Frame<'_> {
        let bg = role::BG;
        self.canvas.set_draw_color(SdlColor::RGB(bg.r, bg.g, bg.b));
        self.canvas.clear();
        Frame {
            canvas: &mut self.canvas,
        }
    }
}

impl Frame<'_> {
    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, c: Color) {
        self.canvas
            .set_draw_color(SdlColor::RGBA(c.r, c.g, c.b, c.a));
        let _ = self.canvas.fill_rect(Rect::new(x, y, w, h));
    }

    pub fn present(self) {
        self.canvas.present();
    }
}
