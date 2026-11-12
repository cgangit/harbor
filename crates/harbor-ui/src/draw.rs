use harbor_gfx::tokens::Color;
use harbor_gfx::Frame;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawOp {
    Fill {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        color: Color,
    },
    Text {
        x: i32,
        y: i32,
        text: String,
        px: u16,
        color: Color,
    },
}

pub trait Surface {
    fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color);
    fn text(&mut self, x: i32, y: i32, text: &str, px: u16, color: Color);
}

impl Surface for Frame<'_> {
    fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        Frame::fill(self, x, y, w, h, color);
    }

    fn text(&mut self, x: i32, y: i32, text: &str, px: u16, color: Color) {
        let w = (text.len() as u32).saturating_mul((px as u32) / 2).max(8);
        Frame::fill(self, x, y, w, px as u32, color);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Recorder {
    pub ops: Vec<DrawOp>,
}

impl Surface for Recorder {
    fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        self.ops.push(DrawOp::Fill {
            x,
            y,
            w,
            h,
            color,
        });
    }

    fn text(&mut self, x: i32, y: i32, text: &str, px: u16, color: Color) {
        self.ops.push(DrawOp::Text {
            x,
            y,
            text: text.to_string(),
            px,
            color,
        });
    }
}

impl Recorder {
    pub fn text_sizes(&self) -> Vec<u16> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                DrawOp::Text { px, .. } => Some(*px),
                _ => None,
            })
            .collect()
    }

    pub fn min_text_px(&self) -> Option<u16> {
        self.text_sizes().into_iter().min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::role;

    #[test]
    fn recorder_captures_fill_and_text() {
        let mut r = Recorder::default();
        r.fill(0, 0, 10, 10, role::BG);
        r.text(8, 8, "Hi", 24, role::TEXT_PRIMARY);
        assert_eq!(r.ops.len(), 2);
        assert_eq!(r.text_sizes(), vec![24]);
    }
}
