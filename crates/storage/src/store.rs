use std::time::Duration;

use chrono::{DateTime, Utc};
use domain::{
    CallbackPayload, EventName, ParseInput, ResumeDetail, ResumeStatus, ResumeSummary, RunPayload,
    RunStatus, RunView, UploadedPayload, Verdict, VersionDetail, VersionSummary,
};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

pub struct Claimed {
    pub id: Uuid,
    pub token: Uuid,
    pub resume_id: Uuid,
    pub version_id: Uuid,
    pub name: EventName,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

pub enum RecordOutcome {
    Recorded,
    Replay,
    UnknownRun,
}

pub struct RemovedVersion {
    pub resume_id: Uuid,
    pub version_id: Uuid,
    pub object_key: String,
}

pub struct VersionFile {
    pub object_key: String,
    pub media_type: String,
}

impl Store {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(2))
            .connect(url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("../../migrations").run(&self.pool).await?;
        Ok(())
    }

    /// First upload. The caller picks both ids so the object key can be written
    /// before the row. The title is the filename until the user renames it.
    pub async fn create_resume(
        &self,
        user_id: Uuid,
        resume_id: Uuid,
        version_id: Uuid,
        filename: &str,
        media_type: &str,
        object_key: &str,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("insert into resumes (id, user_id, title) values ($1, $2, $3)")
            .bind(resume_id)
            .bind(user_id)
            .bind(filename)
            .execute(&mut *tx)
            .await?;
        insert_version(
            &mut tx,
            &NewVersion {
                user_id,
                resume_id,
                version_id,
                number: 1,
                filename,
                media_type,
                object_key,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Another file on an existing resume. Returns the new version number, or
    /// `None` when the resume is unknown or belongs to someone else. The resume
    /// row is locked so two uploads cannot take the same number.
    pub async fn add_version(
        &self,
        user_id: Uuid,
        resume_id: Uuid,
        version_id: Uuid,
        filename: &str,
        media_type: &str,
        object_key: &str,
    ) -> anyhow::Result<Option<i32>> {
        let mut tx = self.pool.begin().await?;
        let owned: Option<Uuid> =
            sqlx::query_scalar("select id from resumes where id = $1 and user_id = $2 for update")
                .bind(resume_id)
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
        if owned.is_none() {
            return Ok(None);
        }
        let number: i32 = sqlx::query_scalar(
            "select coalesce(max(number), 0) + 1 from versions where resume_id = $1",
        )
        .bind(resume_id)
        .fetch_one(&mut *tx)
        .await?;
        insert_version(
            &mut tx,
            &NewVersion {
                user_id,
                resume_id,
                version_id,
                number,
                filename,
                media_type,
                object_key,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(Some(number))
    }

    pub async fn list(&self, user_id: Uuid) -> anyhow::Result<Vec<ResumeSummary>> {
        let rows = sqlx::query(
            "select resumes.id, resumes.title, resumes.created_at,
                    versions.id as version_id, versions.number, versions.filename,
                    versions.status, versions.created_at as version_created_at
             from resumes
             join versions on versions.resume_id = resumes.id
             where resumes.user_id = $1
               and versions.number = (
                   select max(number) from versions as latest where latest.resume_id = resumes.id
               )
             order by versions.created_at desc, resumes.id desc",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(ResumeSummary {
                    id: row.try_get("id")?,
                    title: row.try_get("title")?,
                    created_at: row.try_get("created_at")?,
                    latest: VersionSummary {
                        id: row.try_get("version_id")?,
                        number: row.try_get("number")?,
                        filename: row.try_get("filename")?,
                        status: status(row.try_get("status")?)?,
                        created_at: row.try_get("version_created_at")?,
                    },
                })
            })
            .collect()
    }

    pub async fn detail(&self, user_id: Uuid, id: Uuid) -> anyhow::Result<Option<ResumeDetail>> {
        let Some(row) =
            sqlx::query("select id, title, created_at from resumes where id = $1 and user_id = $2")
                .bind(id)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
        else {
            return Ok(None);
        };
        let versions = sqlx::query(
            "select id, number, filename, media_type, status, created_at, parse
             from versions
             where resume_id = $1
             order by number desc",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let runs = sqlx::query(
            "select runs.id, runs.version_id, runs.plugin_id, runs.status, runs.parse, runs.verdict
             from runs
             join versions on versions.id = runs.version_id
             where versions.resume_id = $1
             order by runs.plugin_id",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let mut details = Vec::with_capacity(versions.len());
        for version in &versions {
            let version_id: Uuid = version.try_get("id")?;
            let parse: Option<Json<ParseInput>> = version.try_get("parse")?;
            details.push(VersionDetail {
                id: version_id,
                number: version.try_get("number")?,
                filename: version.try_get("filename")?,
                media_type: version.try_get("media_type")?,
                status: status(version.try_get("status")?)?,
                created_at: version.try_get("created_at")?,
                parse: parse.map(|json| json.0),
                runs: runs
                    .iter()
                    .filter(|run| run.try_get::<Uuid, _>("version_id").ok() == Some(version_id))
                    .map(run_view)
                    .collect::<anyhow::Result<_>>()?,
            });
        }
        Ok(Some(ResumeDetail {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            created_at: row.try_get("created_at")?,
            versions: details,
        }))
    }

    /// Resolve a file only when both its resume and session owner match.
    pub async fn version_file(
        &self,
        user_id: Uuid,
        resume_id: Uuid,
        version_id: Uuid,
    ) -> anyhow::Result<Option<VersionFile>> {
        let row = sqlx::query(
            "select versions.object_key, versions.media_type
             from versions join resumes on resumes.id = versions.resume_id
             where resumes.user_id = $1 and resumes.id = $2 and versions.id = $3",
        )
        .bind(user_id)
        .bind(resume_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(VersionFile {
                object_key: row.try_get("object_key")?,
                media_type: row.try_get("media_type")?,
            })
        })
        .transpose()
    }

    pub async fn claim(&self) -> anyhow::Result<Option<Claimed>> {
        let token = Uuid::new_v4();
        let Some(row) = sqlx::query(
            "with next as (
                 select id from outbox
                 where dispatched_at is null
                   and available_at <= now()
                   and (claimed_at is null or claimed_at < now() - interval '30 seconds')
                 order by seq
                 for update skip locked
                 limit 1
             )
             update outbox as queued
             set claimed_at = now(), claim_token = $1
             from next
             where queued.id = next.id
             returning queued.id, queued.resume_id, queued.version_id, queued.name,
                       queued.payload, queued.created_at",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let name: String = row.try_get("name")?;
        Ok(Some(Claimed {
            id: row.try_get("id")?,
            token,
            resume_id: row.try_get("resume_id")?,
            version_id: row.try_get("version_id")?,
            name: EventName::parse(&name).ok_or_else(|| anyhow::anyhow!("unknown event {name}"))?,
            payload: row.try_get("payload")?,
            created_at: row.try_get("created_at")?,
        }))
    }

    pub async fn finish(&self, id: Uuid, token: Uuid) -> anyhow::Result<()> {
        let result = sqlx::query(
            "update outbox set dispatched_at = now()
             where id = $1 and claim_token = $2 and dispatched_at is null",
        )
        .bind(id)
        .bind(token)
        .execute(&self.pool)
        .await?;
        anyhow::ensure!(result.rows_affected() == 1, "event {id} was not claimed");
        Ok(())
    }

    pub async fn release(&self, id: Uuid, token: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "update outbox
             set claimed_at = null, claim_token = null, available_at = now() + interval '2 seconds'
             where id = $1 and claim_token = $2 and dispatched_at is null",
        )
        .bind(id)
        .bind(token)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Parse result and the `ResumeParsed` event commit together. A retry of
    /// `ResumeUploaded` does not write a second parse event.
    pub async fn save_parse(&self, version_id: Uuid, parse: &ParseInput) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let resume_id: Uuid = sqlx::query_scalar("select resume_id from versions where id = $1")
            .bind(version_id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query(
            "update versions set status = 'parsed', parse = $2
             where id = $1 and status = 'uploaded'",
        )
        .bind(version_id)
        .bind(Json(parse))
        .execute(&mut *tx)
        .await?;
        let already = sqlx::query(
            "select 1 as present from outbox where version_id = $1 and name = 'ResumeParsed' limit 1",
        )
        .bind(version_id)
        .fetch_optional(&mut *tx)
        .await?;
        if already.is_none() {
            let payload = serde_json::to_value(domain::ParsedPayload {
                parse: parse.clone(),
            })?;
            enqueue(
                &mut tx,
                resume_id,
                version_id,
                EventName::ResumeParsed,
                &payload,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn fanout(&self, version_id: Uuid, plugin_ids: &[&str]) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let resume_id: Uuid = sqlx::query_scalar("select resume_id from versions where id = $1")
            .bind(version_id)
            .fetch_one(&mut *tx)
            .await?;
        let mut inserted = false;
        for plugin_id in plugin_ids {
            let run_id = Uuid::new_v4();
            let result = sqlx::query(
                "insert into runs (id, version_id, plugin_id, status)
                 values ($1, $2, $3, 'requested')
                 on conflict (version_id, plugin_id) do nothing",
            )
            .bind(run_id)
            .bind(version_id)
            .bind(plugin_id)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() == 1 {
                inserted = true;
                let payload = serde_json::to_value(RunPayload {
                    run_id,
                    plugin_id: (*plugin_id).to_owned(),
                })?;
                enqueue(
                    &mut tx,
                    resume_id,
                    version_id,
                    EventName::AtsRunRequested,
                    &payload,
                )
                .await?;
            }
        }
        if inserted {
            sqlx::query(
                "update versions set status = 'running' where id = $1 and status = 'parsed'",
            )
            .bind(version_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// The object a plugin re-reads. This is not the verification parse.
    pub async fn file_of(&self, version_id: Uuid) -> anyhow::Result<(String, String)> {
        let row = sqlx::query("select object_key, media_type from versions where id = $1")
            .bind(version_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((row.try_get("object_key")?, row.try_get("media_type")?))
    }

    /// Deletes one user's rows and returns the object keys the caller must
    /// delete. Another user's resumes are left in place.
    pub async fn delete_user(&self, user_id: Uuid) -> anyhow::Result<Vec<RemovedVersion>> {
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query(
            "select versions.resume_id, versions.id, versions.object_key
             from versions
             join resumes on resumes.id = versions.resume_id
             where resumes.user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        let removed = rows
            .iter()
            .map(|row| {
                Ok(RemovedVersion {
                    resume_id: row.try_get("resume_id")?,
                    version_id: row.try_get("id")?,
                    object_key: row.try_get("object_key")?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        sqlx::query(
            "delete from callback_keys
             where run_id in (
                 select runs.id from runs
                 join versions on versions.id = runs.version_id
                 join resumes on resumes.id = versions.resume_id
                 where resumes.user_id = $1
             )",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from outbox
             where resume_id in (select id from resumes where user_id = $1)",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from runs
             where version_id in (
                 select versions.id from versions
                 join resumes on resumes.id = versions.resume_id
                 where resumes.user_id = $1
             )",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "delete from versions
             where resume_id in (select id from resumes where user_id = $1)",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("delete from resumes where user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(removed)
    }

    pub async fn record_callback(
        &self,
        plugin_id: &str,
        run_id: Uuid,
        idempotency_key: &str,
        parse: &ParseInput,
        verdict: &Verdict,
    ) -> anyhow::Result<RecordOutcome> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "insert into callback_keys (idempotency_key, plugin_id, run_id)
             values ($1, $2, $3)
             on conflict do nothing",
        )
        .bind(idempotency_key)
        .bind(plugin_id)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(RecordOutcome::Replay);
        }
        let row = sqlx::query(
            "update runs set status = 'complete', parse = $3, verdict = $4
             where id = $1 and plugin_id = $2 and status = 'requested'
             returning version_id",
        )
        .bind(run_id)
        .bind(plugin_id)
        .bind(Json(parse))
        .bind(Json(verdict))
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            let exists: Option<i32> =
                sqlx::query_scalar("select 1 from runs where id = $1 and plugin_id = $2")
                    .bind(run_id)
                    .bind(plugin_id)
                    .fetch_optional(&self.pool)
                    .await?;
            return Ok(if exists.is_some() {
                RecordOutcome::Replay
            } else {
                RecordOutcome::UnknownRun
            });
        };
        let version_id: Uuid = row.try_get("version_id")?;
        let resume_id: Uuid = sqlx::query_scalar("select resume_id from versions where id = $1")
            .bind(version_id)
            .fetch_one(&mut *tx)
            .await?;
        let payload = serde_json::to_value(CallbackPayload {
            run_id,
            plugin_id: plugin_id.to_owned(),
            parse: parse.clone(),
            verdict: verdict.clone(),
        })?;
        enqueue(
            &mut tx,
            resume_id,
            version_id,
            EventName::AtsCallbackReceived,
            &payload,
        )
        .await?;
        sqlx::query(
            "update versions set status = 'complete'
             where id = $1
               and not exists (
                   select 1 from runs where version_id = $1 and status = 'requested'
               )",
        )
        .bind(version_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(RecordOutcome::Recorded)
    }
}

struct NewVersion<'a> {
    user_id: Uuid,
    resume_id: Uuid,
    version_id: Uuid,
    number: i32,
    filename: &'a str,
    media_type: &'a str,
    object_key: &'a str,
}

async fn insert_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version: &NewVersion<'_>,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into versions (id, resume_id, number, filename, media_type, object_key, status)
         values ($1, $2, $3, $4, $5, $6, 'uploaded')",
    )
    .bind(version.version_id)
    .bind(version.resume_id)
    .bind(version.number)
    .bind(version.filename)
    .bind(version.media_type)
    .bind(version.object_key)
    .execute(&mut **tx)
    .await?;
    let payload = serde_json::to_value(UploadedPayload {
        user_id: version.user_id,
        resume_id: version.resume_id,
        version_id: version.version_id,
        object_key: version.object_key.to_owned(),
        filename: version.filename.to_owned(),
        media_type: version.media_type.to_owned(),
    })?;
    enqueue(
        tx,
        version.resume_id,
        version.version_id,
        EventName::ResumeUploaded,
        &payload,
    )
    .await
}

fn run_view(run: &sqlx::postgres::PgRow) -> anyhow::Result<RunView> {
    let parse: Option<Json<ParseInput>> = run.try_get("parse")?;
    let verdict: Option<Json<Verdict>> = run.try_get("verdict")?;
    Ok(RunView {
        id: run.try_get("id")?,
        plugin_id: run.try_get("plugin_id")?,
        status: run_status(run.try_get("status")?)?,
        parse: parse.map(|json| json.0),
        verdict: verdict.map(|json| json.0),
    })
}

async fn enqueue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resume_id: Uuid,
    version_id: Uuid,
    name: EventName,
    payload: &impl Serialize,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into outbox (id, resume_id, version_id, name, payload) values ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind(resume_id)
    .bind(version_id)
    .bind(name.as_str())
    .bind(Json(payload))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn status(value: String) -> anyhow::Result<ResumeStatus> {
    ResumeStatus::parse(&value).ok_or_else(|| anyhow::anyhow!("bad resume status {value}"))
}

fn run_status(value: String) -> anyhow::Result<RunStatus> {
    RunStatus::parse(&value).ok_or_else(|| anyhow::anyhow!("bad run status {value}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use testcontainers_modules::postgres::Postgres;
    use testcontainers_modules::testcontainers::ImageExt;
    use testcontainers_modules::testcontainers::runners::AsyncRunner;

    use super::*;

    /// Postgres for one test. Dropping it removes the container.
    struct Ephemeral {
        store: Store,
        _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    }

    impl std::ops::Deref for Ephemeral {
        type Target = Store;

        fn deref(&self) -> &Self::Target {
            &self.store
        }
    }

    impl Ephemeral {
        async fn start() -> anyhow::Result<Self> {
            let container = Postgres::default().with_tag("17-alpine").start().await?;
            let host = container.get_host().await?;
            let port = container.get_host_port_ipv4(5432).await?;
            let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
            let store = Store::connect(&url).await?;
            store.migrate().await?;
            Ok(Self {
                store,
                _container: container,
            })
        }
    }

    fn empty_verdict(score: u8, summary: &str) -> Verdict {
        Verdict {
            kept: vec!["email".into()],
            dropped: vec![],
            score,
            summary: summary.to_owned(),
            factors: vec![],
        }
    }

    #[tokio::test]
    async fn two_workers_claim_different_rows() {
        let store = Ephemeral::start().await.unwrap();
        let user = domain::DEMO_USER_ID;
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        store
            .create_resume(
                user,
                first,
                Uuid::new_v4(),
                "a.pdf",
                domain::PDF,
                "users/demo/resumes/a/versions/1",
            )
            .await
            .unwrap();
        store
            .create_resume(
                user,
                second,
                Uuid::new_v4(),
                "b.pdf",
                domain::PDF,
                "users/demo/resumes/b/versions/1",
            )
            .await
            .unwrap();
        let store_a = store.clone();
        let store_b = store.clone();
        let (left, right) = tokio::join!(store_a.claim(), store_b.claim());
        let left = left.unwrap().unwrap();
        let right = right.unwrap().unwrap();
        assert_ne!(left.id, right.id);
        let ids = [left.resume_id, right.resume_id];
        assert!(ids.contains(&first) && ids.contains(&second));
        assert!(store.claim().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_replayed_key_does_not_write_a_second_verdict() {
        let store = Ephemeral::start().await.unwrap();
        let user = domain::DEMO_USER_ID;
        let resume_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        store
            .create_resume(
                user,
                resume_id,
                version_id,
                "a.pdf",
                domain::PDF,
                "users/demo/resumes/a/versions/1",
            )
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        store.save_parse(version_id, &parse).await.unwrap();
        store.fanout(version_id, &["stub"]).await.unwrap();
        let claimed = store.claim().await.unwrap().unwrap();
        store.finish(claimed.id, claimed.token).await.unwrap();
        let run = store.claim().await.unwrap().unwrap();
        store.finish(run.id, run.token).await.unwrap();
        let requested = store.claim().await.unwrap().unwrap();
        let payload: RunPayload = serde_json::from_value(requested.payload).unwrap();
        store.finish(requested.id, requested.token).await.unwrap();
        let verdict = empty_verdict(65, "Score 65 of 100. Keep the email in the body.");
        let shared = Arc::new(store);
        let first = shared.clone();
        let second = shared.clone();
        let key = payload.run_id.to_string();
        let parse = parse.clone();
        let (left, right) = tokio::join!(
            first.record_callback("stub", payload.run_id, &key, &parse, &verdict),
            second.record_callback("stub", payload.run_id, &key, &parse, &verdict),
        );
        let outcomes = [left.unwrap(), right.unwrap()];
        assert!(
            outcomes
                .iter()
                .any(|outcome| matches!(outcome, RecordOutcome::Recorded))
        );
        assert!(
            outcomes
                .iter()
                .any(|outcome| matches!(outcome, RecordOutcome::Replay))
        );
        let detail = shared.detail(user, resume_id).await.unwrap().unwrap();
        assert_eq!(detail.versions.len(), 1);
        let version = &detail.versions[0];
        assert_eq!(version.status, ResumeStatus::Complete);
        assert!(version.parse.is_some());
        assert_eq!(version.runs.len(), 1);
        assert!(version.runs[0].parse.is_some());
        let stored = version.runs[0].verdict.as_ref().unwrap();
        assert_eq!(stored.score, 65);
        assert_eq!(
            stored.summary,
            "Score 65 of 100. Keep the email in the body."
        );
        assert!(
            shared
                .detail(Uuid::new_v4(), resume_id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn deleting_one_user_leaves_the_other() {
        let store = Ephemeral::start().await.unwrap();
        let ada = Uuid::new_v4();
        let grace = Uuid::new_v4();
        let ada_resume = Uuid::new_v4();
        let ada_version = Uuid::new_v4();
        let grace_resume = Uuid::new_v4();
        store
            .create_resume(
                ada,
                ada_resume,
                ada_version,
                "ada.pdf",
                domain::PDF,
                "users/ada/resumes/a/versions/1",
            )
            .await
            .unwrap();
        store
            .create_resume(
                grace,
                grace_resume,
                Uuid::new_v4(),
                "grace.pdf",
                domain::PDF,
                "users/grace/resumes/g/versions/1",
            )
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        store.save_parse(ada_version, &parse).await.unwrap();
        store.fanout(ada_version, &["stub"]).await.unwrap();
        let removed = store.delete_user(ada).await.unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].resume_id, ada_resume);
        assert_eq!(removed[0].version_id, ada_version);
        assert_eq!(removed[0].object_key, "users/ada/resumes/a/versions/1");
        assert!(store.detail(ada, ada_resume).await.unwrap().is_none());
        let pending = store.claim().await.unwrap().unwrap();
        assert_eq!(pending.resume_id, grace_resume);
        assert!(store.claim().await.unwrap().is_none());
        let kept = store.detail(grace, grace_resume).await.unwrap().unwrap();
        assert_eq!(kept.title, "grace.pdf");
        assert_eq!(kept.versions.len(), 1);
        assert!(kept.versions[0].parse.is_none());
    }

    #[tokio::test]
    async fn a_second_version_keeps_the_first_result() {
        let store = Ephemeral::start().await.unwrap();
        let user = domain::DEMO_USER_ID;
        let resume_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        store
            .create_resume(
                user,
                resume_id,
                first,
                "designed.docx",
                domain::DOCX,
                "users/demo/resumes/a/versions/1",
            )
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Structural);
        store.save_parse(first, &parse).await.unwrap();
        store.fanout(first, &["stub"]).await.unwrap();
        let run_id = store
            .detail(user, resume_id)
            .await
            .unwrap()
            .unwrap()
            .versions[0]
            .runs[0]
            .id;
        let verdict = empty_verdict(15, "Score 15 of 100. Replace the table.");
        store
            .record_callback("stub", run_id, &run_id.to_string(), &parse, &verdict)
            .await
            .unwrap();

        let second = Uuid::new_v4();
        let number = store
            .add_version(
                user,
                resume_id,
                second,
                "plain.pdf",
                domain::PDF,
                "users/demo/resumes/a/versions/2",
            )
            .await
            .unwrap();
        assert_eq!(number, Some(2));
        assert!(
            store
                .add_version(
                    Uuid::new_v4(),
                    resume_id,
                    Uuid::new_v4(),
                    "x.pdf",
                    domain::PDF,
                    "nope",
                )
                .await
                .unwrap()
                .is_none()
        );
        let second_parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        store.save_parse(second, &second_parse).await.unwrap();
        store.fanout(second, &["stub"]).await.unwrap();

        let detail = store.detail(user, resume_id).await.unwrap().unwrap();
        assert_eq!(detail.versions.len(), 2);
        assert_eq!(detail.versions[0].id, second);
        assert_eq!(detail.versions[0].number, 2);
        assert_eq!(detail.versions[0].filename, "plain.pdf");
        assert_eq!(detail.versions[0].status, ResumeStatus::Running);
        assert!(detail.versions[0].runs[0].verdict.is_none());
        assert_eq!(detail.versions[1].id, first);
        assert_eq!(detail.versions[1].status, ResumeStatus::Complete);
        assert_eq!(
            detail.versions[1].runs[0].verdict.as_ref().unwrap().score,
            15
        );
        let listed = store.list(user).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].latest.filename, "plain.pdf");
        assert_eq!(listed[0].latest.number, 2);
    }
}
