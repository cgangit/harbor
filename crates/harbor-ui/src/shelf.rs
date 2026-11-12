use crate::draw::Surface;
use crate::glass::card;
use crate::label::Label;
use crate::theme::{size, SHELF_CARD_H, SHELF_CARD_W, SHELF_GAP};

pub struct Shelf<'a> {
    pub x: i32,
    pub y: i32,
    pub title: &'a str,
    pub items: &'a [String],
    pub focus_col: Option<usize>,
}

impl<'a> Shelf<'a> {
    pub fn draw(&self, s: &mut impl Surface) {
        Label::new(self.x, self.y, self.title)
            .headline()
            .draw(s);
        let card_y = self.y + size::HEADLINE as i32 + 16;
        for (i, item) in self.items.iter().enumerate() {
            let x = self.x + i as i32 * (SHELF_CARD_W as i32 + SHELF_GAP);
            let focused = self.focus_col == Some(i);
            card(s, x, card_y, SHELF_CARD_W, SHELF_CARD_H, focused);
            Label::new(x + 12, card_y + SHELF_CARD_H as i32 - 36, item)
                .caption()
                .draw(s);
        }
    }
}

pub fn empty_state(s: &mut impl Surface, x: i32, y: i32, message: &str) {
    empty_state_hint(s, x, y, message, "OK — Add Source");
}

pub fn empty_state_hint(s: &mut impl Surface, x: i32, y: i32, message: &str, hint: &str) {
    crate::glass::panel(s, x, y, 720, 200);
    Label::new(x + 32, y + 48, message)
        .headline()
        .draw(s);
    Label::new(x + 32, y + 110, hint)
        .caption()
        .muted()
        .draw(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Recorder;
    use crate::focus::{reduce, Focus};
    use harbor_platform::Key;

    #[test]
    fn draws_title_and_captions() {
        let items = ["Alpha".into(), "Beta".into()];
        let mut r = Recorder::default();
        Shelf {
            x: 80,
            y: 200,
            title: "Continue Watching",
            items: &items,
            focus_col: Some(0),
        }
        .draw(&mut r);
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                crate::draw::DrawOp::Text { text, px, .. } => Some((text.as_str(), *px)),
                _ => None,
            })
            .collect();
        assert_eq!(texts[0], ("Continue Watching", size::HEADLINE));
        assert!(texts.iter().any(|(t, px)| *t == "Alpha" && *px == size::CAPTION));
        assert!(texts.iter().any(|(t, px)| *t == "Beta" && *px == size::CAPTION));
    }

    #[test]
    fn empty_home_focus_then_wrap_on_populated_shelf() {
        let empty: &[usize] = &[];
        assert_eq!(
            reduce(Focus::default(), empty, Key::Right),
            Focus { row: 0, col: 0 }
        );
        let rows = &[3];
        let mut f = Focus::default();
        f = reduce(f, rows, Key::Right);
        f = reduce(f, rows, Key::Right);
        f = reduce(f, rows, Key::Right);
        assert_eq!(f.col, 0);
    }
}
