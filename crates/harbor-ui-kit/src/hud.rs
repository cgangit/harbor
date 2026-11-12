use harbor_media::subs::{self, Cue};
use harbor_platform::{t, Lang};
use harbor_session::Settings;
use harbor_ui::draw::Surface;
use harbor_ui::glass::{panel, scrim};
use harbor_ui::label::Label;
use harbor_ui::theme::role;
use harbor_ui::Key;
use harbor_ui::{SCR_H, SCR_W};

use crate::tracks::{self, Track, TrackKind, TrackMenu};

pub const HUD_HIDE_FRAMES: u32 = 240;
pub const PERSIST_EVERY_FRAMES: u32 = 600;
pub const FRAME_NS: i64 = 16_666_667;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContinuePrompt {
    pub position_ns: i64,
    pub choice: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HudState {
    pub paused: bool,
    pub position_ns: i64,
    pub duration_ns: i64,
    pub visible: bool,
    pub idle_frames: u32,
    pub persist_frames: u32,
    pub unavailable: bool,
    pub refuse: Option<String>,
    pub title: String,
    pub prompt: Option<ContinuePrompt>,
    pub audio: Vec<Track>,
    pub subs: Vec<Track>,
    pub audio_idx: usize,
    pub sub_idx: usize,
    pub menu: Option<TrackMenu>,
    pub cues: Vec<Cue>,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            paused: false,
            position_ns: 0,
            duration_ns: 90_000_000_000,
            visible: true,
            idle_frames: 0,
            persist_frames: 0,
            unavailable: false,
            refuse: None,
            title: String::new(),
            prompt: None,
            audio: tracks::demo_audio(),
            subs: tracks::demo_subs(),
            audio_idx: 0,
            sub_idx: 1,
            menu: None,
            cues: tracks::demo_cues(),
        }
    }
}

impl HudState {
    pub fn playing(title: &str) -> Self {
        Self {
            title: title.to_string(),
            ..Self::default()
        }
    }

    pub fn unavailable() -> Self {
        Self {
            unavailable: true,
            visible: true,
            title: String::new(),
            ..Self::default()
        }
    }

    pub fn refused(reason: &str) -> Self {
        Self {
            refuse: Some(reason.to_string()),
            visible: true,
            title: String::new(),
            ..Self::default()
        }
    }

    pub fn with_prompt(mut self, position_ns: i64) -> Self {
        self.paused = true;
        self.prompt = Some(ContinuePrompt {
            position_ns,
            choice: 0,
        });
        self
    }
}

pub fn format_ns(ns: i64) -> String {
    let total = (ns.max(0) / 1_000_000_000) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

pub fn seek_ns(position_ns: i64, duration_ns: i64, delta_s: i32) -> i64 {
    let next = position_ns + i64::from(delta_s) * 1_000_000_000;
    next.clamp(0, duration_ns.max(0))
}

pub fn apply_key(hud: &mut HudState, key: Key, settings: &Settings) -> bool {
    if hud.unavailable || hud.refuse.is_some() {
        return matches!(key, Key::Back | Key::Ok);
    }
    if hud.prompt.is_some() {
        return apply_prompt(hud, key);
    }
    if hud.menu.is_some() {
        return apply_menu_key(hud, key);
    }
    match key {
        Key::Ok | Key::Pause | Key::Play => {
            hud.paused = !hud.paused;
            show(hud);
            true
        }
        Key::Left | Key::Rewind => {
            hud.position_ns = seek_ns(hud.position_ns, hud.duration_ns, -(settings.seek_step_s as i32));
            show(hud);
            true
        }
        Key::Right | Key::FastForward => {
            hud.position_ns = seek_ns(hud.position_ns, hud.duration_ns, settings.seek_step_s as i32);
            show(hud);
            true
        }
        Key::Green => {
            hud.menu = Some(TrackMenu {
                kind: TrackKind::Audio,
                index: hud.audio_idx,
            });
            show(hud);
            true
        }
        Key::Yellow => {
            hud.menu = Some(TrackMenu {
                kind: TrackKind::Subtitle,
                index: hud.sub_idx,
            });
            show(hud);
            true
        }
        _ => false,
    }
}

fn apply_menu_key(hud: &mut HudState, key: Key) -> bool {
    if key == Key::Back {
        hud.menu = None;
        show(hud);
        return true;
    }
    let Some(menu) = hud.menu.as_mut() else {
        return false;
    };
    let (len, selected) = match menu.kind {
        TrackKind::Audio => (hud.audio.len(), hud.audio_idx),
        TrackKind::Subtitle => (hud.subs.len(), hud.sub_idx),
    };
    let _ = selected;
    if let Some(idx) = tracks::apply_menu(menu, key, len) {
        match menu.kind {
            TrackKind::Audio => hud.audio_idx = idx,
            TrackKind::Subtitle => hud.sub_idx = idx,
        }
        hud.menu = None;
        show(hud);
    }
    true
}

pub fn caption<'a>(hud: &'a HudState) -> Option<&'a str> {
    if hud.sub_idx == 0 {
        return None;
    }
    subs::active_text(&hud.cues, hud.position_ns)
}

