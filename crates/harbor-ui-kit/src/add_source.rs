use harbor_platform::{t, Lang};
use harbor_ui::Key;
use harbor_ui::draw::Surface;
use harbor_ui::glass::{card, panel, scrim};
use harbor_ui::label::Label;
use harbor_ui::{SCR_H, SCR_W};

pub const TIMEOUT_FRAMES: u32 = 500;
pub const SMB_TIMEOUT_FRAMES: u32 = 600;
pub const KINDS: [&str; 2] = ["DLNA", "SMB"];
pub const SMB_AUTH_FAILED: &str = "authentication failed";
pub const SMB_UNREACHABLE: &str = "network-unreachable";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmbForm {
    pub host: String,
    pub share: String,
    pub username: String,
    pub password: String,
    pub domain: String,
    pub focus: usize,
    pub error: Option<String>,
}

impl SmbForm {
    pub fn new() -> Self {
        Self {
            host: String::new(),
            share: String::new(),
            username: String::new(),
            password: String::new(),
            domain: String::new(),
            focus: 0,
            error: None,
        }
    }

    pub fn field_count() -> usize {
        6
    }

    pub fn field_value(&self, i: usize) -> &str {
        match i {
            0 => &self.host,
            1 => &self.share,
            2 => &self.username,
            3 => &self.password,
            4 => &self.domain,
            _ => "Save",
        }
    }

