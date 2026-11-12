pub const SCR_W: u32 = 1920;
pub const SCR_H: u32 = 1080;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub mod size {
    pub const HERO: u16 = 72;
    pub const DISPLAY: u16 = 48;
    pub const TITLE: u16 = 40;
    pub const HEADLINE: u16 = 32;
    pub const BODY: u16 = 28;
    pub const LABEL: u16 = 26;
    pub const CAPTION: u16 = 24;
    pub const MICRO: u16 = 22;
    pub const DIAGNOSTIC: u16 = 20;
}

pub mod role {
    use super::{rgb, Color};

    pub const BG: Color = rgb(10, 12, 18);
    pub const TEXT_PRIMARY: Color = rgb(245, 247, 250);
    pub const ACCENT: Color = rgb(88, 166, 255);
    pub const CONTROL_IDLE_FILL: Color = rgb(32, 38, 52);
    pub const SCRIM: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 77,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_is_1080p() {
        assert_eq!(SCR_W, 1920);
        assert_eq!(SCR_H, 1080);
    }

    #[test]
    fn caption_floor() {
        assert!(size::CAPTION >= 24);
        assert!(size::MICRO < size::CAPTION);
        assert!(size::HERO > size::DISPLAY);
    }
}