fn apply_prompt(hud: &mut HudState, key: Key) -> bool {
    let Some(prompt) = hud.prompt.as_mut() else {
        return false;
    };
    match key {
        Key::Left | Key::Up => {
            prompt.choice = 0;
            true
        }
        Key::Right | Key::Down => {
            prompt.choice = 1;
            true
        }
        Key::Ok => {
            let start = if prompt.choice == 0 {
                prompt.position_ns
            } else {
                0
            };
            hud.position_ns = start;
            hud.paused = false;
            hud.prompt = None;
            show(hud);
            true
        }
        _ => false,
    }
}

pub fn tick(hud: &mut HudState) {
    if hud.unavailable || hud.refuse.is_some() || hud.prompt.is_some() || hud.menu.is_some() {
        return;
    }
    if !hud.paused {
        hud.position_ns = (hud.position_ns + FRAME_NS).min(hud.duration_ns);
        hud.persist_frames = hud.persist_frames.saturating_add(1);
    }
    if hud.visible {
        hud.idle_frames = hud.idle_frames.saturating_add(1);
        if hud.idle_frames >= HUD_HIDE_FRAMES {
            hud.visible = false;
        }
    }
}

pub fn persist_due(hud: &HudState) -> bool {
    hud.persist_frames >= PERSIST_EVERY_FRAMES
}

pub fn mark_persisted(hud: &mut HudState) {
    hud.persist_frames = 0;
}

pub fn ended(hud: &HudState) -> bool {
    !hud.unavailable && hud.duration_ns > 0 && hud.position_ns >= hud.duration_ns
}

fn show(hud: &mut HudState) {
    hud.visible = true;
    hud.idle_frames = 0;
}

pub fn draw(s: &mut impl Surface, hud: &HudState) {
    draw_lang(s, hud, Lang::En);
}

pub fn draw_lang(s: &mut impl Surface, hud: &HudState, lang: Lang) {
    if hud.unavailable {
        draw_unavailable_lang(s, lang);
        return;
    }
    if let Some(reason) = &hud.refuse {
        draw_refuse_lang(s, reason, lang);
        return;
    }
    if let Some(prompt) = &hud.prompt {
        draw_prompt(s, hud, prompt, lang);
        return;
    }
    if let Some(menu) = &hud.menu {
        let (tracks, selected) = match menu.kind {
            TrackKind::Audio => (hud.audio.as_slice(), hud.audio_idx),
            TrackKind::Subtitle => (hud.subs.as_slice(), hud.sub_idx),
        };
        tracks::draw_menu_lang(s, menu, tracks, selected, lang);
        return;
    }
    if let Some(text) = caption(hud) {
        draw_caption(s, text);
    }
    if !hud.visible {
        return;
    }
    let y = SCR_H as i32 - 180;
    panel(s, 80, y, SCR_W - 160, 140);
    let label = if hud.paused {
        t(lang, "paused")
    } else {
        t(lang, "playing")
    };
    Label::new(112, y + 24, label).headline().draw(s);
    if !hud.title.is_empty() {
        Label::new(320, y + 28, &hud.title).caption().muted().draw(s);
    }
    let clock = format!(
        "{} / {}",
        format_ns(hud.position_ns),
        format_ns(hud.duration_ns)
    );
    Label::new(112, y + 72, &clock).caption().draw(s);
    let bar_x = 112;
    let bar_y = y + 108;
    let bar_w = SCR_W - 224;
    s.fill(bar_x, bar_y, bar_w, 8, role::CONTROL_IDLE_FILL);
    let filled = if hud.duration_ns <= 0 {
        0
    } else {
        ((hud.position_ns as u128 * bar_w as u128) / hud.duration_ns as u128) as u32
    };
    s.fill(bar_x, bar_y, filled.max(4), 8, role::ACCENT);
}

