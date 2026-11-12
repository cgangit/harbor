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
}
