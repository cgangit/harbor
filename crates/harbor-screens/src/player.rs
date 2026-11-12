use harbor_platform::Lang;
use harbor_ui::draw::Surface;
use harbor_ui_kit::HudState;

pub fn draw(s: &mut impl Surface, hud: &HudState) {
    draw_lang(s, hud, Lang::En);
}

pub fn draw_lang(s: &mut impl Surface, hud: &HudState, lang: Lang) {
    harbor_ui_kit::draw_lang(s, hud, lang);
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;
    use harbor_ui_kit::HudState;

    #[test]
    fn playing_hud_shows_clock() {
        let mut hud = HudState::playing("the-river.mkv");
        hud.position_ns = 12_000_000_000;
        let mut r = Recorder::default();
        draw(&mut r, &hud);
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"Playing"));
        assert!(texts.iter().any(|t| t.contains("00:12")));
        assert!(texts.contains(&"the-river.mkv"));
    }
}