fn draw_caption(s: &mut impl Surface, text: &str) {
    let y = SCR_H as i32 - 260;
    panel(s, 240, y, SCR_W - 480, 72);
    Label::new(280, y + 20, text).caption().draw(s);
}

fn draw_prompt(s: &mut impl Surface, hud: &HudState, prompt: &ContinuePrompt, lang: Lang) {
    scrim(s, 0, 0, SCR_W, SCR_H);
    panel(s, 480, 320, 960, 360);
    Label::new(520, 360, t(lang, "continue_prompt"))
        .headline()
        .draw(s);
    let clock = format!("{} {}", t(lang, "resume_at"), format_ns(prompt.position_ns));
    Label::new(520, 420, &clock).caption().muted().draw(s);
    if !hud.title.is_empty() {
        Label::new(520, 460, &hud.title).caption().muted().draw(s);
    }
    let labels = [t(lang, "continue"), t(lang, "start_over")];
    for (i, name) in labels.iter().enumerate() {
        let x = 520 + i as i32 * 280;
        harbor_ui::glass::card(s, x, 520, 240, 72, prompt.choice == i);
        Label::new(x + 24, 540, name).caption().draw(s);
    }
}

pub fn draw_refuse(s: &mut impl Surface, reason: &str) {
    draw_refuse_lang(s, reason, Lang::En);
}

pub fn draw_refuse_lang(s: &mut impl Surface, reason: &str, lang: Lang) {
    scrim(s, 0, 0, SCR_W, SCR_H);
    panel(s, 480, 360, 960, 280);
    Label::new(520, 400, t(lang, "cannot_decode_file"))
        .headline()
        .draw(s);
    Label::new(520, 460, reason).caption().muted().draw(s);
    Label::new(520, 520, t(lang, "ok_back"))
        .caption()
        .muted()
        .draw(s);
}

pub fn draw_unavailable(s: &mut impl Surface) {
    draw_unavailable_lang(s, Lang::En);
}

