use std::collections::HashMap;
use std::hash::Hash;

use sdl2::event::Event;
use sdl2::keyboard::Scancode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color as SdlColor;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::surface::Surface;
use sdl2::video::Window as SdlWindow;
use sdl2::video::WindowContext;
use sdl2::EventPump;
use sdl2::Sdl;

use crate::text;
use crate::tokens::{role, Color, SCR_H, SCR_W};
use harbor_platform::Key;

/// How many distinct `(text, px, color)` textures may live before the whole
/// map is dropped (design D3): labels are stable frame to frame, so a
/// clear-all never thrashes in practice, while dynamic strings (the HUD
/// clock) cannot grow the cache without bound.
const TEXT_CACHE_CAP: usize = 512;

pub struct Window {
    _sdl: Sdl,
    /// Declared before `canvas` on purpose: Rust drops fields in declaration
    /// order, and the cached text textures must be destroyed while their
    /// renderer (owned by `canvas`) is still alive.
    text: TextState,
    canvas: Canvas<SdlWindow>,
    events: EventPump,
    raw: RawEventLog,
}

/// Everything `Frame::draw_text` needs: the rasterizer, its texture cache,
/// and the creator that owns the textures.
struct TextState {
    creator: TextureCreator<WindowContext>,
    renderer: Option<text::Renderer>,
    textures: BoundedCache<TextKey, (Texture, u32, u32)>,
}

/// Cache key for one rendered label: the string, its size, and its colour.
type TextKey = (String, u16, [u8; 4]);

/// A `HashMap` with a whole-map eviction policy: once `cap` entries are in,
/// the next insert clears the map first (design D3, "clear-all overflow").
pub struct BoundedCache<K, V> {
    map: HashMap<K, V>,
    cap: usize,
}

impl<K: Eq + Hash, V> BoundedCache<K, V> {
    pub fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            cap,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.map.len() >= self.cap {
            self.map.clear();
        }
        self.map.insert(key, value);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

/// Logs the first [`RawEventLog::LIMIT`] key/button events — plus a short
/// [`RawEventLog::MOTION_LIMIT`] tail of pointer motion — to the app log so
/// that codes reported by an unverified device (webOS remote keycodes, Magic
/// Remote pointer events) are visible without a debugger.
#[derive(Default)]
pub struct RawEventLog {
    seen: usize,
    motion_seen: usize,
}

impl RawEventLog {
    pub const LIMIT: usize = 20;
    /// Pointer motion gets its own small budget: an air mouse floods the
    /// queue (device log: `20/20` motion lines in one run, which crowded out
    /// the Back press we were waiting to capture), and a handful of
    /// coordinates is enough to prove the pointer maps onto 1920×1080.
    pub const MOTION_LIMIT: usize = 8;

    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the line for this event while under the class's budget,
    /// `None` after. Motion counts separately so it can never exhaust the
    /// key/button budget.
    pub fn record(&mut self, class: &str, codes: &[i64]) -> Option<String> {
        let (count, limit) = if class == "motion" {
            (&mut self.motion_seen, Self::MOTION_LIMIT)
        } else {
            (&mut self.seen, Self::LIMIT)
        };
        if *count >= limit {
            return None;
        }
        *count += 1;
        let list = codes
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "evt {class} codes=[{list}] {count}/{limit}",
            count = *count
        ))
    }
}

