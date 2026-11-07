//! Private ATS urls. `ATS_PROVIDERS=id=http://127.0.0.1:port` tells the worker
//! where to hand a file. The provider is not on the gateway. It posts the
//! callback itself. Unset, the compiled stub stays in process.

use std::collections::HashMap;
use std::sync::Arc;

use domain::{Ats, Factor, LayoutConfidence, ParseInput, ResumeFile, VendorRead, Verdict};

pub struct Fleet {
    pub plugins: Vec<Arc<dyn Ats>>,
    pub endpoints: HashMap<String, String>,
}

struct Kind {
    id: &'static str,
    name: &'static str,
    subtitle: &'static str,
}

const FLEET: &[Kind] = &[
    Kind {
        id: "greenline",
        name: "Greenline",
        subtitle: "Standard headings and plain text. Ignores the header. Simulated, modeled on publicly described parser behavior.",
    },
    Kind {
        id: "levr",
        name: "Levr",
        subtitle: "Exact skill tokens and bullet density. Simulated, modeled on publicly described parser behavior.",
    },
    Kind {
        id: "workdaye",
        name: "Workdaye",
        subtitle: "Contact in the body, dates as MM/YYYY. Simulated, modeled on publicly described parser behavior.",
    },
    Kind {
        id: "ash",
        name: "Ash",
        subtitle: "Keeps more structure, still drops images. Simulated, modeled on publicly described parser behavior.",
    },
];

struct RemoteAts {
    id: &'static str,
    name: &'static str,
    subtitle: &'static str,
    secret: String,
}

impl Ats for RemoteAts {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn subtitle(&self) -> &'static str {
        self.subtitle
    }

    fn secret(&self) -> &str {
        &self.secret
    }

    fn read(&self, _file: &ResumeFile<'_>) -> VendorRead {
        // The provider reads the file. This value is not what gets stored.
        let factors = vec![Factor {
            label: "could not read file".into(),
            delta: -50,
            advice: "The provider reads this file.".into(),
        }];
        VendorRead {
            parse: ParseInput::empty(LayoutConfidence::Inferred),
            verdict: Verdict {
                kept: Vec::new(),
                dropped: vec!["file".into()],
                score: 0,
                summary: "Score 0 of 100. The provider reads this file.".into(),
                factors,
            },
        }
    }
}

pub fn fleet_from_env(secret: String) -> anyhow::Result<Fleet> {
    let spec = std::env::var("ATS_PROVIDERS").ok();
    fleet_from_spec(secret, spec.as_deref())
}

pub fn fleet_from_spec(secret: String, spec: Option<&str>) -> anyhow::Result<Fleet> {
    let Some(spec) = spec.map(str::trim).filter(|spec| !spec.is_empty()) else {
        return Ok(Fleet {
            plugins: plugins::compiled(secret),
            endpoints: HashMap::new(),
        });
    };
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
        let Some(kind) = FLEET.iter().find(|kind| kind.id == id) else {
            anyhow::bail!("unknown ats provider {id}");
        };
        if url.is_empty() || !(url.starts_with("http://") || url.starts_with("https://")) {
            anyhow::bail!("provider {id} url must be http(s)");
        }
        if endpoints.values().any(|existing| existing == url) {
            anyhow::bail!("provider {id} reuses a url");
        }
        endpoints.insert(id.to_owned(), url.to_owned());
        plugins.push(Arc::new(RemoteAts {
            id: kind.id,
            name: kind.name,
            subtitle: kind.subtitle,
            secret: secret.clone(),
        }) as Arc<dyn Ats>);
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
    fn unset_spec_keeps_the_compiled_stub() {
        let fleet = fleet_from_spec("secret".into(), None).unwrap();
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