pub fn draw_unavailable_lang(s: &mut impl Surface, lang: Lang) {
    scrim(s, 0, 0, SCR_W, SCR_H);
    panel(s, 480, 360, 960, 280);
    Label::new(520, 400, t(lang, "native_unavailable"))
        .headline()
        .draw(s);
    Label::new(520, 460, t(lang, "cannot_decode_path"))
        .caption()
        .muted()
        .draw(s);
    Label::new(520, 520, t(lang, "ok_back"))
        .caption()
        .muted()
        .draw(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;

    #[test]
    fn ok_toggles_pause_and_shows_hud() {
        let mut hud = HudState::playing("clip");
        hud.visible = false;
        apply_key(&mut hud, Key::Ok, &Settings::default());
        assert!(hud.paused);
        assert!(hud.visible);
        apply_key(&mut hud, Key::Ok, &Settings::default());
        assert!(!hud.paused);
    }

    #[test]
    fn arrows_seek_ten_seconds() {
        let mut hud = HudState::playing("clip");
        hud.position_ns = 30_000_000_000;
        apply_key(&mut hud, Key::Right, &Settings::default());
        assert_eq!(hud.position_ns, 40_000_000_000);
        apply_key(&mut hud, Key::Left, &Settings::default());
        assert_eq!(hud.position_ns, 30_000_000_000);
    }

    #[test]
    fn hides_after_four_seconds() {
        let mut hud = HudState::playing("clip");
        hud.paused = true;
        for _ in 0..HUD_HIDE_FRAMES {
            tick(&mut hud);
        }
        assert!(!hud.visible);
    }

    #[test]
    fn sandbox_panel_paints_copy() {
        let mut r = Recorder::default();
        draw(&mut r, &HudState::unavailable());
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"Native video unavailable"));
        assert!(texts.iter().any(|t| t.contains("cannot decode")));
    }

    #[test]
    fn refuse_panel_paints_codec() {
        let mut r = Recorder::default();
        draw(
            &mut r,
            &HudState::refused("unsupported_codec: This TV cannot decode HEVC 10-bit"),
        );
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"This TV cannot decode this file"));
        assert!(texts.iter().any(|t| t.contains("HEVC 10-bit")));
        assert!(texts.iter().any(|t| t.contains("unsupported_codec")));
    }

    #[test]
    fn format_clock() {
        assert_eq!(format_ns(0), "00:00");
        assert_eq!(format_ns(65_000_000_000), "01:05");
    }

    #[test]
    fn continue_ok_resumes_start_over_zeros() {
        let mut hud = HudState::playing("clip").with_prompt(12_000_000_000);
        apply_key(&mut hud, Key::Ok, &Settings::default());
        assert_eq!(hud.position_ns, 12_000_000_000);
        assert!(hud.prompt.is_none());
        let mut hud = HudState::playing("clip").with_prompt(12_000_000_000);
        apply_key(&mut hud, Key::Right, &Settings::default());
        apply_key(&mut hud, Key::Ok, &Settings::default());
        assert_eq!(hud.position_ns, 0);
        assert!(!hud.paused);
    }

    #[test]
    fn green_opens_audio_yellow_selects_sub_off() {
        let mut hud = HudState::playing("clip");
        apply_key(&mut hud, Key::Green, &Settings::default());
        assert_eq!(hud.menu.as_ref().unwrap().kind, TrackKind::Audio);
        apply_key(&mut hud, Key::Back, &Settings::default());
        assert!(hud.menu.is_none());
        apply_key(&mut hud, Key::Yellow, &Settings::default());
        apply_key(&mut hud, Key::Up, &Settings::default());
        apply_key(&mut hud, Key::Ok, &Settings::default());
        assert_eq!(hud.sub_idx, 0);
        assert!(hud.menu.is_none());
    }

    #[test]
    fn srt_overlay_at_playhead() {
        let mut hud = HudState::playing("clip");
        hud.position_ns = 2_000_000_000;
        assert_eq!(caption(&hud), Some("The river at dusk"));
        hud.sub_idx = 0;
        assert_eq!(caption(&hud), None);
        let mut r = Recorder::default();
        hud.sub_idx = 1;
        hud.visible = false;
        draw(&mut r, &hud);
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"The river at dusk"));
    }

    #[test]
    fn prompt_paints_choices() {
        let mut r = Recorder::default();
        draw(&mut r, &HudState::playing("clip").with_prompt(12_000_000_000));
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"Continue watching"));
        assert!(texts.contains(&"Continue"));
        assert!(texts.contains(&"Start over"));
    }

    #[test]
    fn zh_hans_hud_and_refuse() {
        let mut hud = HudState::playing("clip");
        hud.visible = true;
        let mut r = Recorder::default();
        draw_lang(&mut r, &hud, Lang::ZhHans);
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"播放中"));
        let mut r = Recorder::default();
        draw_lang(
            &mut r,
            &HudState::refused("unsupported_codec: HEVC 10-bit"),
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
        assert!(texts.contains(&"此电视无法解码此文件"));
        assert!(texts.contains(&"确定 / 返回"));
    }
}
