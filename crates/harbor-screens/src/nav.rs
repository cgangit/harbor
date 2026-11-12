use harbor_platform::Key;
use harbor_session::{
    should_offer, CredentialVault, Resume, ResumeStore, Settings, SourceConfig, SourceStore,
};
use harbor_ui::{reduce, Focus};
use harbor_ui_kit::{AddAction, AddSource, HudState};

use crate::catalog::Catalog;
use crate::{rows, Screen};

pub const DEMO_SOURCE: &str = "demo";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub screen: Screen,
    pub focus: Focus,
    pub hud: Option<HudState>,
    pub store: ResumeStore,
    pub add: Option<AddSource>,
    pub added: Vec<String>,
    pub sources: SourceStore,
    pub vault: CredentialVault,
    pub settings: Settings,
    stack: Vec<(Screen, Focus)>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            screen: Screen::Home,
            focus: Focus::default(),
            hud: None,
            store: ResumeStore::in_memory(),
            add: None,
            added: Vec::new(),
            sources: SourceStore::in_memory(),
            vault: CredentialVault::in_memory(),
            settings: Settings::default(),
            stack: Vec::new(),
        }
    }
}

impl View {
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn with_store(store: ResumeStore) -> Self {
        Self {
            store,
            ..Self::default()
        }
    }
}

pub fn handle(view: &View, catalog: &Catalog, key: Key) -> View {
    if view.add.is_some() {
        return handle_add(view, key);
    }
    if view.screen == Screen::Player {
        return handle_player(view, key);
    }
    match key {
        Key::Left | Key::Right | Key::Up | Key::Down => {
            let mut next = view.clone();
            next.focus = reduce(view.focus, &rows(view.screen, catalog), key);
            next.hud = None;
            next
        }
        Key::Back => pop(view),
        Key::Ok => on_ok(view, catalog),
        _ => view.clone(),
    }
}

fn handle_add(view: &View, key: Key) -> View {
    let mut next = view.clone();
    let Some(add) = next.add.as_mut() else {
        return next;
    };
    match harbor_ui_kit::apply_add_key(add, key) {
        AddAction::Close => next.add = None,
        AddAction::Added(name) => {
            next.add = None;
            if !next.added.contains(&name) {
                next.added.push(name);
            }
        }
        AddAction::AddedSmb(form) => {
            let id = harbor_session::smb_id(&form.host, &form.share);
            next.sources.put(SourceConfig::smb(
                &id,
                &form.host,
                &form.share,
                &form.username,
                &form.domain,
            ));
            next.vault.put(&id, &form.password);
            let name = next
                .sources
                .get(&id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| format!("{}/{}", form.host, form.share));
            next.add = None;
            if !next.added.contains(&name) {
                next.added.push(name);
            }
        }
        AddAction::None => {}
    }
    next
}

pub fn tick_add(view: &mut View) {
    if let Some(add) = view.add.as_mut() {
        harbor_ui_kit::tick_add(add);
    }
}

fn handle_player(view: &View, key: Key) -> View {
    let unavailable = view.hud.as_ref().map(|h| h.unavailable).unwrap_or(false);
    let overlay = view
        .hud
        .as_ref()
        .map(|h| h.menu.is_some() || h.prompt.is_some())
        .unwrap_or(false);
    let refused = view.hud.as_ref().and_then(|h| h.refuse.as_ref()).is_some();
    if (key == Key::Back && !overlay) || ((unavailable || refused) && key == Key::Ok) {
        let mut next = view.clone();
        persist(&mut next);
        let mut next = pop(&next);
        next.hud = None;
        return next;
    }
    let mut next = view.clone();
    if let Some(hud) = next.hud.as_mut() {
        harbor_ui_kit::apply_key(hud, key, &Settings::default());
    }
    if !overlay {
        persist(&mut next);
    }
    next
}

fn push(view: &View, screen: Screen) -> View {
    let mut next = view.clone();
    next.stack.push((view.screen, view.focus));
    next.screen = screen;
    next.focus = Focus::default();
    next.hud = None;
    next.add = None;
    next
}

fn pop(view: &View) -> View {
    let mut next = view.clone();
    match next.stack.pop() {
        Some((screen, focus)) => {
            next.screen = screen;
            next.focus = focus;
            next.hud = None;
            next.add = None;
            next
        }
        None => View {
            store: view.store.clone(),
            added: view.added.clone(),
            sources: view.sources.clone(),
            vault: view.vault.clone(),
            settings: view.settings.clone(),
            ..View::default()
        },
    }
}

