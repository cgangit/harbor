use sdl2::event::Event;
use sdl2::keyboard::Scancode;
use sdl2::mouse::MouseButton;
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
    raw: RawEventLog,
}

/// Logs the first [`RawEventLog::LIMIT`] raw input events to the app log so
/// that codes reported by an unverified device (webOS remote keycodes, Magic
/// Remote pointer events) are visible without a debugger.
#[derive(Default)]
pub struct RawEventLog {
    seen: usize,
}

impl RawEventLog {
    pub const LIMIT: usize = 20;

    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the line for this event while under the budget, `None` after.
    pub fn record(&mut self, class: &str, codes: &[i64]) -> Option<String> {
        if self.seen >= Self::LIMIT {
            return None;
        }
        self.seen += 1;
        let list = codes
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "evt {class} codes=[{list}] {}/{}",
            self.seen,
            Self::LIMIT
        ))
    }
}

/// Map a `KeyDown` event onto `Key`. Prefers the scancode (host keyboards),
/// falls back to the keycode — some platforms, notably webOS, deliver only
/// `keycode`. Returns `None` only when the event carries neither.
pub fn key_from_event(scancode: Option<i32>, keycode: Option<i32>) -> Option<Key> {
    let from_sc = scancode.map(harbor_platform::from_sdl_scancode);
    let from_kc = keycode.map(harbor_platform::from_sdl_keycode);
    from_sc
        .filter(|k| *k != Key::Unknown)
        .or_else(|| from_kc.filter(|k| *k != Key::Unknown))
        .or(from_sc.or(from_kc))
}

/// Map a mouse button press onto `Key`. A left-button click activates the
/// focused item exactly like OK (spec: "Pointer click activates like OK").
pub fn key_from_mouse(button: i32) -> Option<Key> {
    match button {
        b if b == MouseButton::Left as i32 => Some(Key::Ok),
        _ => None,
    }
}

/// Event types sdl2-rs 0.37 can decode without panicking. `Event::from_ll`
/// converts unknown types through `EventType::try_from`, whose `transmute`
/// aborts on an invalid discriminant. The host's `libSDL2` is sdl2-compat
/// (SDL2 API over SDL3), which leaks SDL3-native window event numbers
/// (0x202..0x20F, e.g. 0x207 = PIXEL_SIZE_CHANGED) into the queue, so
/// anything outside this allowlist is dropped at the queue instead of being
/// polled. Fail-closed: a type we do not know costs nothing we use.
pub fn is_decodable_event_type(t: u32) -> bool {
    matches!(t,
        0x100..=0x107      // quit, app lifecycle, locale changed
        | 0x150            // display
        | 0x200..=0x201    // window, syswm
        | 0x300..=0x305    // key down/up, text editing/input, keymap, ext
        | 0x400..=0x403    // mouse motion/button/wheel
        | 0x600..=0x605    // joystick
        | 0x650..=0x65b    // game controller
        | 0x700..=0x702    // touch
        | 0x800..=0x802    // gesture
        | 0x900            // clipboard
        | 0x1000..=0x1003  // drop
        | 0x1100..=0x1101  // audio device
        | 0x1200           // sensor
        | 0x2000..=0x2001  // render targets/device reset
    )
}

unsafe extern "C" fn queue_filter(
    _userdata: *mut std::os::raw::c_void,
    event: *mut sdl2::sys::SDL_Event,
) -> std::os::raw::c_int {
    if event.is_null() {
        return 1;
    }
    is_decodable_event_type((*event).type_) as std::os::raw::c_int
}

/// Install the queue filter. Must run before any event is queued (i.e. before
/// the window exists), so `Window::open` calls it right after `sdl2::init`.
fn install_event_filter() {
    unsafe {
        sdl2::sys::SDL_SetEventFilter(Some(queue_filter), std::ptr::null_mut());
    }
}

pub struct Frame<'a> {
    canvas: &'a mut Canvas<SdlWindow>,
}

