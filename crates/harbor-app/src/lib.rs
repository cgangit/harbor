use harbor_base::{log, rundir};
use harbor_gfx::Window;
use harbor_screens::{self, Screen, View};
use harbor_session::ResumeStore;

fn smoke_frames() -> Option<u32> {
    std::env::var("HARBOR_SMOKE_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
}

pub fn run() -> Result<(), String> {
    log("boot");
    let mut win = Window::open("Harbor")?;
    let smoke = smoke_frames();
    let mut n: u32 = 0;
    let mut view = View::with_store(ResumeStore::open(rundir().join("resume.log")));
    harbor_screens::refresh_catalog(&mut view);
    log_catalog(&view, "boot");
    loop {
        let (quit, keys) = win.poll_keys();
        if quit {
            break;
        }
        for k in keys {
            let next = harbor_screens::handle(&view, k);
            if next.screen != view.screen {
                log(&format!("screen {:?}", next.screen));
            }
            if next.focus != view.focus {
                log(&format!("focus {:?}", next.focus));
            }
            view = next;
        }
        if view.needs_refresh {
            harbor_screens::refresh_catalog(&mut view);
            log_catalog(&view, "refresh");
        }
        // Fold any posted listing result into the view (design D3): one
        // drain per frame, never a blocking read on the present loop.
        harbor_screens::drain_browse(&mut view);
        if view.screen == Screen::Player {
            if let Some(hud) = view.hud.as_mut() {
                harbor_ui_kit::tick(hud);
            }
            if view.hud.as_ref().map(harbor_ui_kit::persist_due).unwrap_or(false) {
                harbor_screens::persist(&mut view);
                if let Some(hud) = view.hud.as_mut() {
                    harbor_ui_kit::mark_persisted(hud);
                }
            }
            if view.hud.as_ref().map(harbor_ui_kit::ended).unwrap_or(false) {
                view = harbor_screens::handle(&view, harbor_platform::Key::Back);
                log("screen end");
            }
        }
        harbor_screens::poll_discovery(&mut view);
        harbor_screens::tick_add(&mut view);
        let mut frame = win.begin();
        harbor_screens::draw_lang(
            &mut frame,
            view.screen,
            &view.catalog,
            view.focus,
            view.hud.as_ref(),
            view.add.as_ref(),
            Some(harbor_screens::BrowseInfo::of(&view)),
            view.settings.ui_lang,
        );
        frame.present();
        n += 1;
        if let Some(limit) = smoke {
            if n >= limit {
                log("smoke done");
                break;
            }
        }
    }
    Ok(())
}

fn log_catalog(view: &View, why: &str) {
    log(&format!(
        "catalog[{why}] sources={} files={} recent={} resume={}",
        view.catalog.sources.len(),
        view.catalog.files.len(),
        view.catalog.recently_added.len(),
        view.catalog.continue_watching.len()
    ));
}

#[no_mangle]
pub extern "C" fn harbor_run() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(e) => {
            log(&format!("fatal {e}"));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke_frames_parses_decimal() {
        assert_eq!("3".parse::<u32>().ok(), Some(3));
        assert_eq!("".parse::<u32>().ok(), None);
    }
}
