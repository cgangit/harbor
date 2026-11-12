use crate::draw::Surface;
use crate::theme::{role, size, Color};

pub struct Label<'a> {
    pub x: i32,
    pub y: i32,
    pub text: &'a str,
    pub px: u16,
    pub color: Color,
}

impl<'a> Label<'a> {
    pub fn new(x: i32, y: i32, text: &'a str) -> Self {
        Self {
            x,
            y,
            text,
            px: size::BODY,
            color: role::TEXT_PRIMARY,
        }
    }

    pub fn caption(mut self) -> Self {
        self.px = size::CAPTION;
        self
    }

    pub fn headline(mut self) -> Self {
        self.px = size::HEADLINE;
        self
    }

    pub fn muted(mut self) -> Self {
        self.color = role::TEXT_MUTED;
        self
    }

    pub fn draw(&self, s: &mut impl Surface) {
        s.text(self.x, self.y, self.text, self.px, self.color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Recorder;

    #[test]
    fn caption_uses_floor() {
        let mut r = Recorder::default();
        Label::new(0, 0, "Continue").caption().draw(&mut r);
        assert_eq!(r.text_sizes(), vec![size::CAPTION]);
        assert!(size::CAPTION >= 24);
    }
}
