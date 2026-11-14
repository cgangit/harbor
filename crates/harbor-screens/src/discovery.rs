//! SSDP discovery feeding the Add Source dialog (network-source-browsing D5):
//! real MediaServer names replace the old hard-coded list, and the shared
//! result handle lets the `Added` action turn a picked name back into a
//! persisted `SourceConfig`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use harbor_sources::MediaServer;
use harbor_ui_kit::AddStep;

use crate::nav::View;

/// Shared handle to an in-flight SSDP discovery (D5). A newtype because
/// `Mutex<T>` never gets `Eq`, which `View` derives; equality compares the
/// collected servers (same-arc shortcut avoids locking a mutex twice).
#[derive(Clone, Debug)]
pub struct DiscoveryHandle(pub Arc<Mutex<Vec<MediaServer>>>);

impl PartialEq for DiscoveryHandle {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
            == *other.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Eq for DiscoveryHandle {}

impl DiscoveryHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    pub fn with_servers(servers: Vec<MediaServer>) -> Self {
        Self(Arc::new(Mutex::new(servers)))
    }
}

impl Default for DiscoveryHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Drive SSDP discovery while the Add dialog sits on its Discover step.
///
/// Called once per frame from the app loop, next to `tick_add`:
/// - entering the Discover step starts a worker thread (design D3: the UI
///   thread never blocks on the network);
/// - finished results are folded into the step's server list, re-opening a
///   stale `Empty` state if the list lands after the dialog's own timeout;
/// - the shared handle stays on the view until the dialog closes so the
///   `Added` action can resolve the picked name.
pub fn poll_discovery(view: &mut View) {
    let in_discover = matches!(
        view.add.as_ref().map(|a| &a.step),
        Some(AddStep::Discover { .. })
    );
    if in_discover && view.discovery.is_none() {
        let shared = DiscoveryHandle::new();
        view.discovery = Some(shared.clone());
        std::thread::spawn(move || {
            let found = harbor_sources::discover_media_servers(
                harbor_sources::SSDP_MULTICAST,
                Duration::from_millis(harbor_sources::DISCOVER_TIMEOUT_MS),
            );
            match found {
                Ok(servers) => *shared.0.lock().unwrap_or_else(|e| e.into_inner()) = servers,
                Err(e) => harbor_base::log(&format!("discovery failed: {e}")),
            }
        });
    }
    let Some(shared) = view.discovery.clone() else {
        return;
    };
    let servers = shared.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if servers.is_empty() {
        return;
    }
    let names: Vec<String> = servers.iter().map(|s| s.friendly_name.clone()).collect();
    let Some(add) = view.add.as_mut() else {
        return;
    };
    if matches!(add.step, AddStep::Empty) {
        // The list outlived the dialog's own frame budget: show it rather
        // than the "no servers" state.
        add.step = AddStep::Discover {
            frames: 0,
            servers: names,
            index: 0,
        };
    } else if let AddStep::Discover { servers: current, .. } = &mut add.step {
        if *current != names {
            *current = names;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_platform::Key;
    use harbor_sources::{Kind, MediaServer};
    use harbor_ui_kit::AddSource;

    use crate::nav::handle;

    fn living_room() -> MediaServer {
        MediaServer {
            udn: "uuid:minidlna-living".into(),
            friendly_name: "Living Room NAS".into(),
            location: "http://192.168.1.10:8200/rootDesc.xml".into(),
            control_url: "http://192.168.1.10:8200/ctl/ContentDir".into(),
        }
    }

    #[test]
    fn discovery_results_feed_the_dialog_and_land_in_source_store() {
        let mut v = View::default();
        // Simulate the worker having finished while the dialog is open; the
        // poll must not spawn a second (real, networked) discovery.
        v.discovery = Some(DiscoveryHandle::with_servers(vec![living_room()]));
        v.add = Some(AddSource::discovering(Vec::new()));

        poll_discovery(&mut v);
        match &v.add.as_ref().unwrap().step {
            AddStep::Discover { servers, .. } => {
                assert_eq!(servers.clone(), vec!["Living Room NAS".to_string()]);
            }
            other => panic!("expected Discover with names, got {other:?}"),
        }

        let v = handle(&v, Key::Ok);
        assert!(v.add.is_none());
        let cfg = v
            .sources
            .get("dlna:uuid:minidlna-living")
            .expect("discovered server persisted to the source store");
        assert_eq!(cfg.kind, Kind::Dlna);
        assert_eq!(cfg.name, "Living Room NAS");
        assert_eq!(
            cfg.dlna_control_url.as_deref(),
            Some("http://192.168.1.10:8200/ctl/ContentDir")
        );
        assert_eq!(v.added, vec!["Living Room NAS".to_string()]);
        assert!(v.needs_refresh);
    }

    #[test]
    fn late_results_reopen_the_empty_state() {
        let mut v = View::default();
        v.discovery = Some(DiscoveryHandle::with_servers(vec![living_room()]));
        v.add = Some(AddSource::discovering(Vec::new()));
        v.add.as_mut().unwrap().step = AddStep::Empty;

        poll_discovery(&mut v);
        match &v.add.as_ref().unwrap().step {
            AddStep::Discover { servers, .. } => {
                assert_eq!(servers.clone(), vec!["Living Room NAS".to_string()]);
            }
            other => panic!("expected Discover after late results, got {other:?}"),
        }
    }
}