impl Window {
    pub fn open(title: &str) -> Result<Self, String> {
        let sdl = sdl2::init()?;
        install_event_filter();
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
            raw: RawEventLog::new(),
        })
    }

    pub fn poll_keys(&mut self) -> (bool, Vec<Key>) {
        let mut quit = false;
        let mut keys = Vec::new();
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => quit = true,
                Event::KeyDown {
                    scancode,
                    keycode,
                    repeat: false,
                    ..
                } => {
                    let sc = scancode.map(|s| s as i32);
                    let kc = keycode.map(|k| k.into_i32());
                    if let Some(line) =
                        self.raw.record("keydown", &[sc.unwrap_or(-1) as i64, kc.unwrap_or(-1) as i64])
                    {
                        harbor_base::log(&line);
                    }
                    if sc == Some(Scancode::Q as i32) {
                        quit = true;
                    }
                    if let Some(key) = key_from_event(sc, kc) {
                        keys.push(key);
                    }
                }
                Event::MouseButtonDown {
                    mouse_btn, clicks, ..
                } => {
                    let btn = mouse_btn as i32;
                    if let Some(line) = self.raw.record("mousedown", &[btn as i64, clicks as i64]) {
                        harbor_base::log(&line);
                    }
                    if let Some(key) = key_from_mouse(btn) {
                        keys.push(key);
                    }
                }
                Event::MouseMotion { x, y, .. } => {
                    if let Some(line) = self.raw.record("motion", &[x as i64, y as i64]) {
                        harbor_base::log(&line);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const MASK: i32 = 1 << 30;

    #[test]
    fn scancode_path_is_preferred() {
        // arrow key delivered with both scancode and keycode
        assert_eq!(
            key_from_event(Some(80), Some(80 | MASK)),
            Some(Key::Left)
        );
        // host keyboard: Return
        assert_eq!(key_from_event(Some(40), Some(13)), Some(Key::Ok));
    }

    #[test]
    fn keycode_only_event_still_maps() {
        // webOS-style event: scancode missing
        assert_eq!(key_from_event(None, Some(13)), Some(Key::Ok));
        assert_eq!(key_from_event(None, Some(27)), Some(Key::Back));
        assert_eq!(key_from_event(None, Some(82 | MASK)), Some(Key::Up));
        assert_eq!(key_from_event(None, Some(81 | MASK)), Some(Key::Down));
    }

    #[test]
    fn unmapped_scancode_falls_back_to_keycode() {
        // scancode present but unknown to us, keycode carries the meaning
        assert_eq!(key_from_event(Some(0), Some(13)), Some(Key::Ok));
        assert_eq!(key_from_event(Some(0), Some(81 | MASK)), Some(Key::Down));
    }

    #[test]
    fn event_without_any_code_yields_nothing() {
        assert_eq!(key_from_event(None, None), None);
    }

    #[test]
    fn unknown_codes_stay_unknown_not_none() {
        assert_eq!(key_from_event(Some(0), None), Some(Key::Unknown));
        assert_eq!(key_from_event(None, Some('z' as i32)), Some(Key::Unknown));
    }

    #[test]
    fn left_click_activates_like_ok() {
        assert_eq!(key_from_mouse(MouseButton::Left as i32), Some(Key::Ok));
        assert_eq!(key_from_mouse(MouseButton::Right as i32), None);
        assert_eq!(key_from_mouse(MouseButton::Middle as i32), None);
        assert_eq!(key_from_mouse(0), None);
    }

    #[test]
    fn raw_event_log_budget_and_format() {
        let mut log = RawEventLog::new();
        for i in 0..RawEventLog::LIMIT {
            let line = log.record("keydown", &[82, 1073741906]).unwrap();
            assert!(line.contains("keydown"), "{line}");
            assert!(line.contains("codes=[82,1073741906]"), "{line}");
            assert!(line.contains(&format!("{}/{}", i + 1, RawEventLog::LIMIT)), "{line}");
        }
        assert_eq!(log.record("mousedown", &[1, 1]), None);
    }

    #[test]
    fn event_filter_accepts_sdl2_types() {
        for t in [
            0x100, 0x107, 0x150, 0x200, 0x201, 0x300, 0x301, 0x305, 0x400, 0x401, 0x403,
            0x600, 0x605, 0x650, 0x65b, 0x700, 0x702, 0x800, 0x802, 0x900, 0x1000, 0x1003,
            0x1100, 0x1101, 0x1200, 0x2000, 0x2001,
        ] {
            assert!(is_decodable_event_type(t), "0x{t:x} should pass");
        }
    }

    #[test]
    fn event_filter_drops_foreign_types() {
        // SDL3-native numbers leaked by sdl2-compat (0x207 observed on host)
        for t in [0x151, 0x157, 0x202, 0x207, 0x20f, 0x8001, 0xffff, 0x7f00] {
            assert!(!is_decodable_event_type(t), "0x{t:x} should be dropped");
        }
    }
}