    pub fn set_field(&mut self, i: usize, value: &str) {
        match i {
            0 => self.host = value.to_string(),
            1 => self.share = value.to_string(),
            2 => self.username = value.to_string(),
            3 => self.password = value.to_string(),
            4 => self.domain = value.to_string(),
            _ => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddStep {
    Kind,
    Discover {
        frames: u32,
        servers: Vec<String>,
        index: usize,
    },
    Empty,
    Smb(SmbForm),
    SmbUnreachable(SmbForm),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddSource {
    pub step: AddStep,
    pub kind_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddAction {
    None,
    Close,
    Added(String),
    AddedSmb(SmbForm),
}

impl AddSource {
    pub fn pick() -> Self {
        Self {
            step: AddStep::Kind,
            kind_index: 0,
        }
    }

    pub fn smb_auth_failed(form: SmbForm) -> Self {
        let mut form = form;
        form.error = Some(SMB_AUTH_FAILED.into());
        form.focus = 2;
        Self {
            step: AddStep::Smb(form),
            kind_index: 1,
        }
    }

    pub fn smb_unreachable(form: SmbForm) -> Self {
        Self {
            step: AddStep::SmbUnreachable(form),
            kind_index: 1,
        }
    }

    pub fn discovering(servers: Vec<String>) -> Self {
        Self {
            step: AddStep::Discover {
                frames: 0,
                index: 0,
                servers,
            },
            kind_index: 0,
        }
    }
}

pub fn tick(state: &mut AddSource) {
    if let AddStep::Discover { frames, servers, .. } = &mut state.step {
        *frames = frames.saturating_add(1);
        if *frames >= TIMEOUT_FRAMES && servers.is_empty() {
            state.step = AddStep::Empty;
        }
    }
}

pub fn apply_key(state: &mut AddSource, key: Key) -> AddAction {
    match &mut state.step {
        AddStep::Kind => match key {
            Key::Back => AddAction::Close,
            Key::Left | Key::Up => {
                state.kind_index = 0;
                AddAction::None
            }
            Key::Right | Key::Down => {
                state.kind_index = 1;
                AddAction::None
            }
            Key::Ok if state.kind_index == 0 => {
                state.step = AddStep::Discover {
                    frames: 0,
                    servers: Vec::new(),
                    index: 0,
                };
                AddAction::None
            }
            Key::Ok => {
                state.step = AddStep::Smb(SmbForm::new());
                AddAction::None
            }
            _ => AddAction::None,
        },
        AddStep::Discover {
            servers,
            index,
            ..
        } => match key {
            Key::Back => {
                state.step = AddStep::Kind;
                AddAction::None
            }
            Key::Up | Key::Left if !servers.is_empty() => {
                *index = (*index + servers.len() - 1) % servers.len();
                AddAction::None
            }
            Key::Down | Key::Right if !servers.is_empty() => {
                *index = (*index + 1) % servers.len();
                AddAction::None
            }
            Key::Ok if !servers.is_empty() => {
                AddAction::Added(servers[*index].clone())
            }
            _ => AddAction::None,
        },
        AddStep::Empty => match key {
            Key::Back => {
                state.step = AddStep::Kind;
                AddAction::None
            }
            Key::Ok => {
                state.step = AddStep::Discover {
                    frames: 0,
                    servers: Vec::new(),
                    index: 0,
                };
                AddAction::None
            }
            _ => AddAction::None,
        },
        AddStep::Smb(form) => match key {
            Key::Back => {
                state.step = AddStep::Kind;
                AddAction::None
            }
            Key::Up | Key::Left => {
                form.focus = (form.focus + SmbForm::field_count() - 1) % SmbForm::field_count();
                AddAction::None
            }
            Key::Down | Key::Right => {
                form.focus = (form.focus + 1) % SmbForm::field_count();
                AddAction::None
            }
            Key::Ok if form.focus == 5 => match harbor_session::validate_smb(&form.host, &form.share)
            {
                Ok(()) => AddAction::AddedSmb(form.clone()),
                Err(msg) => {
                    form.error = Some(msg.to_string());
                    AddAction::None
                }
            },
            _ => AddAction::None,
        },
        AddStep::SmbUnreachable(form) => match key {
            Key::Back => {
                state.step = AddStep::Kind;
                AddAction::None
            }
            Key::Ok => {
                state.step = AddStep::Smb(form.clone());
                AddAction::None
            }
            _ => AddAction::None,
        },
    }
}

pub fn draw(s: &mut impl Surface, state: &AddSource) {
    draw_lang(s, state, Lang::En);
}

pub fn draw_lang(s: &mut impl Surface, state: &AddSource, lang: Lang) {
    scrim(s, 0, 0, SCR_W, SCR_H);
    panel(s, 400, 160, 1120, 720);
    Label::new(448, 200, t(lang, "add_source")).headline().draw(s);
    match &state.step {
        AddStep::Kind => {
            Label::new(448, 270, t(lang, "choose_type"))
                .caption()
                .muted()
                .draw(s);
            for (i, name) in KINDS.iter().enumerate() {
                let y = 340 + i as i32 * 96;
                card(s, 448, y, 640, 80, state.kind_index == i);
                Label::new(480, y + 24, name).caption().draw(s);
            }
        }
        AddStep::Discover { servers, index, .. } => {
            Label::new(448, 270, t(lang, "looking_servers"))
                .caption()
                .muted()
                .draw(s);
            if servers.is_empty() {
                Label::new(448, 340, t(lang, "searching_lan"))
                    .caption()
                    .draw(s);
            } else {
                for (i, name) in servers.iter().enumerate() {
                    let y = 340 + i as i32 * 80;
                    card(s, 448, y, 720, 64, *index == i);
                    Label::new(480, y + 16, name).caption().draw(s);
                }
            }
        }
        AddStep::Empty => {
            Label::new(448, 280, t(lang, "no_servers"))
                .headline()
                .draw(s);
            Label::new(448, 350, t(lang, "lan_hint"))
                .caption()
                .muted()
                .draw(s);
            card(s, 448, 430, 320, 72, true);
            Label::new(480, 450, t(lang, "retry")).caption().draw(s);
        }
        AddStep::Smb(form) => {
            Label::new(448, 270, t(lang, "smb_share"))
                .caption()
                .muted()
                .draw(s);
            let labels = [
                t(lang, "host"),
                t(lang, "share"),
                t(lang, "username"),
                t(lang, "password"),
                t(lang, "domain"),
                t(lang, "save"),
            ];
            for (i, name) in labels.iter().enumerate() {
                let y = 320 + i as i32 * 72;
                card(s, 448, y, 720, 64, form.focus == i);
                let value = if i == 3 && !form.password.is_empty() {
                    "********".to_string()
                } else if i == 5 {
                    String::new()
                } else {
                    form.field_value(i).to_string()
                };
                let line = if value.is_empty() {
                    (*name).to_string()
                } else {
                    format!("{name}: {value}")
                };
                Label::new(480, y + 16, &line).caption().draw(s);
            }
            if let Some(err) = &form.error {
                Label::new(448, 760, err).caption().muted().draw(s);
            }
        }
        AddStep::SmbUnreachable(_) => {
            Label::new(448, 280, t(lang, "unreachable"))
                .headline()
                .draw(s);
            Label::new(448, 350, t(lang, "smb_timeout_hint"))
                .caption()
                .muted()
                .draw(s);
            card(s, 448, 430, 320, 72, true);
            Label::new(480, 450, t(lang, "retry")).caption().draw(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_ui::draw::Recorder;

    #[test]
    fn dlna_ok_then_timeout_empty_retry() {
        let mut s = AddSource::pick();
        assert_eq!(apply_key(&mut s, Key::Ok), AddAction::None);
        match &s.step {
            AddStep::Discover { servers, .. } => assert!(servers.is_empty()),
            other => panic!("{other:?}"),
        }
        for _ in 0..TIMEOUT_FRAMES {
            tick(&mut s);
        }
        assert_eq!(s.step, AddStep::Empty);
        apply_key(&mut s, Key::Ok);
        match s.step {
            AddStep::Discover { frames, .. } => assert_eq!(frames, 0),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn select_fixture_server() {
        let mut s = AddSource::discovering(vec!["Living Room NAS".into()]);
        assert_eq!(
            apply_key(&mut s, Key::Ok),
            AddAction::Added("Living Room NAS".into())
        );
    }

    #[test]
    fn empty_paints_lan_hint() {
        let mut r = Recorder::default();
        draw(
            &mut r,
            &AddSource {
                step: AddStep::Empty,
                kind_index: 0,
            },
        );
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"No MediaServers found"));
        assert!(texts.contains(&"TV and server must share a LAN"));
        assert!(texts.contains(&"Retry"));
    }

    #[test]
    fn smb_requires_host_and_share_then_saves() {
        let mut s = AddSource::pick();
        apply_key(&mut s, Key::Down);
        apply_key(&mut s, Key::Ok);
        let AddStep::Smb(form) = &mut s.step else {
            panic!("{:?}", s.step);
        };
        form.focus = 5;
        assert_eq!(apply_key(&mut s, Key::Ok), AddAction::None);
        let AddStep::Smb(form) = &mut s.step else {
            panic!();
        };
        assert_eq!(form.error.as_deref(), Some("Host is required"));
        form.host = "nas".into();
        form.share = "media".into();
        form.username = "alice".into();
        form.password = "s3cret".into();
        form.domain = "HOME".into();
        form.focus = 5;
        match apply_key(&mut s, Key::Ok) {
            AddAction::AddedSmb(f) => {
                assert_eq!(f.host, "nas");
                assert_eq!(f.share, "media");
                assert_eq!(f.password, "s3cret");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smb_form_hides_password() {
        let mut form = SmbForm::new();
        form.password = "s3cret".into();
        form.host = "nas".into();
        form.share = "media".into();
        let mut r = Recorder::default();
        draw(
            &mut r,
            &AddSource {
                step: AddStep::Smb(form),
                kind_index: 1,
            },
        );
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.contains("********")));
        assert!(!texts.iter().any(|t| t.contains("s3cret")));
        assert!(texts.contains(&"SMB share"));
    }

    fn filled_form() -> SmbForm {
        let mut form = SmbForm::new();
        form.host = "nas".into();
        form.share = "media".into();
        form.username = "alice".into();
        form.password = "bad".into();
        form
    }

    #[test]
    fn smb_auth_fail_offers_edit_credentials() {
        let mut s = AddSource::smb_auth_failed(filled_form());
        match &s.step {
            AddStep::Smb(form) => {
                assert_eq!(form.error.as_deref(), Some(SMB_AUTH_FAILED));
                assert_eq!(form.host, "nas");
                assert_eq!(form.focus, 2);
            }
            other => panic!("{other:?}"),
        }
        let mut r = Recorder::default();
        draw(&mut r, &s);
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.contains(SMB_AUTH_FAILED)));
        assert!(texts.contains(&"SMB share"));
        apply_key(&mut s, Key::Down);
        match &s.step {
            AddStep::Smb(form) => assert_eq!(form.focus, 3),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smb_timeout_is_unreachable_retry_edits_form() {
        let mut s = AddSource::smb_unreachable(filled_form());
        match &s.step {
            AddStep::SmbUnreachable(_) => {}
            other => panic!("{other:?}"),
        }
        let mut r = Recorder::default();
        draw(&mut r, &s);
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&SMB_UNREACHABLE));
        assert!(texts.contains(&"Host did not respond in 10 seconds"));
        assert!(texts.contains(&"Retry"));
        apply_key(&mut s, Key::Ok);
        match s.step {
            AddStep::Smb(form) => {
                assert_eq!(form.host, "nas");
                assert_eq!(form.share, "media");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(SMB_TIMEOUT_FRAMES, 600);
    }

    #[test]
    fn zh_hans_add_source_empty_and_smb() {
        let mut r = Recorder::default();
        draw_lang(
            &mut r,
            &AddSource {
                step: AddStep::Empty,
                kind_index: 0,
            },
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
        assert!(texts.contains(&"添加来源"));
        assert!(texts.contains(&"未找到媒体服务器"));
        assert!(texts.contains(&"重试"));
        let mut r = Recorder::default();
        draw_lang(
            &mut r,
            &AddSource {
                step: AddStep::SmbUnreachable(SmbForm::new()),
                kind_index: 1,
            },
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
        assert!(texts.contains(&"网络不可达"));
        assert!(texts.contains(&"主机 10 秒内未响应"));
    }
}
