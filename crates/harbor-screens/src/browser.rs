use harbor_platform::{t, Lang};
use harbor_ui::draw::Surface;
use harbor_ui::glass::card;
use harbor_ui::label::Label;
use harbor_ui::theme::{SHELF_CARD_W, SHELF_GAP};
use harbor_ui::Focus;

use crate::catalog::{Catalog, AZ, SORTS};

pub fn draw(s: &mut impl Surface, catalog: &Catalog, focus: Focus) {
    draw_lang(s, catalog, focus, Lang::En);
}

pub fn draw_lang(s: &mut impl Surface, catalog: &Catalog, focus: Focus, lang: Lang) {
    Label::new(80, 36, t(lang, "browser")).headline().draw(s);
    Label::new(80, 96, t(lang, "sort")).caption().muted().draw(s);
    let sorts = [t(lang, "name"), t(lang, "date"), t(lang, "size")];
    for (i, name) in sorts.iter().enumerate() {
        let x = 80 + i as i32 * 180;
        card(s, x, 130, 160, 56, focus.row == 0 && focus.col == i);
        Label::new(x + 16, 144, name).caption().draw(s);
    }
    Label::new(80, 210, "A-Z").caption().muted().draw(s);
    for (i, letter) in AZ.iter().enumerate() {
        let x = 80 + i as i32 * 68;
        card(s, x, 244, 52, 52, focus.row == 1 && focus.col == i);
        Label::new(x + 14, 256, letter).caption().draw(s);
    }
    Label::new(80, 320, t(lang, "library")).headline().draw(s);
    for (i, name) in catalog.files.iter().enumerate() {
        let y = 370 + i as i32 * (56 + SHELF_GAP / 2);
        card(
            s,
            80,
            y,
            SHELF_CARD_W * 2,
            56,
            focus.row == 2 && focus.col == i,
        );
        Label::new(100, y + 14, name).caption().draw(s);
    }
}

pub fn rows(catalog: &Catalog) -> Vec<usize> {
    vec![SORTS.len(), AZ.len(), catalog.files.len().max(1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;

    #[test]
    fn browser_paints_sort_az_and_files() {
        let mut r = Recorder::default();
        draw(&mut r, &Catalog::demo(), Focus { row: 2, col: 0 });
        let texts = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Name"));
        assert!(texts.contains(&"Date"));
        assert!(texts.contains(&"A"));
        assert!(texts.contains(&"Z"));
        assert!(texts.contains(&"the-river.mkv"));
    }
}
