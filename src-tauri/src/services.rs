//! Shared background daemon registry (AI, Python sandbox, recovery …).
//!
//! Daemons are shared by every window. Each implements [`Service`]; the
//! registry tracks lifecycle state and publishes changes on the event bus.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::events::{AppEvent, EventBus};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ServiceState {
    Stopped,
    Running,
    /// The daemon cannot run in this build (e.g. not implemented yet).
    Unavailable {
        reason: String,
    },
    Failed {
        error: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub id: &'static str,
    pub name: &'static str,
    #[serde(flatten)]
    pub state: ServiceState,
}

pub trait Service: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    /// Start the daemon. Returning `Ok(None)` means running; `Ok(Some(reason))`
    /// means the daemon is unavailable in this build.
    fn start(&self, bus: &EventBus) -> Result<Option<String>, String>;
    fn stop(&self) {}
}

#[derive(Default)]
pub struct ServiceRegistry {
    services: Vec<Arc<dyn Service>>,
    states: Mutex<HashMap<&'static str, ServiceState>>,
}

impl ServiceRegistry {
    pub fn register(&mut self, service: Arc<dyn Service>) {
        self.states
            .lock()
            .unwrap()
            .insert(service.id(), ServiceState::Stopped);
        self.services.push(service);
    }

    fn set(&self, bus: &EventBus, svc: &dyn Service, state: ServiceState) {
        self.states.lock().unwrap().insert(svc.id(), state.clone());
        bus.publish(AppEvent::ServiceChanged {
            info: ServiceInfo {
                id: svc.id(),
                name: svc.name(),
                state,
            },
        });
    }

    /// Replace a registered service with the same id (e.g. a real implementation of a planned one).
    pub fn replace(&mut self, service: Arc<dyn Service>) {
        if let Some(slot) = self.services.iter_mut().find(|s| s.id() == service.id()) {
            *slot = service;
        } else {
            self.register(service);
        }
    }

    pub fn start_all(&self, bus: &EventBus) {
        for svc in &self.services {
            let state = match svc.start(bus) {
                Ok(None) => ServiceState::Running,
                Ok(Some(reason)) => ServiceState::Unavailable { reason },
                Err(error) => ServiceState::Failed { error },
            };
            self.set(bus, svc.as_ref(), state);
        }
    }

    pub fn stop_all(&self, bus: &EventBus) {
        for svc in &self.services {
            if self.state(svc.id()) == Some(ServiceState::Running) {
                svc.stop();
                self.set(bus, svc.as_ref(), ServiceState::Stopped);
            }
        }
    }

    pub fn state(&self, id: &str) -> Option<ServiceState> {
        self.states.lock().unwrap().get(id).cloned()
    }

    pub fn list(&self) -> Vec<ServiceInfo> {
        let states = self.states.lock().unwrap();
        self.services
            .iter()
            .map(|s| ServiceInfo {
                id: s.id(),
                name: s.name(),
                state: states.get(s.id()).cloned().unwrap_or(ServiceState::Stopped),
            })
            .collect()
    }
}

/// Placeholder for a daemon whose implementation lands in a later task.
pub struct PlannedService {
    pub id: &'static str,
    pub name: &'static str,
    pub task: &'static str,
}

impl Service for PlannedService {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn start(&self, _bus: &EventBus) -> Result<Option<String>, String> {
        Ok(Some(format!("not implemented yet ({})", self.task)))
    }
}

/// The daemons every Prism Office process hosts.
pub fn default_registry() -> ServiceRegistry {
    let mut r = ServiceRegistry::default();
    for (id, name, task) in [
        ("ai", "AI services", "X-02"),
        ("python", "Python sandbox", "P3-12"),
        ("recovery", "Auto-recovery journal", "X-01"),
    ] {
        r.register(Arc::new(PlannedService { id, name, task }));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct Echo(AtomicBool);
    impl Service for Echo {
        fn id(&self) -> &'static str {
            "echo"
        }
        fn name(&self) -> &'static str {
            "Echo"
        }
        fn start(&self, _: &EventBus) -> Result<Option<String>, String> {
            self.0.store(true, Ordering::SeqCst);
            Ok(None)
        }
        fn stop(&self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }

    struct Broken;
    impl Service for Broken {
        fn id(&self) -> &'static str {
            "broken"
        }
        fn name(&self) -> &'static str {
            "Broken"
        }
        fn start(&self, _: &EventBus) -> Result<Option<String>, String> {
            Err("boom".into())
        }
    }

    #[test]
    fn lifecycle_and_events() {
        let bus = EventBus::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        bus.subscribe(move |e| {
            if let AppEvent::ServiceChanged { info } = e {
                ev.lock().unwrap().push((info.id, info.state.clone()));
            }
        });

        let echo = Arc::new(Echo(AtomicBool::new(false)));
        let mut reg = default_registry();
        reg.register(echo.clone());
        reg.register(Arc::new(Broken));
        reg.start_all(&bus);

        assert!(echo.0.load(Ordering::SeqCst));
        assert_eq!(reg.state("echo"), Some(ServiceState::Running));
        assert!(matches!(
            reg.state("broken"),
            Some(ServiceState::Failed { .. })
        ));
        assert!(matches!(
            reg.state("python"),
            Some(ServiceState::Unavailable { .. })
        ));
        assert_eq!(reg.list().len(), 5);

        reg.stop_all(&bus);
        assert!(!echo.0.load(Ordering::SeqCst));
        assert_eq!(reg.state("echo"), Some(ServiceState::Stopped));
        // unavailable services are not "stopped"
        assert!(matches!(
            reg.state("ai"),
            Some(ServiceState::Unavailable { .. })
        ));
        assert_eq!(events.lock().unwrap().len(), 6);
    }

    #[test]
    fn service_info_json_shape() {
        let info = ServiceInfo {
            id: "ai",
            name: "AI",
            state: ServiceState::Unavailable { reason: "x".into() },
        };
        let v = serde_json::to_value(info).unwrap();
        assert_eq!(v["state"], "unavailable");
        assert_eq!(v["reason"], "x");
    }
}
