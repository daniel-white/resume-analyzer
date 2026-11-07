//! Private ATS urls. The fleet plugins do not score in this process. The worker
//! hands each file to the Python fake (`python3 -m providers --all`), and that
//! process posts the callback. Unset, those plugins use the loopback ports.
//! `ATS_PROVIDERS=id=url` overrides a port (the chart uses ClusterIP names).
//! `ATS_PROVIDERS=stub` keeps the compiled stub in process.

use std::collections::HashMap;
use std::sync::Arc;

use domain::Ats;

pub struct Fleet {
    pub plugins: Vec<Arc<dyn Ats>>,
    pub endpoints: HashMap<String, String>,
}

/// `python3 -m providers --all`. Not routed. Same ports as `just dev`.
const LOCAL_PYTHON: &str = "\
greenline=http://127.0.0.1:8101,\
levr=http://127.0.0.1:8102,\
workdaye=http://127.0.0.1:8103,\
ash=http://127.0.0.1:8104,\
smart_resume=http://127.0.0.1:8105,\
indy_hackers_hiring=http://127.0.0.1:8106";

pub fn fleet_from_env(secret: String) -> anyhow::Result<Fleet> {
    let spec = std::env::var("ATS_PROVIDERS").ok();
    fleet_from_spec(secret, spec.as_deref())
}

pub fn fleet_from_spec(secret: String, spec: Option<&str>) -> anyhow::Result<Fleet> {
    match spec.map(str::trim).filter(|spec| !spec.is_empty()) {
        None => registered(secret, LOCAL_PYTHON),
        Some("stub" | "compiled") => Ok(Fleet {
            plugins: plugins::compiled(secret),
            endpoints: HashMap::new(),
        }),
        Some(spec) => registered(secret, spec),
    }
}

/// Plugins are the compiled fleet. The url is where the worker posts the file.
fn registered(secret: String, spec: &str) -> anyhow::Result<Fleet> {
    let available = plugins::fleet(secret);
    let mut plugins = Vec::new();
    let mut endpoints = HashMap::new();
    for item in spec.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let Some((id, url)) = item.split_once('=') else {
            anyhow::bail!("ATS_PROVIDERS entry {item:?} is not id=url");
        };
        let id = id.trim();
        let url = url.trim().trim_end_matches('/');
        let Some(plugin) = available.iter().find(|plugin| plugin.id() == id) else {
            anyhow::bail!("unknown ats provider {id}");
        };
        if url.is_empty() || !(url.starts_with("http://") || url.starts_with("https://")) {
            anyhow::bail!("provider {id} url must be http(s)");
        }
        if endpoints.values().any(|existing| existing == url) {
            anyhow::bail!("provider {id} reuses a url");
        }
        endpoints.insert(id.to_owned(), url.to_owned());
        plugins.push(Arc::clone(plugin));
    }
    if plugins.is_empty() {
        anyhow::bail!("ATS_PROVIDERS is empty");
    }
    Ok(Fleet { plugins, endpoints })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_spec_uses_the_python_fleet() {
        let fleet = fleet_from_spec("secret".into(), None).unwrap();
        assert_eq!(fleet.plugins.len(), 6);
        assert_eq!(
            fleet
                .plugins
                .iter()
                .map(|plugin| plugin.id())
                .collect::<Vec<_>>(),
            [
                "greenline",
                "levr",
                "workdaye",
                "ash",
                "smart_resume",
                "indy_hackers_hiring"
            ]
        );
        assert_eq!(
            fleet.endpoints.get("greenline").map(String::as_str),
            Some("http://127.0.0.1:8101")
        );
        assert_eq!(
            fleet.endpoints.get("ash").map(String::as_str),
            Some("http://127.0.0.1:8104")
        );
        assert_eq!(fleet.plugins[0].secret(), "secret");
        assert_eq!(fleet.plugins[4].name(), "SmartResume");
        assert_eq!(fleet.plugins[4].secret(), "secret");
        assert_eq!(fleet.endpoints["smart_resume"], "http://127.0.0.1:8105");
        assert_eq!(fleet.plugins[5].name(), "IndyHackersHiring");
        assert_eq!(fleet.plugins[5].secret(), "secret");
        assert_eq!(
            fleet.endpoints["indy_hackers_hiring"],
            "http://127.0.0.1:8106"
        );
        assert!(fleet.plugins[3].subtitle().contains("Simulated"));
    }

    #[test]
    fn stub_spec_keeps_the_compiled_stub() {
        let fleet = fleet_from_spec("secret".into(), Some("stub")).unwrap();
        assert!(fleet.endpoints.is_empty());
        assert_eq!(fleet.plugins.len(), 1);
        assert_eq!(fleet.plugins[0].id(), "stub");
    }

    #[test]
    fn a_private_url_registers_that_provider() {
        let fleet = fleet_from_spec(
            "secret".into(),
            Some("greenline=http://127.0.0.1:8101/, ash=http://127.0.0.1:8104"),
        )
        .unwrap();
        assert_eq!(
            fleet.endpoints.get("greenline").map(String::as_str),
            Some("http://127.0.0.1:8101")
        );
        assert_eq!(fleet.plugins.len(), 2);
        assert_eq!(fleet.plugins[0].id(), "greenline");
        assert_eq!(fleet.plugins[1].name(), "Ash");
        assert!(fleet.plugins[1].subtitle().contains("Simulated"));
        assert_eq!(fleet.plugins[0].secret(), "secret");
    }

    #[test]
    fn a_public_name_or_a_bad_url_is_rejected() {
        let Err(err) = fleet_from_spec("s".into(), Some("greenhouse=http://127.0.0.1:1")) else {
            panic!("expected an error");
        };
        assert!(err.to_string().contains("unknown"));
        assert!(fleet_from_spec("s".into(), Some("greenline=ftp://127.0.0.1:1")).is_err());
        assert!(
            fleet_from_spec(
                "s".into(),
                Some("greenline=http://127.0.0.1:1,levr=http://127.0.0.1:1"),
            )
            .is_err()
        );
    }
}