fn on_ok(view: &View, catalog: &Catalog) -> View {
    match view.screen {
        Screen::Home => {
            let screen = if catalog.is_empty() || view.focus.row == 3 {
                Screen::Settings
            } else {
                Screen::Browser
            };
            push(view, screen)
        }
        Screen::Browser => {
            if view.focus.row == 2 {
                let title = catalog
                    .files
                    .get(view.focus.col)
                    .cloned()
                    .unwrap_or_else(|| "Item".into());
                open_player(view, &title)
            } else {
                view.clone()
            }
        }
        Screen::Player => pop(view),
        Screen::Settings => {
            if view.focus.row == crate::settings::lang_row_index(catalog) {
                let mut next = view.clone();
                next.settings.ui_lang = next.settings.ui_lang.next();
                next
            } else if catalog.sources.is_empty() || view.focus.row == crate::settings::add_row(catalog)
            {
                let mut next = view.clone();
                next.add = Some(AddSource::pick());
                next
            } else {
                view.clone()
            }
        }
    }
}

fn open_player(view: &View, title: &str) -> View {
    let mut next = push(view, Screen::Player);
    next.hud = Some(open_hud(title, &view.store));
    next
}

fn open_hud(title: &str, store: &ResumeStore) -> HudState {
    if std::env::var_os("HARBOR_FORCE_UNAVAILABLE").is_some() {
        return HudState::unavailable();
    }
    if let Ok(reason) = std::env::var("HARBOR_FORCE_REFUSE") {
        if !reason.is_empty() {
            return HudState::refused(&reason);
        }
    }
    let hud = HudState::playing(title);
    if let Some(r) = store.get(DEMO_SOURCE, title) {
        if should_offer(r.position_ns, r.duration_ns) {
            return hud.with_prompt(r.position_ns);
        }
    }
    hud
}

