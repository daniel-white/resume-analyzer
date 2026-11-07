//! Process configuration. Defaults are for a port-forward to Minikube.
//! The chart overrides every value with in-cluster DNS.

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub bind: String,
    /// HMAC key for the session cookie. The demo default is not a production secret.
    pub auth_secret: String,
    /// Origin of the web app, used as the prefix of the link in the email.
    pub app_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env(
                "DATABASE_URL",
                "postgres://resume:resume@127.0.0.1:54329/resume",
            ),
            s3_endpoint: env("S3_ENDPOINT", "http://127.0.0.1:4566"),
            s3_region: env("S3_REGION", "us-east-1"),
            s3_bucket: env("S3_BUCKET", "resumes"),
            s3_access_key: env("S3_ACCESS_KEY", "test"),
            s3_secret_key: env("S3_SECRET_KEY", "test"),
            smtp_host: env("SMTP_HOST", "127.0.0.1"),
            smtp_port: env("SMTP_PORT", "1025")
                .parse()
                .expect("SMTP_PORT must be a port number"),
            bind: env("BIND", "0.0.0.0:8080"),
            auth_secret: env("AUTH_SECRET", "dev-only-auth-secret-change-me"),
            app_origin: env("APP_ORIGIN", "https://app.resume.local"),
        }
    }

    pub fn cookie_secure(&self) -> bool {
        self.app_origin.starts_with("https://")
    }

    /// Ports nothing listens on, for the degraded-health test.
    #[cfg(test)]
    pub fn unreachable() -> Self {
        Self {
            database_url: "postgres://resume:resume@127.0.0.1:1/resume".into(),
            s3_endpoint: "http://127.0.0.1:1".into(),
            s3_region: "us-east-1".into(),
            s3_bucket: "resumes".into(),
            s3_access_key: "test".into(),
            s3_secret_key: "test".into(),
            smtp_host: "127.0.0.1".into(),
            smtp_port: 1,
            bind: "127.0.0.1:0".into(),
            auth_secret: "dev-only-auth-secret-change-me".into(),
            app_origin: "https://app.resume.local".into(),
        }
    }
}

fn env(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_owned())
}
