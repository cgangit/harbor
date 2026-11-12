use crate::draw::Surface;
use crate::theme::{role, FOCUS_RING_PX, GLASS_PAD};

pub fn scrim(s: &mut impl Surface, x: i32, y: i32, w: u32, h: u32) {
    s.fill(x, y, w, h, role::SCRIM);
}

pub fn panel(s: &mut impl Surface, x: i32, y: i32, w: u32, h: u32) {
    s.fill(x, y, w, h, role::GLASS);
}

pub fn card(s: &mut impl Surface, x: i32, y: i32, w: u32, h: u32, focused: bool) {
    if focused {
        s.fill(
            x - FOCUS_RING_PX as i32,
            y - FOCUS_RING_PX as i32,
            w + FOCUS_RING_PX * 2,
            h + FOCUS_RING_PX * 2,
            role::FOCUS_RING,
        );
    }
    s.fill(x, y, w, h, role::CARD);
}

pub fn padded_panel(s: &mut impl Surface, x: i32, y: i32, w: u32, h: u32) {
    panel(
        s,
        x - GLASS_PAD / 4,
        y - GLASS_PAD / 4,
        w + GLASS_PAD as u32 / 2,
        h + GLASS_PAD as u32 / 2,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{DrawOp, Recorder};

    #[test]
    fn scrim_uses_theme_alpha() {
        let mut r = Recorder::default();
        scrim(&mut r, 0, 0, 1920, 1080);
        match &r.ops[0] {
            DrawOp::Fill { color, w, h, .. } => {
                assert_eq!(*color, role::SCRIM);
                assert_eq!((*w, *h), (1920, 1080));
            }
            _ => panic!("expected fill"),
        }
    }

    #[test]
    fn focused_card_paints_ring_then_fill() {
        let mut r = Recorder::default();
        card(&mut r, 40, 40, 280, 158, true);
        assert_eq!(r.ops.len(), 2);
        match &r.ops[0] {
            DrawOp::Fill { color, .. } => assert_eq!(*color, role::FOCUS_RING),
            _ => panic!("ring"),
        }
        match &r.ops[1] {
            DrawOp::Fill { color, .. } => assert_eq!(*color, role::CARD),
            _ => panic!("card"),
        }
    }
}
