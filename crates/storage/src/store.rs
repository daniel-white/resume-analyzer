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

pub struct AnalysisBatch {
    pub resume_id: Uuid,
    pub version_id: Uuid,
    pub email: String,
    pub filename: String,
    pub number: i32,
    pub runs: Vec<(Uuid, String, Verdict)>,
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

    /// Claim a version once the oldest pending result's batching window expires.
    /// Include all its ready results, including arrivals inside that window.
    pub async fn claim_analysis_batch(&self) -> anyhow::Result<Option<AnalysisBatch>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "select versions.id, resumes.id as resume_id, users.email, versions.filename, versions.number
             from versions
             join resumes on resumes.id = versions.resume_id
             join users on users.id = resumes.user_id
             where (versions.notification_lease_until is null or versions.notification_lease_until < now())
               and exists (select 1 from runs where runs.version_id = versions.id
                 and notification_due_at <= now() and notified_at is null)
             order by versions.created_at
             for update of versions skip locked limit 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let version_id: Uuid = row.try_get("id")?;
        sqlx::query("update versions set notification_lease_until = now() + interval '60 seconds' where id = $1")
            .bind(version_id).execute(&mut *tx).await?;
        let runs = sqlx::query("select id, plugin_id, verdict from runs where version_id = $1 and notification_due_at is not null and notified_at is null order by plugin_id")
            .bind(version_id).fetch_all(&mut *tx).await?;
        let batch = AnalysisBatch {
            resume_id: row.try_get("resume_id")?,
            version_id,
            email: row.try_get("email")?,
            filename: row.try_get("filename")?,
            number: row.try_get("number")?,
            runs: runs
                .iter()
                .map(|row| {
                    Ok((
                        row.try_get("id")?,
                        row.try_get("plugin_id")?,
                        row.try_get::<Json<Verdict>, _>("verdict")?.0,
                    ))
                })
                .collect::<anyhow::Result<_>>()?,
        };
        tx.commit().await?;
        Ok(Some(batch))
    }

    pub async fn finish_analysis_batch(
        &self,
        batch: &AnalysisBatch,
        sent: bool,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        if sent {
            let ids: Vec<Uuid> = batch.runs.iter().map(|(id, _, _)| *id).collect();
            sqlx::query("update runs set notified_at = now() where id = any($1)")
                .bind(ids)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("update versions set notification_lease_until = null where id = $1")
            .bind(batch.version_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
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
                "update versions set status = 'running' where id = $1 and status in ('parsed', 'complete')",
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

    /// Deletes an owned resume or one version, including its results and queued
    /// work. Locking the resume serializes deletion with version uploads.
    /// Deleting the last version also removes the empty resume.
    pub async fn delete_resume(
        &self,
        user_id: Uuid,
        resume_id: Uuid,
        version_id: Option<Uuid>,
    ) -> anyhow::Result<Option<Vec<RemovedVersion>>> {
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
        let rows = sqlx::query(
            "select id, object_key from versions
             where resume_id = $1 and ($2::uuid is null or id = $2) for update",
        )
        .bind(resume_id)
        .bind(version_id)
        .fetch_all(&mut *tx)
        .await?;
        if version_id.is_some() && rows.is_empty() {
            return Ok(None);
        }
        let removed = rows
            .iter()
            .map(|row| {
                Ok(RemovedVersion {
                    resume_id,
                    version_id: row.try_get("id")?,
                    object_key: row.try_get("object_key")?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        for version in &removed {
            sqlx::query("delete from callback_keys where run_id in (select id from runs where version_id = $1)")
                .bind(version.version_id).execute(&mut *tx).await?;
            sqlx::query("delete from outbox where version_id = $1")
                .bind(version.version_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("delete from runs where version_id = $1")
                .bind(version.version_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("delete from versions where id = $1")
                .bind(version.version_id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("delete from resumes where id = $1 and not exists (select 1 from versions where resume_id = $1)")
            .bind(resume_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(removed))
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
            "update runs set status = 'complete', parse = $3, verdict = $4,
                 notification_due_at = now() + interval '8 seconds'
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
            report: None,
        }
    }

    #[tokio::test]
    async fn analysis_batches_wait_group_retry_and_do_not_repeat() {
        let store = Ephemeral::start().await.unwrap();
        let user = Uuid::new_v4();
        sqlx::query("insert into users (id, email) values ($1, 'owner@example.com')")
            .bind(user)
            .execute(store.pool())
            .await
            .unwrap();
        let resume = Uuid::new_v4();
        let version = Uuid::new_v4();
        store
            .create_resume(user, resume, version, "resume.pdf", domain::PDF, "file")
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        store.save_parse(version, &parse).await.unwrap();
        store
            .fanout(version, &["greenline", "levr", "ash"])
            .await
            .unwrap();
        let detail = store.detail(user, resume).await.unwrap().unwrap();
        let runs = &detail.versions[0].runs;
        for run in runs.iter().filter(|run| run.plugin_id != "ash") {
            store
                .record_callback(
                    &run.plugin_id,
                    run.id,
                    &run.id.to_string(),
                    &parse,
                    &empty_verdict(85, "done"),
                )
                .await
                .unwrap();
        }
        assert!(store.claim_analysis_batch().await.unwrap().is_none());
        sqlx::query("update runs set notification_due_at = now() - interval '1 second' where plugin_id = 'greenline'")
            .execute(store.pool()).await.unwrap();
        let batch = store.claim_analysis_batch().await.unwrap().unwrap();
        assert_eq!(batch.email, "owner@example.com");
        assert_eq!(batch.resume_id, resume);
        assert_eq!(batch.version_id, version);
        assert_eq!(
            batch.runs.len(),
            2,
            "include nearby results even if their own window hasn't elapsed"
        );
        assert!(
            store.claim_analysis_batch().await.unwrap().is_none(),
            "leased by one sender"
        );
        store.finish_analysis_batch(&batch, false).await.unwrap();
        let retry = store.claim_analysis_batch().await.unwrap().unwrap();
        assert_eq!(retry.runs.len(), 2);
        store.finish_analysis_batch(&retry, true).await.unwrap();
        let greenline = runs
            .iter()
            .find(|run| run.plugin_id == "greenline")
            .unwrap();
        assert!(matches!(
            store
                .record_callback(
                    "greenline",
                    greenline.id,
                    "another-key",
                    &parse,
                    &empty_verdict(85, "done")
                )
                .await
                .unwrap(),
            RecordOutcome::Replay
        ));
        assert!(store.claim_analysis_batch().await.unwrap().is_none());
        let ash = runs.iter().find(|run| run.plugin_id == "ash").unwrap();
        store
            .record_callback(
                "ash",
                ash.id,
                &ash.id.to_string(),
                &parse,
                &empty_verdict(90, "later"),
            )
            .await
            .unwrap();
        sqlx::query("update runs set notification_due_at = now() - interval '1 second' where plugin_id = 'ash'")
            .execute(store.pool()).await.unwrap();
        let later = store.claim_analysis_batch().await.unwrap().unwrap();
        assert_eq!(later.runs.len(), 1);
        assert_eq!(later.runs[0].1, "ash");
        store.finish_analysis_batch(&later, false).await.unwrap();
        store.delete_resume(user, resume, None).await.unwrap();
        assert!(store.claim_analysis_batch().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn concurrent_fanout_adds_a_new_plugin_once_after_completion() {
        let store = Ephemeral::start().await.unwrap();
        let resume_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        store
            .create_resume(
                domain::DEMO_USER_ID,
                resume_id,
                version_id,
                "a.pdf",
                domain::PDF,
                "a",
            )
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        store.save_parse(version_id, &parse).await.unwrap();
        store.fanout(version_id, &["greenline"]).await.unwrap();
        let before = store
            .detail(domain::DEMO_USER_ID, resume_id)
            .await
            .unwrap()
            .unwrap();
        let original = before.versions[0].runs[0].id;
        let verdict = empty_verdict(65, "original result");
        store
            .record_callback(
                "greenline",
                original,
                &original.to_string(),
                &parse,
                &verdict,
            )
            .await
            .unwrap();
        let (left, right) = tokio::join!(
            store.fanout(version_id, &["greenline", "smart_resume"]),
            store.fanout(version_id, &["greenline", "smart_resume"]),
        );
        left.unwrap();
        right.unwrap();
        let detail = store
            .detail(domain::DEMO_USER_ID, resume_id)
            .await
            .unwrap()
            .unwrap();
        let version = &detail.versions[0];
        assert_eq!(version.status, ResumeStatus::Running);
        assert_eq!(version.runs.len(), 2);
        let kept = version.runs.iter().find(|run| run.id == original).unwrap();
        assert_eq!(kept.verdict.as_ref().unwrap().summary, "original result");
        let new = version
            .runs
            .iter()
            .find(|run| run.plugin_id == "smart_resume")
            .unwrap();
        assert_eq!(new.status, RunStatus::Requested);
        let count: i64 = sqlx::query_scalar("select count(*) from outbox where version_id = $1 and name = 'AtsRunRequested' and payload->>'plugin_id' = 'smart_resume'")
            .bind(version_id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(count, 1);
        store
            .record_callback(
                "smart_resume",
                new.id,
                &new.id.to_string(),
                &parse,
                &verdict,
            )
            .await
            .unwrap();
        let completed = store
            .detail(domain::DEMO_USER_ID, resume_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed.versions[0].status, ResumeStatus::Complete);
    }

    #[tokio::test]
    async fn deleting_versions_is_owned_and_removes_results_and_work() {
        let store = Ephemeral::start().await.unwrap();
        let owner = Uuid::new_v4();
        let resume = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        store
            .create_resume(owner, resume, first, "old.pdf", domain::PDF, "old-key")
            .await
            .unwrap();
        store
            .add_version(owner, resume, second, "new.pdf", domain::PDF, "new-key")
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        store.save_parse(second, &parse).await.unwrap();
        store.fanout(second, &["stub"]).await.unwrap();
        let detail = store.detail(owner, resume).await.unwrap().unwrap();
        let run_id = detail.versions[0].runs[0].id;
        store
            .record_callback(
                "stub",
                run_id,
                "delete-test-key",
                &parse,
                &empty_verdict(95, "done"),
            )
            .await
            .unwrap();
        assert!(
            store
                .delete_resume(Uuid::new_v4(), resume, None)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .delete_resume(owner, resume, Some(Uuid::new_v4()))
                .await
                .unwrap()
                .is_none()
        );
        let removed = store
            .delete_resume(owner, resume, Some(second))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].object_key, "new-key");
        let detail = store.detail(owner, resume).await.unwrap().unwrap();
        assert_eq!(detail.versions.len(), 1);
        assert_eq!(detail.versions[0].id, first);
        let remaining: i64 =
            sqlx::query_scalar("select count(*) from callback_keys where run_id = $1")
                .bind(run_id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(remaining, 0);
        let removed = store
            .delete_resume(owner, resume, Some(first))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(removed[0].object_key, "old-key");
        assert!(store.detail(owner, resume).await.unwrap().is_none());
        assert!(store.claim().await.unwrap().is_none());

        store
            .create_resume(owner, resume, first, "old.pdf", domain::PDF, "old-key")
            .await
            .unwrap();
        store
            .add_version(owner, resume, second, "new.pdf", domain::PDF, "new-key")
            .await
            .unwrap();
        assert_eq!(
            store
                .delete_resume(owner, resume, None)
                .await
                .unwrap()
                .unwrap()
                .len(),
            2
        );
        assert!(store.detail(owner, resume).await.unwrap().is_none());
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
