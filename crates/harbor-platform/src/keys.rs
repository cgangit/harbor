#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Ok,
    Back,
    Play,
    Pause,
    Stop,
    Rewind,
    FastForward,
    Red,
    Green,
    Yellow,
    Blue,
    Unknown,
}

pub fn from_sdl_scancode(scancode: i32) -> Key {
    match scancode {
        80 => Key::Left,
        79 => Key::Right,
        82 => Key::Up,
        81 => Key::Down,
        40 | 44 => Key::Ok,
        41 | 42 => Key::Back,
        19 => Key::Play,
        48 => Key::Pause,
        22 => Key::Stop,
        21 => Key::Rewind,
        9 => Key::FastForward,
        10 => Key::Green,
        28 => Key::Yellow,
        _ => Key::Unknown,
    }
}

/// SDL keycodes (`SDLK_*`): printable keys carry their ASCII value, all others
/// are `SDL_SCANCODE_TO_KEYCODE(scancode)` = `scancode | (1 << 30)`.
/// Some platforms (notably webOS) deliver `KeyDown` with only `keycode` set,
/// so this path exists alongside `from_sdl_scancode`.
pub fn from_sdl_keycode(keycode: i32) -> Key {
    const MASK: i32 = 1 << 30;
    const KP_ENTER: i32 = 88 | MASK;
    const AC_BACK: i32 = 270 | MASK;
    const LEFT: i32 = 80 | MASK;
    const RIGHT: i32 = 79 | MASK;
    const UP: i32 = 82 | MASK;
    const DOWN: i32 = 81 | MASK;
    const AUDIOPLAY: i32 = 261 | MASK;
    const AUDIOSTOP: i32 = 260 | MASK;
    const AUDIOREWIND: i32 = 285 | MASK;
    const AUDIOFASTFORWARD: i32 = 286 | MASK;
    match keycode {
        // SDLK_RETURN '\r', SDLK_SPACE, SDLK_KP_ENTER
        13 | 32 | KP_ENTER => Key::Ok,
        // SDLK_ESCAPE '\x1B', SDLK_BACKSPACE '\b', SDLK_AC_BACK
        27 | 8 | AC_BACK => Key::Back,
        LEFT => Key::Left,
        RIGHT => Key::Right,
        UP => Key::Up,
        DOWN => Key::Down,
        // SDLK_AUDIOPLAY / SDLK_AUDIOSTOP / SDLK_AUDIOREWIND / SDLK_AUDIOFASTFORWARD
        AUDIOPLAY => Key::Play,
        AUDIOSTOP => Key::Stop,
        AUDIOREWIND => Key::Rewind,
        AUDIOFASTFORWARD => Key::FastForward,
        _ => Key::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_scancodes() {
        assert_eq!(from_sdl_scancode(80), Key::Left);
        assert_eq!(from_sdl_scancode(79), Key::Right);
        assert_eq!(from_sdl_scancode(82), Key::Up);
        assert_eq!(from_sdl_scancode(81), Key::Down);
    }

    #[test]
    fn enter_and_escape() {
        assert_eq!(from_sdl_scancode(40), Key::Ok);
        assert_eq!(from_sdl_scancode(41), Key::Back);
    }

    #[test]
    fn color_keys_from_g_y() {
        assert_eq!(from_sdl_scancode(10), Key::Green);
        assert_eq!(from_sdl_scancode(28), Key::Yellow);
    }

    #[test]
    fn unknown_is_stable() {
        assert_eq!(from_sdl_scancode(0), Key::Unknown);
        assert_eq!(from_sdl_scancode(-1), Key::Unknown);
    }

    #[test]
    fn keycode_enter_and_escape() {
        const MASK: i32 = 1 << 30;
        assert_eq!(from_sdl_keycode(13), Key::Ok); // SDLK_RETURN
        assert_eq!(from_sdl_keycode(32), Key::Ok); // SDLK_SPACE
        assert_eq!(from_sdl_keycode(88 | MASK), Key::Ok); // SDLK_KP_ENTER
        assert_eq!(from_sdl_keycode(27), Key::Back); // SDLK_ESCAPE
        assert_eq!(from_sdl_keycode(8), Key::Back); // SDLK_BACKSPACE
        assert_eq!(from_sdl_keycode(270 | MASK), Key::Back); // SDLK_AC_BACK
    }

    #[test]
    fn keycode_arrows() {
        const MASK: i32 = 1 << 30;
        assert_eq!(from_sdl_keycode(80 | MASK), Key::Left);
        assert_eq!(from_sdl_keycode(79 | MASK), Key::Right);
        assert_eq!(from_sdl_keycode(82 | MASK), Key::Up);
        assert_eq!(from_sdl_keycode(81 | MASK), Key::Down);
    }

    #[test]
    fn keycode_media_keys() {
        const MASK: i32 = 1 << 30;
        assert_eq!(from_sdl_keycode(261 | MASK), Key::Play); // SDLK_AUDIOPLAY
        assert_eq!(from_sdl_keycode(260 | MASK), Key::Stop); // SDLK_AUDIOSTOP
        assert_eq!(from_sdl_keycode(285 | MASK), Key::Rewind);
        assert_eq!(from_sdl_keycode(286 | MASK), Key::FastForward);
    }

    #[test]
    fn keycode_unknown_is_stable() {
        assert_eq!(from_sdl_keycode(0), Key::Unknown);
        assert_eq!(from_sdl_keycode('z' as i32), Key::Unknown);
        assert_eq!(from_sdl_keycode(-1), Key::Unknown);
    }

    #[test]
    fn keycode_and_scancode_paths_agree_on_shared_keys() {
        const MASK: i32 = 1 << 30;
        for (sc, kc) in [(80, 80 | MASK), (79, 79 | MASK), (82, 82 | MASK), (81, 81 | MASK)] {
            assert_eq!(from_sdl_scancode(sc), from_sdl_keycode(kc));
        }
        assert_eq!(from_sdl_scancode(40), from_sdl_keycode(13));
        assert_eq!(from_sdl_scancode(41), from_sdl_keycode(27));
    }
}