pub fn persist(view: &mut View) {
    let Some(hud) = view.hud.as_ref() else {
        return;
    };
    if hud.unavailable || hud.refuse.is_some() || hud.title.is_empty() || hud.prompt.is_some() {
        return;
    }
    view.store.put(Resume {
        source_id: DEMO_SOURCE.into(),
        entry_id: hud.title.clone(),
        position_ns: hud.position_ns,
        duration_ns: hud.duration_ns,
        updated_unix: 0,
    });
}

    #[cfg(test)]
    mod tests {
        use super::*;
        use harbor_sources::Source;

    #[test]
    fn populated_home_ok_opens_browser_sources_ok_opens_settings() {
        let cat = Catalog::demo();
        let browser = handle(&View::default(), &cat, Key::Ok);
        assert_eq!(browser.screen, Screen::Browser);
        assert_eq!(browser.depth(), 1);

        let mut sources = View::default();
        sources.focus.row = 3;
        let settings = handle(&sources, &cat, Key::Ok);
        assert_eq!(settings.screen, Screen::Settings);
        assert_eq!(settings.depth(), 1);
    }

    #[test]
    fn empty_home_ok_opens_settings() {
        let next = handle(&View::default(), &Catalog::empty(), Key::Ok);
        assert_eq!(next.screen, Screen::Settings);
    }

    #[test]
    fn back_on_home_stays() {
        let home = handle(&View::default(), &Catalog::demo(), Key::Back);
        assert_eq!(home, View::default());
    }

    #[test]
    fn back_restores_previous_focus() {
        let cat = Catalog::demo();
        let mut home = View::default();
        home.focus.row = 3;
        home.focus.col = 1;
        let settings = handle(&home, &cat, Key::Ok);
        assert_eq!(settings.screen, Screen::Settings);
        let back = handle(&settings, &cat, Key::Back);
        assert_eq!(back.screen, Screen::Home);
        assert_eq!(back.focus, home.focus);
        assert_eq!(back.depth(), 0);
    }

    #[test]
    fn keyboard_walk_home_browser_back_settings() {
        let cat = Catalog::demo();
        let mut v = View::default();
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.screen, Screen::Browser);
        v = handle(&v, &cat, Key::Back);
        assert_eq!(v.screen, Screen::Home);
        v.focus.row = 3;
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.screen, Screen::Settings);
        v = handle(&v, &cat, Key::Back);
        assert_eq!(v.screen, Screen::Home);
    }

    fn browser_on_file() -> View {
        let cat = Catalog::demo();
        let mut v = handle(&View::default(), &cat, Key::Ok);
        v.focus.row = 2;
        v.focus.col = 2;
        handle(&v, &cat, Key::Ok)
    }

    #[test]
    fn browser_file_opens_player_back_unloads() {
        let cat = Catalog::demo();
        let player = browser_on_file();
        assert_eq!(player.screen, Screen::Player);
        let hud = player.hud.as_ref().expect("hud");
        assert_eq!(hud.title, "the-river.mkv");
        assert!(!hud.paused);
        let back = handle(&player, &cat, Key::Back);
        assert_eq!(back.screen, Screen::Browser);
        assert!(back.hud.is_none());
        assert_eq!(back.focus.row, 2);
    }

    #[test]
    fn player_ok_pauses_arrows_seek() {
        let cat = Catalog::demo();
        let mut v = browser_on_file();
        v = handle(&v, &cat, Key::Ok);
        assert!(v.hud.as_ref().unwrap().paused);
        v.hud.as_mut().unwrap().position_ns = 20_000_000_000;
        v = handle(&v, &cat, Key::Right);
        assert_eq!(v.hud.as_ref().unwrap().position_ns, 30_000_000_000);
        v = handle(&v, &cat, Key::Left);
        assert_eq!(v.hud.as_ref().unwrap().position_ns, 20_000_000_000);
    }

    #[test]
    fn refuse_ok_returns() {
        let cat = Catalog::demo();
        let mut v = browser_on_file();
        v.hud = Some(HudState::refused(
            "unsupported_codec: This TV cannot decode VP9",
        ));
        let back = handle(&v, &cat, Key::Ok);
        assert_eq!(back.screen, Screen::Browser);
        assert!(back.hud.is_none());
    }

    #[test]
    fn unavailable_ok_returns() {
        let cat = Catalog::demo();
        let mut v = browser_on_file();
        v.hud = Some(HudState::unavailable());
        let back = handle(&v, &cat, Key::Ok);
        assert_eq!(back.screen, Screen::Browser);
        assert!(back.hud.is_none());
    }

    #[test]
    fn pause_persists_and_reopen_offers_continue() {
        let cat = Catalog::demo();
        let mut v = browser_on_file();
        v.hud.as_mut().unwrap().position_ns = 12_000_000_000;
        v = handle(&v, &cat, Key::Ok);
        assert!(v.store.get(DEMO_SOURCE, "the-river.mkv").is_some());
        v = handle(&v, &cat, Key::Back);
        v.focus.row = 2;
        v.focus.col = 2;
        v = handle(&v, &cat, Key::Ok);
        let hud = v.hud.as_ref().unwrap();
        assert_eq!(hud.prompt.as_ref().unwrap().position_ns, 12_000_000_000);
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.hud.as_ref().unwrap().position_ns, 12_000_000_000);
        assert!(v.hud.as_ref().unwrap().prompt.is_none());
    }

    #[test]
    fn yellow_opens_subs_back_stays_on_player() {
        let cat = Catalog::demo();
        let mut v = browser_on_file();
        v = handle(&v, &cat, Key::Yellow);
        assert!(v.hud.as_ref().unwrap().menu.is_some());
        v = handle(&v, &cat, Key::Back);
        assert_eq!(v.screen, Screen::Player);
        assert!(v.hud.as_ref().unwrap().menu.is_none());
    }

    #[test]
    fn settings_add_source_dlna_timeout_then_pick_server() {
        let cat = Catalog::demo();
        let mut v = View::default();
        v.focus.row = 3;
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.screen, Screen::Settings);
        v.focus.row = 1;
        v = handle(&v, &cat, Key::Ok);
        assert!(v.add.is_some());
        v = handle(&v, &cat, Key::Ok);
        for _ in 0..harbor_ui_kit::TIMEOUT_FRAMES {
            tick_add(&mut v);
        }
        assert_eq!(v.add.as_ref().unwrap().step, harbor_ui_kit::AddStep::Empty);
        v = handle(&v, &cat, Key::Back);
        assert_eq!(v.screen, Screen::Settings);
        assert!(v.add.is_some());
        v.add = Some(harbor_ui_kit::AddSource::discovering(vec![
            "Living Room NAS".into(),
        ]));
        v = handle(&v, &cat, Key::Ok);
        assert!(v.add.is_none());
        assert_eq!(v.added, vec!["Living Room NAS".to_string()]);
        assert_eq!(v.screen, Screen::Settings);
    }

    #[test]
    fn add_smb_persists_source_password_in_vault() {
        let cat = Catalog::demo();
        let mut v = View::default();
        v.focus.row = 3;
        v = handle(&v, &cat, Key::Ok);
        v.focus.row = 1;
        v = handle(&v, &cat, Key::Ok);
        v = handle(&v, &cat, Key::Down);
        v = handle(&v, &cat, Key::Ok);
        let form = match v.add.as_mut().map(|a| &mut a.step) {
            Some(harbor_ui_kit::AddStep::Smb(form)) => form,
            other => panic!("{other:?}"),
        };
        form.host = "nas".into();
        form.share = "media".into();
        form.username = "alice".into();
        form.password = "s3cret".into();
        form.domain = "HOME".into();
        form.focus = 5;
        v = handle(&v, &cat, Key::Ok);
        assert!(v.add.is_none());
        let cfg = v.sources.get("smb:nas/media").unwrap();
        assert_eq!(cfg.smb_host.as_deref(), Some("nas"));
        assert_eq!(cfg.smb_username.as_deref(), Some("alice"));
        assert_eq!(v.vault.get("smb:nas/media"), Some("s3cret"));
        assert!(v.added.iter().any(|n| n.contains("nas/media")));
        let creds = cfg.smb_creds(v.vault.get(&cfg.id).unwrap_or("")).unwrap();
        let src = harbor_sources::SmbSource::fixture(creds);
        let page = src.list("").unwrap();
        assert!(page.entries.iter().any(|e| e.name == "Movies"));
        let movies = src.list("Movies").unwrap();
        let mut h = src.open(&movies.entries[0]).unwrap();
        let mut buf = [0u8; 4];
        assert_eq!(h.read_at(0, &mut buf).unwrap(), 4);
        assert_eq!(&buf, b"fake");
    }

    #[test]
    fn smb_auth_fail_returns_to_edit_credentials() {
        let mut form = harbor_ui_kit::SmbForm::new();
        form.host = "nas".into();
        form.share = "media".into();
        form.username = "alice".into();
        form.password = "bad".into();
        form.focus = 5;
        let creds = harbor_sources::SmbCreds::new("nas", "media", "alice", "", "bad");
        let err = harbor_sources::SmbSource::auth_fail(creds)
            .list("")
            .unwrap_err();
        match err {
            harbor_sources::Error::Auth { .. } => {}
            other => panic!("{other:?}"),
        }
        let mut v = View::default();
        v.add = Some(harbor_ui_kit::AddSource::smb_auth_failed(form));
        match &v.add.as_ref().unwrap().step {
            harbor_ui_kit::AddStep::Smb(form) => {
                assert_eq!(form.error.as_deref(), Some(harbor_ui_kit::SMB_AUTH_FAILED));
                assert_eq!(form.focus, 2);
            }
            other => panic!("{other:?}"),
        }
        v = handle(&v, &Catalog::demo(), Key::Down);
        match &v.add.as_ref().unwrap().step {
            harbor_ui_kit::AddStep::Smb(form) => assert_eq!(form.focus, 3),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smb_timeout_shows_unreachable() {
        let mut form = harbor_ui_kit::SmbForm::new();
        form.host = "nas".into();
        form.share = "media".into();
        let creds = harbor_sources::SmbCreds::new("nas", "media", "", "", "");
        let err = harbor_sources::SmbSource::timed_out(creds)
            .list("")
            .unwrap_err();
        match err {
            harbor_sources::Error::Timeout { what } => assert_eq!(what, "SMB connect"),
            other => panic!("{other:?}"),
        }
        let mut v = View::default();
        v.add = Some(harbor_ui_kit::AddSource::smb_unreachable(form));
        match &v.add.as_ref().unwrap().step {
            harbor_ui_kit::AddStep::SmbUnreachable(_) => {}
            other => panic!("{other:?}"),
        }
        v = handle(&v, &Catalog::demo(), Key::Ok);
        match v.add.unwrap().step {
            harbor_ui_kit::AddStep::Smb(form) => assert_eq!(form.host, "nas"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn settings_ok_on_language_row_cycles_lang() {
        let cat = Catalog::demo();
        let mut v = View::default();
        v.focus.row = 3;
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.screen, Screen::Settings);
        assert_eq!(v.settings.ui_lang, harbor_platform::Lang::En);
        v.focus.row = crate::settings::lang_row_index(&cat);
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.settings.ui_lang, harbor_platform::Lang::ZhHans);
        v = handle(&v, &cat, Key::Ok);
        assert_eq!(v.settings.ui_lang, harbor_platform::Lang::En);
        assert!(v.add.is_none());
    }

    #[test]
    fn ninety_percent_clears_resume() {
        let cat = Catalog::demo();
        let mut v = browser_on_file();
        v.hud.as_mut().unwrap().position_ns = 81_000_000_000;
        v = handle(&v, &cat, Key::Ok);
        assert!(v.store.get(DEMO_SOURCE, "the-river.mkv").is_none());
    }
}
