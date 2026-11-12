use harbor_platform::{t, Lang};
use harbor_ui::draw::Surface;
use harbor_ui::glass::card;
use harbor_ui::label::Label;
use harbor_ui::shelf::empty_state_hint;
use harbor_ui::theme::{SHELF_CARD_W, SHELF_GAP};
use harbor_ui::Focus;

use crate::catalog::Catalog;

pub fn draw(s: &mut impl Surface, catalog: &Catalog, focus: Focus) {
    draw_lang(s, catalog, focus, Lang::En);
}

pub fn draw_lang(s: &mut impl Surface, catalog: &Catalog, focus: Focus, lang: Lang) {
    Label::new(80, 36, t(lang, "settings")).headline().draw(s);
    Label::new(80, 96, t(lang, "sources")).headline().draw(s);
    if catalog.sources.is_empty() {
        empty_state_hint(
            s,
            80,
            160,
            t(lang, "add_source_hint"),
            t(lang, "ok_add_source"),
        );
        lang_row(s, 380, focus.row == 1, lang);
        return;
    }
    for (i, name) in catalog.sources.iter().enumerate() {
        let y = 160 + i as i32 * (64 + SHELF_GAP / 2);
        card(
            s,
            80,
            y,
            SHELF_CARD_W * 2,
            64,
            focus.row == 0 && focus.col == i,
        );
        Label::new(104, y + 18, name).caption().draw(s);
    }
    let add_y = 160 + catalog.sources.len() as i32 * (64 + SHELF_GAP / 2) + 12;
    card(
        s,
        80,
        add_y,
        SHELF_CARD_W,
        64,
        focus.row == 1 && focus.col == 0,
    );
    Label::new(104, add_y + 18, t(lang, "add_source"))
        .caption()
        .draw(s);
    lang_row(s, add_y + 76, focus.row == 2, lang);
}

fn lang_row(s: &mut impl Surface, y: i32, focused: bool, lang: Lang) {
    card(s, 80, y, SHELF_CARD_W * 2, 64, focused);
    let value = match lang {
        Lang::En => t(lang, "lang_en"),
        Lang::ZhHans => t(lang, "lang_zh"),
    };
    let line = format!("{}: {value}", t(lang, "ui_language"));
    Label::new(104, y + 18, &line).caption().draw(s);
}

pub fn rows(catalog: &Catalog) -> Vec<usize> {
    if catalog.sources.is_empty() {
        vec![1, 1]
    } else {
        vec![catalog.sources.len(), 1, 1]
    }
}

pub fn add_row(catalog: &Catalog) -> usize {
    if catalog.sources.is_empty() {
        0
    } else {
        1
    }
}

pub fn lang_row_index(catalog: &Catalog) -> usize {
    if catalog.sources.is_empty() {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;

    #[test]
    fn settings_lists_demo_sources() {
        let mut r = Recorder::default();
        draw(&mut r, &Catalog::demo(), Focus::default());
        let texts = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"USB Drive"));
        assert!(texts.contains(&"Add Source"));
        assert!(texts.iter().any(|t| t.contains("UI language")));
        assert!(texts.iter().any(|t| t.contains("English")));
    }

    #[test]
    fn language_row_paints_zh_hans() {
        let mut r = Recorder::default();
        draw_lang(
            &mut r,
            &Catalog::demo(),
            Focus { row: 2, col: 0 },
            Lang::ZhHans,
        );
        let texts = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"设置"));
        assert!(texts.contains(&"添加来源"));
        assert!(texts.iter().any(|t| t.contains("界面语言")));
        assert!(texts.iter().any(|t| t.contains("简体中文")));
    }
}
