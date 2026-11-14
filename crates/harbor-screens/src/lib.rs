pub mod browser;
pub mod catalog;
pub mod discovery;
pub mod home;
pub mod nav;
pub mod player;
pub mod settings;

use harbor_platform::{t, Lang};
use harbor_ui::draw::Surface;
use harbor_ui::glass::card;
use harbor_ui::label::Label;
use harbor_ui::Focus;
use harbor_ui_kit::{AddSource, HudState};

pub use catalog::{build_catalog, refresh_catalog, Catalog};
pub use discovery::poll_discovery;
pub use nav::{handle, persist, tick_add, View};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Home,
    Browser,
    Settings,
    Player,
}

pub fn rows(screen: Screen, catalog: &Catalog) -> Vec<usize> {
    match screen {
        Screen::Home => home::rows(catalog),
        Screen::Browser => browser::rows(catalog),
        Screen::Settings => settings::rows(catalog),
        Screen::Player => vec![1],
    }
}

pub fn draw(
    s: &mut impl Surface,
    screen: Screen,
    catalog: &Catalog,
    focus: Focus,
    hud: Option<&HudState>,
    add: Option<&AddSource>,
) {
    draw_lang(s, screen, catalog, focus, hud, add, Lang::En);
}

pub fn draw_lang(
    s: &mut impl Surface,
    screen: Screen,
    catalog: &Catalog,
    focus: Focus,
    hud: Option<&HudState>,
    add: Option<&AddSource>,
    lang: Lang,
) {
    if screen != Screen::Player {
        chrome(s, screen, lang);
    }
    match screen {
        Screen::Home => home::draw_lang(s, catalog, focus, lang),
        Screen::Browser => browser::draw_lang(s, catalog, focus, lang),
        Screen::Settings => settings::draw_lang(s, catalog, focus, lang),
        Screen::Player => player::draw_lang(s, hud.unwrap_or(&HudState::playing("Player")), lang),
    }
    if let Some(add) = add {
        harbor_ui_kit::draw_add_lang(s, add, lang);
    }
}

fn chrome(s: &mut impl Surface, screen: Screen, lang: Lang) {
    let names = [t(lang, "home"), t(lang, "browser"), t(lang, "settings")];
    for (i, name) in names.iter().enumerate() {
        let x = 1480 + i as i32 * 140;
        let active = matches!(
            (screen, i),
            (Screen::Home, 0) | (Screen::Browser, 1) | (Screen::Settings, 2)
        );
        card(s, x, 28, 128, 48, active);
        Label::new(x + 16, 40, name).caption().draw(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;

    #[test]
    fn demo_screens_paint_distinct_titles() {
        let cat = Catalog::fixture();
        for (screen, title) in [
            (Screen::Home, "Harbor"),
            (Screen::Browser, "Browser"),
            (Screen::Settings, "Settings"),
        ] {
            let mut r = Recorder::default();
            draw(&mut r, screen, &cat, Focus::default(), None, None);
            let texts: Vec<_> = r
                .ops
                .iter()
                .filter_map(|op| match op {
                    harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            assert!(texts.contains(&title), "{screen:?} missing {title}");
            assert!(texts.contains(&"Home"));
            assert!(texts.contains(&"Browser"));
            assert!(texts.contains(&"Settings"));
        }
    }

    #[test]
    fn zh_hans_chrome_and_settings() {
        let cat = Catalog::fixture();
        let mut r = Recorder::default();
        draw_lang(
            &mut r,
            Screen::Settings,
            &cat,
            Focus { row: 2, col: 0 },
            None,
            None,
            Lang::ZhHans,
        );
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"首页"));
        assert!(texts.contains(&"浏览"));
        assert!(texts.contains(&"设置"));
        assert!(texts.contains(&"添加来源"));
    }

    #[test]
    fn content_text_meets_caption_floor() {
        let cat = Catalog::fixture();
        let screens = [Screen::Home, Screen::Browser, Screen::Settings];
        for screen in screens {
            let mut r = Recorder::default();
            draw(&mut r, screen, &cat, Focus::default(), None, None);
            for px in r.text_sizes() {
                assert!(
                    px >= harbor_ui::size::CAPTION,
                    "{screen:?} painted {px}px below caption floor"
                );
            }
        }
        for (screen, hud) in [
            (Screen::Home, None),
            (Screen::Player, Some(HudState::playing("clip"))),
        ] {
            let mut r = Recorder::default();
            let cat_ref = if screen == Screen::Home {
                Catalog::empty()
            } else {
                cat.clone()
            };
            draw(&mut r, screen, &cat_ref, Focus::default(), hud.as_ref(), None);
            for px in r.text_sizes() {
                assert!(px >= harbor_ui::size::CAPTION);
            }
        }
    }
}
