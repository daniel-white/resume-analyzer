//! Build metadata reported to the frontend, with no dependency on the database.

use axum::Json;
use domain::BuildInfo;

const IDENTITY_DEFAULT: &str = "VERGEN_IDEMPOTENT_OUTPUT";
const SHORT_SHA_LEN: usize = 7;

/// The API's version, commit, and whether this is a local development build.
#[must_use]
pub fn info() -> BuildInfo {
    let sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");
    let sha = if sha == IDENTITY_DEFAULT || sha.is_empty() {
        "unknown"
    } else {
        sha
    };

    let dev = match option_env!("DEV_BUILD") {
        Some("false") => false,
        Some("true") => true,
        _ => cfg!(debug_assertions) || sha == "unknown",
    };

    BuildInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: sha.chars().take(SHORT_SHA_LEN).collect(),
        dev,
    }
}

/// Axum handler for `GET /api/version`.
pub async fn route() -> Json<BuildInfo> {
    Json(info())
}

#[cfg(test)]
mod tests {
    use super::{SHORT_SHA_LEN, info};

    #[test]
    fn reports_a_semver_and_short_commit() {
        let info = info();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.version.split('.').count() >= 3);
        assert!(info.commit.len() <= SHORT_SHA_LEN);
        assert_ne!(info.commit, "");
    }
}
