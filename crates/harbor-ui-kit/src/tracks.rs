use harbor_media::subs::{self, Cue};
use harbor_platform::{t, Lang};
use harbor_ui::draw::Surface;
use harbor_ui::glass::{card, panel, scrim};
use harbor_ui::label::Label;
use harbor_ui::Key;
use harbor_ui::{SCR_H, SCR_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    Audio,
    Subtitle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub id: usize,
    pub kind: TrackKind,
    pub name: String,
    pub lang: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackMenu {
    pub kind: TrackKind,
    pub index: usize,
}

pub fn demo_audio() -> Vec<Track> {
    vec![
        Track {
            id: 0,
            kind: TrackKind::Audio,
            name: "English".into(),
            lang: "en".into(),
        },
        Track {
            id: 1,
            kind: TrackKind::Audio,
            name: "Japanese".into(),
            lang: "ja".into(),
        },
    ]
}

pub fn demo_subs() -> Vec<Track> {
    vec![
        Track {
            id: 0,
            kind: TrackKind::Subtitle,
            name: "Off".into(),
            lang: "off".into(),
        },
        Track {
            id: 1,
            kind: TrackKind::Subtitle,
            name: "English".into(),
            lang: "en".into(),
        },
        Track {
            id: 2,
            kind: TrackKind::Subtitle,
            name: "Simplified Chinese".into(),
            lang: "zh-Hans".into(),
        },
    ]
}

pub fn demo_cues() -> Vec<Cue> {
    subs::parse(subs::SAMPLE_SRT)
}

pub fn apply_menu(menu: &mut TrackMenu, key: Key, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match key {
        Key::Up | Key::Left => {
            menu.index = (menu.index + len - 1) % len;
            None
        }
        Key::Down | Key::Right => {
            menu.index = (menu.index + 1) % len;
            None
        }
        Key::Ok => Some(menu.index),
        Key::Back => None,
        _ => None,
    }
}

pub fn draw_menu(s: &mut impl Surface, menu: &TrackMenu, tracks: &[Track], selected: usize) {
    draw_menu_lang(s, menu, tracks, selected, Lang::En);
}

pub fn draw_menu_lang(
    s: &mut impl Surface,
    menu: &TrackMenu,
    tracks: &[Track],
    selected: usize,
    lang: Lang,
) {
    scrim(s, 0, 0, SCR_W, SCR_H);
    let title = match menu.kind {
        TrackKind::Audio => t(lang, "audio"),
        TrackKind::Subtitle => t(lang, "subtitles"),
    };
    panel(s, 560, 200, 800, 640);
    Label::new(600, 240, title).headline().draw(s);
    for (i, t) in tracks.iter().enumerate() {
        let y = 320 + i as i32 * 80;
        card(s, 600, y, 720, 64, menu.index == i);
        let mark = if i == selected { "* " } else { "  " };
        let line = format!("{mark}{}", t.name);
        Label::new(624, y + 16, &line).caption().draw(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_and_select() {
        let mut m = TrackMenu {
            kind: TrackKind::Subtitle,
            index: 0,
        };
        apply_menu(&mut m, Key::Down, 3);
        assert_eq!(m.index, 1);
        apply_menu(&mut m, Key::Up, 3);
        assert_eq!(m.index, 0);
        apply_menu(&mut m, Key::Up, 3);
        assert_eq!(m.index, 2);
        assert_eq!(apply_menu(&mut m, Key::Ok, 3), Some(2));
    }
}
