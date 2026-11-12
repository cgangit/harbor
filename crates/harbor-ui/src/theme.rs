pub use harbor_gfx::tokens::{role as gfx_role, size, Color, SCR_H, SCR_W, rgb};

pub mod role {
    use super::{rgb, Color};

    pub const BG: Color = rgb(10, 12, 18);
    pub const SURFACE: Color = rgb(18, 22, 32);
    pub const TEXT_PRIMARY: Color = rgb(245, 247, 250);
    pub const TEXT_MUTED: Color = rgb(168, 178, 196);
    pub const ACCENT: Color = rgb(88, 166, 255);
    pub const CONTROL_IDLE_FILL: Color = rgb(32, 38, 52);
    pub const FOCUS_RING: Color = rgb(120, 190, 255);
    pub const CARD: Color = rgb(28, 34, 48);
    pub const GLASS: Color = Color {
        r: 24,
        g: 30,
        b: 44,
        a: 210,
    };
    pub const SCRIM: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 77,
    };
}

pub const GLASS_PAD: i32 = 24;
pub const SHELF_CARD_W: u32 = 280;
pub const SHELF_CARD_H: u32 = 158;
pub const SHELF_GAP: i32 = 20;
pub const FOCUS_RING_PX: u32 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_floor_is_couch_safe() {
        assert!(size::CAPTION >= 24);
        assert!(size::BODY >= size::CAPTION);
        assert!(size::MICRO < size::CAPTION);
    }

    #[test]
    fn glass_is_translucent() {
        assert!(role::GLASS.a < 255);
        assert!(role::GLASS.a > 0);
        assert_eq!(role::SCRIM.a, 77);
    }
}