/// Map a `KeyDown` event onto `Key`. Prefers the scancode (host keyboards),
/// falls back to the keycode — some platforms, notably webOS, deliver only
/// `keycode`. Returns `None` only when the event carries neither.
///
/// **Device fact (webOS 4.x TV, read off `events.log`):** the keycode is the
/// raw *scancode number*, not an `SDLK_*` value — `codes=[-1,81]` means
/// scancode absent, keycode `81` = `SDL_SCANCODE_DOWN`, whereas SDL's
/// contract would be `81 | (1 << 30)`. The third candidate translates
/// `keycode` through the scancode table, consulted only when no real
/// scancode was delivered and both canonical readings failed; a host event
/// that carries a scancode can never reach it, so its ASCII keycodes
/// (e.g. TAB = 9 = `SDLK_*`… but scancode 9 = `F`) are not misread.
pub fn key_from_event(scancode: Option<i32>, keycode: Option<i32>) -> Option<Key> {
    if scancode.is_none() && keycode.is_none() {
        return None;
    }
    let from_sc = scancode.map(harbor_platform::from_sdl_scancode);
    let from_kc = keycode.map(harbor_platform::from_sdl_keycode);
    // webOS shape only: with no scancode present, the raw keycode number may
    // *be* the scancode (81 → DOWN). A host event always carries a scancode,
    // so its ASCII keycodes are never reinterpreted through this table.
    let from_kc_as_sc = keycode
        .filter(|_| scancode.is_none())
        .map(harbor_platform::from_sdl_scancode);
    Some(
        [from_sc, from_kc, from_kc_as_sc]
            .into_iter()
            .flatten()
            .find(|k| *k != Key::Unknown)
            .unwrap_or(Key::Unknown),
    )
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
    text: &'a mut TextState,
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
        canvas.set_blend_mode(BlendMode::Blend);
        let events = sdl.event_pump()?;
        let creator = canvas.texture_creator();
        let renderer = text::Renderer::load();
        if renderer.is_none() {
            // Design D4: the build artifact is corrupt - keep the app alive
            // on placeholder bars and leave the diagnosis in the log.
            harbor_base::log("text: embedded font failed to parse; labels render as bars");
        }
        Ok(Self {
            _sdl: sdl,
            text: TextState {
                creator,
                renderer,
                textures: BoundedCache::new(TEXT_CACHE_CAP),
            },
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
            text: &mut self.text,
        }
    }
}

impl Frame<'_> {
    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, c: Color) {
        self.canvas
            .set_draw_color(SdlColor::RGBA(c.r, c.g, c.b, c.a));
        let _ = self.canvas.fill_rect(Rect::new(x, y, w, h));
    }

    /// Draw `text` as glyphs at `(x, y)` with the line box the placeholder
    /// bar used (design D3). Returns `false` when no text can be drawn —
    /// font unavailable or texture upload failed — so the caller keeps the
    /// bar fallback (design D4). Rendered strings are cached as textures by
    /// `(text, px, colour)`; a miss rasterizes once and reuses the texture
    /// every frame after that.
    pub fn draw_text(&mut self, x: i32, y: i32, text: &str, px: u16, color: Color) -> bool {
        let key: TextKey = (text.to_string(), px, [color.r, color.g, color.b, color.a]);
        if let Some((texture, w, h)) = self.text.textures.get(&key) {
            let dst = Rect::new(x, y, *w, *h);
            let _ = self.canvas.copy(texture, None, Some(dst));
            return true;
        }
        let Some(renderer) = self.text.renderer.as_mut() else {
            return false;
        };
        let rendered = renderer.render_to_buffer(text, px, color);
        let (w, h) = (rendered.width as u32, rendered.height as u32);
        let texture = match make_text_texture(&self.text.creator, &rendered) {
            Ok(texture) => texture,
            Err(err) => {
                // Pathological input (e.g. a multi-thousand-pixel name):
                // fall back to the bar rather than dropping the label.
                harbor_base::log(&format!("text: texture upload failed: {err}"));
                return false;
            }
        };
        let dst = Rect::new(x, y, w, h);
        let _ = self.canvas.copy(&texture, None, Some(dst));
        self.text.textures.insert(key, (texture, w, h));
        true
    }

    pub fn present(self) {
        self.canvas.present();
    }
}

