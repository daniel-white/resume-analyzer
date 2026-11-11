//! Liveness of the three dependencies the API cannot run without.
//! A check is `ok` only when the service answered the way we will use it:
//! Postgres accepts a query, the bucket exists, SMTP sent its greeting.

use std::time::Duration;

use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use crate::config::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    Ok,
    Error,
}

#[derive(Debug, Serialize)]
pub struct Checks {
    pub postgres: Check,
    pub s3: Check,
    pub smtp: Check,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub status: Check,
    pub checks: Checks,
}

impl Report {
    pub fn from_checks(checks: Checks) -> Self {
        let status =
            if checks.postgres == Check::Ok && checks.s3 == Check::Ok && checks.smtp == Check::Ok {
                Check::Ok
            } else {
                Check::Error
            };
        Self { status, checks }
    }

    pub fn http_status(&self) -> axum::http::StatusCode {
        if self.status == Check::Ok {
            axum::http::StatusCode::OK
        } else {
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

pub async fn report_for(config: &Config, s3: &s3::Client) -> Report {
    let (postgres, s3_check, smtp) = tokio::join!(
        probe_postgres(&config.database_url),
        probe_s3(s3, &config.s3_bucket),
        probe_smtp(&config.smtp_host, config.smtp_port),
    );
    Report::from_checks(Checks {
        postgres,
        s3: s3_check,
        smtp,
    })
}

pub async fn s3_client(config: &Config) -> s3::Client {
    let auth = s3::Auth::Static(
        s3::Credentials::new(config.s3_access_key.clone(), config.s3_secret_key.clone())
            .expect("valid static credentials"),
    );

    s3::Client::builder(&config.s3_endpoint)
        .expect("valid endpoint")
        .region(config.s3_region.clone())
        .auth(auth)
        .addressing_style(s3::AddressingStyle::Path)
        .timeout(Duration::from_secs(2))
        .max_attempts(1)
        .build()
        .expect("valid client")
}

async fn probe_postgres(database_url: &str) -> Check {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect(database_url)
        .await;
    let Ok(pool) = pool else {
        return Check::Error;
    };
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => Check::Ok,
        Err(_) => Check::Error,
    }
}

async fn bucket_exists(s3: &s3::Client, bucket: &str) -> bool {
    s3.buckets().head(bucket).send().await.is_ok()
}

async fn probe_s3(s3: &s3::Client, bucket: &str) -> Check {
    if bucket_exists(s3, bucket).await {
        return Check::Ok;
    }
    // The bucket is created on the way up, so a readiness retry succeeds once
    // LocalStack is accepting requests. A bucket that already exists is fine.
    let _ = s3.buckets().create(bucket).send().await;
    if bucket_exists(s3, bucket).await {
        Check::Ok
    } else {
        Check::Error
    }
}

async fn probe_smtp(host: &str, port: u16) -> Check {
    let connect =
        tokio::time::timeout(Duration::from_secs(2), TcpStream::connect((host, port))).await;
    let Ok(Ok(mut stream)) = connect else {
        return Check::Error;
    };
    let mut buf = [0_u8; 128];
    let read = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf)).await;
    match read {
        Ok(Ok(n)) if buf[..n].starts_with(b"220") => Check::Ok,
        _ => Check::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checks(postgres: Check, s3: Check, smtp: Check) -> Checks {
        Checks { postgres, s3, smtp }
    }

    #[test]
    fn all_reachable_is_ok() {
        let report = Report::from_checks(checks(Check::Ok, Check::Ok, Check::Ok));
        assert_eq!(report.status, Check::Ok);
        assert_eq!(report.http_status(), axum::http::StatusCode::OK);
    }

    #[test]
    fn any_failure_degrades_the_report() {
        let report = Report::from_checks(checks(Check::Ok, Check::Error, Check::Ok));
        assert_eq!(report.status, Check::Error);
        assert_eq!(
            report.http_status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
