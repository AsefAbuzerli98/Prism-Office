//! In-process event bus. Every published event is fanned out to Rust
//! subscribers; `lib.rs` installs one subscriber that forwards events to all
//! webview windows as `prism://event`.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::documents::DocumentInfo;
use crate::services::ServiceInfo;
use crate::settings::Settings;

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AppEvent {
    DocumentOpened {
        info: DocumentInfo,
    },
    DocumentSaved {
        info: DocumentInfo,
    },
    DocumentClosed {
        id: u64,
    },
    ServiceChanged {
        info: ServiceInfo,
    },
    /// Files the OS asked us to open (file association, second instance, drag on dock).
    OpenFiles {
        paths: Vec<String>,
    },
    SettingsChanged {
        settings: Settings,
    },
    RecentChanged,
}

type Subscriber = Arc<dyn Fn(&AppEvent) + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventBus {
    subscribers: Arc<Mutex<Vec<Subscriber>>>,
}

impl EventBus {
    pub fn subscribe(&self, f: impl Fn(&AppEvent) + Send + Sync + 'static) {
        self.subscribers.lock().unwrap().push(Arc::new(f));
    }

    pub fn publish(&self, event: AppEvent) {
        // Clone the list so subscribers may publish/subscribe re-entrantly.
        let subs: Vec<Subscriber> = self.subscribers.lock().unwrap().clone();
        for s in subs {
            s(&event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn fans_out_and_serialises_tagged() {
        let bus = EventBus::default();
        let n = Arc::new(AtomicUsize::new(0));
        for _ in 0..2 {
            let n = n.clone();
            bus.subscribe(move |_| {
                n.fetch_add(1, Ordering::SeqCst);
            });
        }
        bus.publish(AppEvent::DocumentClosed { id: 4 });
        assert_eq!(n.load(Ordering::SeqCst), 2);
        let json = serde_json::to_value(AppEvent::OpenFiles {
            paths: vec!["a.docx".into()],
        })
        .unwrap();
        assert_eq!(json["type"], "openFiles");
        assert_eq!(json["paths"][0], "a.docx");
    }
}