/// Upload one rendered label as an RGBA texture with blending on. The pixel
/// buffer is borrowed by the surface only until SDL has copied it.
fn make_text_texture(
    creator: &TextureCreator<WindowContext>,
    rendered: &text::RenderedText,
) -> Result<Texture, String> {
    let mut pixels = rendered.pixels.clone();
    let surface = Surface::from_data(
        &mut pixels,
        rendered.width as u32,
        rendered.height as u32,
        (rendered.width * 4) as u32,
        PixelFormatEnum::RGBA32,
    )?;
    let mut texture = creator
        .create_texture_from_surface(&surface)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(BlendMode::Blend);
    Ok(texture)
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

    /// The shape actually observed on the webOS 4.x TV, read off the device
    /// `events.log` (`evt keydown codes=[-1,81] 1/20`): no scancode, and the
    /// keycode is the raw *scancode number* — 81 = DOWN, not the SDL
    /// contract's `81 | (1 << 30)`. Before this fallback every remote key
    /// decoded to `Unknown` and the UI was stuck on one focus item.
    #[test]
    fn webos_raw_scancode_number_keycode_maps() {
        assert_eq!(key_from_event(None, Some(80)), Some(Key::Left));
        assert_eq!(key_from_event(None, Some(79)), Some(Key::Right));
        assert_eq!(key_from_event(None, Some(82)), Some(Key::Up));
        assert_eq!(key_from_event(None, Some(81)), Some(Key::Down));
        assert_eq!(key_from_event(None, Some(40)), Some(Key::Ok));
        // plausible Back codes (ESCAPE scancode, or AC_BACK's 270)
        assert_eq!(key_from_event(None, Some(41)), Some(Key::Back));
        // masked SDLK_* input still resolves canonically first
        assert_eq!(key_from_event(None, Some(82 | MASK)), Some(Key::Up));
    }

    /// The scancode-number reinterpretation is gated on the scancode field
    /// being absent: a host event that carries a real scancode keeps its
    /// ASCII keycode meaning (keycode 9 = TAB must not become FastForward
    /// via scancode 9 = `F`; keycode 40 = `(` must not become OK).
    #[test]
    fn raw_keycode_fallback_never_hijacks_a_real_scancode() {
        assert_eq!(key_from_event(Some(43), Some(9)), Some(Key::Unknown));
        assert_eq!(key_from_event(Some(30), Some(40)), Some(Key::Unknown));
        assert_eq!(key_from_event(Some(80), Some(80 | MASK)), Some(Key::Left));
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

    /// Regression from the device log: a Magic Remote's motion events filled
    /// the shared budget (`20/20` motion lines) and the Back press we needed
    /// to see was never recorded. Motion now has its own small budget and
    /// cannot crowd out keys or buttons.
    #[test]
    fn motion_budget_cannot_crowd_out_keys() {
        let mut log = RawEventLog::new();
        for i in 0..RawEventLog::MOTION_LIMIT {
            let line = log.record("motion", &[100 + i as i64, 200]).unwrap();
            assert!(
                line.contains(&format!("{}/{}", i + 1, RawEventLog::MOTION_LIMIT)),
                "{line}"
            );
        }
        assert_eq!(log.record("motion", &[9, 9]), None, "motion budget spent");
        // keys still have their full budget afterwards
        let line = log.record("keydown", &[-1, 41]).unwrap();
        assert!(line.contains("keydown"), "{line}");
        assert!(line.contains("1/20"), "{line}");
        // buttons too
        assert!(
            log.record("mousedown", &[1, 1])
                .unwrap()
                .contains("mousedown codes=[1,1]")
        );
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

    #[test]
    fn bounded_cache_serves_hits_and_misses() {
        let mut cache: BoundedCache<&str, u32> = BoundedCache::new(4);
        assert!(cache.get(&"x").is_none());
        cache.insert("x", 7);
        assert_eq!(cache.get(&"x"), Some(&7));
        assert_eq!(cache.len(), 1);
    }

    /// Design D3's eviction policy: at capacity the next insert clears the
    /// whole map first, so the cache can never exceed `cap` entries and
    /// dynamic strings (the HUD clock) cannot grow it without bound.
    #[test]
    fn bounded_cache_clears_all_at_capacity() {
        let mut cache: BoundedCache<&str, u32> = BoundedCache::new(3);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        assert_eq!(cache.len(), 3, "cap entries fit");
        cache.insert("d", 4);
        assert_eq!(cache.len(), 1, "overflow drops everything but the new entry");
        assert_eq!(cache.get(&"d"), Some(&4));
        assert!(cache.get(&"a").is_none());
        assert!(cache.get(&"b").is_none());
        assert!(cache.get(&"c").is_none());
    }
}
