use harbor_platform::{t, Lang};
use harbor_ui::draw::Surface;
use harbor_ui::glass::{card, panel};
use harbor_ui::label::Label;
use harbor_ui::shelf::{empty_state_hint, Shelf};
use harbor_ui::theme::{size, SHELF_CARD_H, SHELF_CARD_W};
use harbor_ui::Focus;

use crate::catalog::Catalog;

pub fn draw(s: &mut impl Surface, catalog: &Catalog, focus: Focus) {
    draw_lang(s, catalog, focus, Lang::En);
}

pub fn draw_lang(s: &mut impl Surface, catalog: &Catalog, focus: Focus, lang: Lang) {
    Label::new(80, 36, "Harbor").headline().draw(s);
    if catalog.is_empty() {
        empty_state_hint(
            s,
            600,
            400,
            t(lang, "no_sources"),
            t(lang, "ok_add_source"),
        );
        return;
    }
    hero(s, catalog, focus.row == 0, lang);
    let cont_y = 430;
    Shelf {
        x: 80,
        y: cont_y,
        title: t(lang, "continue_watching"),
        items: &catalog.continue_watching,
        focus_col: (focus.row == 1).then_some(focus.col),
    }
    .draw(s);
    let recent_y = cont_y + size::HEADLINE as i32 + SHELF_CARD_H as i32 + 48;
    Shelf {
        x: 80,
        y: recent_y,
        title: t(lang, "recently_added"),
        items: &catalog.recently_added,
        focus_col: (focus.row == 2).then_some(focus.col),
    }
    .draw(s);
    let src_y = recent_y + size::HEADLINE as i32 + SHELF_CARD_H as i32 + 48;
    Shelf {
        x: 80,
        y: src_y,
        title: t(lang, "sources"),
        items: &catalog.sources,
        focus_col: (focus.row == 3).then_some(focus.col),
    }
    .draw(s);
}

fn hero(s: &mut impl Surface, catalog: &Catalog, focused: bool, lang: Lang) {
    let title = catalog
        .continue_watching
        .first()
        .map(String::as_str)
        .unwrap_or("Harbor");
    panel(s, 80, 96, 1760, 300);
    card(s, 110, 126, SHELF_CARD_W + 80, 240, focused);
    Label::new(430, 160, title).headline().draw(s);
    Label::new(430, 214, t(lang, "resume_from"))
        .caption()
        .muted()
        .draw(s);
}

pub fn rows(catalog: &Catalog) -> Vec<usize> {
    if catalog.is_empty() {
        return vec![1];
    }
    vec![
        1,
        catalog.continue_watching.len(),
        catalog.recently_added.len(),
        catalog.sources.len(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;

    #[test]
    fn demo_home_paints_shelves() {
        let mut r = Recorder::default();
        draw(&mut r, &Catalog::fixture(), Focus { row: 1, col: 0 });
        let texts = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Continue Watching"));
        assert!(texts.contains(&"Recently Added"));
        assert!(texts.contains(&"Sources"));
        assert!(texts.contains(&"The River"));
        assert!(texts.contains(&"USB Drive"));
    }

    #[test]
    fn empty_home_offers_add_source() {
        let mut r = Recorder::default();
        draw(&mut r, &Catalog::empty(), Focus::default());
        let texts = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"No sources yet"));
        assert!(texts.iter().any(|t| t.contains("Add Source")));
    }
}
