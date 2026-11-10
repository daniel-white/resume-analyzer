use std::time::Duration;

use chrono::{DateTime, Utc};
use domain::{
    AnalyzeFeedbackPayload, CallbackPayload, EventName, FeedbackAnalysis, GenerateMarkdownPayload,
    PROFILE_SUMMARY_ID, ParseInput, ProfileSummary, ResumeDetail, ResumeMarkdown, ResumeStatus,
    ResumeSummary, RunPayload, RunStatus, RunView, SectionKind, SectionMap, SectionVote,
    UploadedPayload, Verdict, VersionDetail, VersionSummary,
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

    pub async fn stage_upload(
        &self,
        id: Uuid,
        filename: &str,
        media_type: &str,
        object_key: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("insert into pending_uploads (id, filename, media_type, object_key) values ($1, $2, $3, $4)")
            .bind(id).bind(filename).bind(media_type).bind(object_key).execute(&self.pool).await?;
        Ok(())
    }

    /// Expired bytes are deleted before their metadata, so failed deletes retry.
    pub async fn clean_pending_uploads(&self, objects: &dyn crate::Objects) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        // Prevent cleanup from deleting bytes during a concurrent verified claim.
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as("select id, object_key from pending_uploads where expires_at <= now() limit 100 for update skip locked")
                .fetch_all(&mut *tx)
                .await?;
        for (id, key) in rows {
            objects.delete(&key).await?;
            sqlx::query("delete from pending_uploads where id = $1 and expires_at <= now()")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
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

    /// Renames the set, not its uploaded files. Unknown and unowned ids look alike.
    pub async fn rename_resume(
        &self,
        user_id: Uuid,
        id: Uuid,
        title: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query("update resumes set title = $3 where id = $1 and user_id = $2")
            .bind(id)
            .bind(user_id)
            .bind(title)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
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
            "select id, number, filename, media_type, status, created_at, parse, sections,
                    profile_summary
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
        let votes = sqlx::query(
            "select feedback.version_id, feedback.section_id, feedback.vote
             from section_feedback feedback
             join versions on versions.id = feedback.version_id
             where versions.resume_id = $1 and feedback.user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        let mut details = Vec::with_capacity(versions.len());
        for version in &versions {
            let version_id: Uuid = version.try_get("id")?;
            let parse: Option<Json<ParseInput>> = version.try_get("parse")?;
            let mut sections: Option<SectionMap> = version
                .try_get::<Option<Json<SectionMap>>, _>("sections")?
                .map(|json| json.0);
            if let Some(map) = &mut sections {
                for section in &mut map.sections {
                    section.feedback = votes.iter().find_map(|vote| {
                        if vote.try_get::<Uuid, _>("version_id").ok() == Some(version_id)
                            && vote.try_get::<String, _>("section_id").ok().as_deref()
                                == Some(section.id.as_str())
                        {
                            match vote.try_get::<String, _>("vote").ok().as_deref() {
                                Some("up") => Some(SectionVote::Up),
                                Some("down") => Some(SectionVote::Down),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    });
                }
            }
            let mut profile_summary: Option<ProfileSummary> = version
                .try_get::<Option<Json<ProfileSummary>>, _>("profile_summary")?
                .map(|json| json.0);
            if let Some(summary) = &mut profile_summary {
                summary.feedback = votes.iter().find_map(|vote| {
                    if vote.try_get::<Uuid, _>("version_id").ok() == Some(version_id)
                        && vote.try_get::<String, _>("section_id").ok().as_deref()
                            == Some(PROFILE_SUMMARY_ID)
                    {
                        match vote.try_get::<String, _>("vote").ok().as_deref() {
                            Some("up") => Some(SectionVote::Up),
                            Some("down") => Some(SectionVote::Down),
                            _ => None,
                        }
                    } else {
                        None
                    }
                });
            }
            details.push(VersionDetail {
                id: version_id,
                number: version.try_get("number")?,
                filename: version.try_get("filename")?,
                media_type: version.try_get("media_type")?,
                status: status(version.try_get("status")?)?,
                created_at: version.try_get("created_at")?,
                parse: parse.map(|json| json.0),
                sections,
                profile_summary,
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

    /// Parse result, model readings, and the `ResumeParsed` event commit
    /// together. A retry of `ResumeUploaded` does not write a second parse event.
    pub async fn save_parse(
        &self,
        version_id: Uuid,
        parse: &ParseInput,
        sections: Option<&SectionMap>,
        profile_summary: Option<&ProfileSummary>,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let resume_id: Uuid = sqlx::query_scalar("select resume_id from versions where id = $1")
            .bind(version_id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query(
            "update versions set status = 'parsed', parse = $2, sections = $3,
                                    profile_summary = $4
             where id = $1 and status = 'uploaded'",
        )
        .bind(version_id)
        .bind(jsonb(parse)?)
        .bind(sections.map(|sections| jsonb(sections)).transpose()?)
        .bind(profile_summary.map(|summary| jsonb(summary)).transpose()?)
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

    /// Record, change, or clear one user's vote on a model reading. Returns
    /// false when the version is not the caller's or the reading is absent, so
    /// the API can answer 404 without leaking ownership.
    pub async fn vote_section(
        &self,
        user_id: Uuid,
        resume_id: Uuid,
        version_id: Uuid,
        section_id: &str,
        vote: Option<SectionVote>,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool.begin().await?;
        let owned: Option<Uuid> = sqlx::query_scalar(
            "select versions.id
             from versions join resumes on resumes.id = versions.resume_id
             where resumes.user_id = $1 and resumes.id = $2 and versions.id = $3
               and (
                   exists (
                       select 1
                       from jsonb_array_elements(
                           coalesce(versions.sections -> 'sections', '[]'::jsonb)
                       ) as section
                       where section ->> 'id' = $4
                   )
                   or ($4 = $5 and versions.profile_summary is not null)
               )",
        )
        .bind(user_id)
        .bind(resume_id)
        .bind(version_id)
        .bind(section_id)
        .bind(PROFILE_SUMMARY_ID)
        .fetch_optional(&mut *tx)
        .await?;
        if owned.is_none() {
            return Ok(false);
        }
        match vote {
            Some(vote) => {
                sqlx::query(
                    "insert into section_feedback (version_id, user_id, section_id, vote)
                     values ($1, $2, $3, $4)
                     on conflict (version_id, user_id, section_id)
                     do update set vote = excluded.vote, updated_at = now()",
                )
                .bind(version_id)
                .bind(user_id)
                .bind(section_id)
                .bind(vote_key(vote))
                .execute(&mut *tx)
                .await?;
            }
            None => {
                sqlx::query(
                    "delete from section_feedback
                     where version_id = $1 and user_id = $2 and section_id = $3",
                )
                .bind(version_id)
                .bind(user_id)
                .bind(section_id)
                .execute(&mut *tx)
                .await?;
            }
        }
        if vote == Some(SectionVote::Down) {
            self.enqueue_feedback_tx(&mut tx, user_id, resume_id, version_id, section_id)
                .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    /// Queue the diagnosis for one down vote. The markdown is built once per
    /// version: before it exists this queues `GenerateMarkdown`, and the
    /// markdown handler fans out to every down vote. Afterwards it queues
    /// `AnalyzeFeedback` for this vote directly.
    async fn enqueue_feedback_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
        resume_id: Uuid,
        version_id: Uuid,
        section_id: &str,
    ) -> anyhow::Result<()> {
        let has_markdown: bool = sqlx::query_scalar(
            "select resume_markdown is not null from versions where id = $1",
        )
        .bind(version_id)
        .fetch_one(&mut **tx)
        .await?;
        if !has_markdown {
            let pending: Option<Uuid> = sqlx::query_scalar(
                "select id from outbox
                 where version_id = $1 and name = $2 and dispatched_at is null
                 limit 1",
            )
            .bind(version_id)
            .bind(EventName::GenerateMarkdown.as_str())
            .fetch_optional(&mut **tx)
            .await?;
            if pending.is_none() {
                let payload = GenerateMarkdownPayload { version_id };
                enqueue(
                    tx,
                    resume_id,
                    version_id,
                    EventName::GenerateMarkdown,
                    &payload,
                )
                .await?;
            }
            return Ok(());
        }
        let target = feedback_target(tx, version_id, section_id).await?;
        let payload = AnalyzeFeedbackPayload {
            version_id,
            user_id,
            section_id: target.id,
            section_heading: target.heading,
            section_kind: target.kind,
        };
        enqueue(
            tx,
            resume_id,
            version_id,
            EventName::AnalyzeFeedback,
            &payload,
        )
        .await
    }

    /// After the markdown is stored, diagnose every current down vote on the
    /// version. Called by the `GenerateMarkdown` handler.
    pub async fn enqueue_feedback_analyses(&self, version_id: Uuid) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let resume_id: Uuid =
            sqlx::query_scalar("select resume_id from versions where id = $1 for update")
                .bind(version_id)
                .fetch_one(&mut *tx)
                .await?;
        let rows = sqlx::query(
            "select feedback.user_id, feedback.section_id
             from section_feedback feedback
             where feedback.version_id = $1 and feedback.vote = 'down'
               and not exists (
                   select 1 from feedback_analysis analysis
                   where analysis.version_id = feedback.version_id
                     and analysis.user_id = feedback.user_id
                     and analysis.section_id = feedback.section_id
                     and analysis.created_at >= feedback.updated_at
               )
             order by feedback.updated_at",
        )
        .bind(version_id)
        .fetch_all(&mut *tx)
        .await?;
        for row in rows {
            let user_id: Uuid = row.try_get("user_id")?;
            let section_id: String = row.try_get("section_id")?;
            let target = feedback_target(&mut tx, version_id, &section_id).await?;
            let payload = AnalyzeFeedbackPayload {
                version_id,
                user_id,
                section_id: target.id,
                section_heading: target.heading,
                section_kind: target.kind,
            };
            enqueue(
                &mut tx,
                resume_id,
                version_id,
                EventName::AnalyzeFeedback,
                &payload,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Store the markdown rendering. Idempotent: the first build wins, so a
    /// retried `GenerateMarkdown` does not replace the text a diagnosis used.
    pub async fn save_resume_markdown(
        &self,
        version_id: Uuid,
        markdown: &ResumeMarkdown,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "update versions set resume_markdown = $2
             where id = $1 and resume_markdown is null",
        )
        .bind(version_id)
        .bind(jsonb(markdown)?)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// The stored markdown, when it has been built.
    pub async fn resume_markdown(
        &self,
        version_id: Uuid,
    ) -> anyhow::Result<Option<ResumeMarkdown>> {
        let row = sqlx::query("select resume_markdown from versions where id = $1")
            .bind(version_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(row
            .try_get::<Option<Json<ResumeMarkdown>>, _>("resume_markdown")?
            .map(|json| json.0))
    }

    /// The parse and section reading a markdown build needs. Both are written
    /// together by `save_parse`, so either both are present or neither is.
    pub async fn parse_and_sections(
        &self,
        version_id: Uuid,
    ) -> anyhow::Result<Option<(ParseInput, SectionMap)>> {
        let row = sqlx::query("select parse, sections from versions where id = $1")
            .bind(version_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let parse: Option<Json<ParseInput>> = row.try_get("parse")?;
        let sections: Option<Json<SectionMap>> = row.try_get("sections")?;
        Ok(match (parse, sections) {
            (Some(parse), Some(sections)) => Some((parse.0, sections.0)),
            _ => None,
        })
    }

    /// Store one model diagnosis. Every thumbs down keeps its own row.
    pub async fn save_feedback_analysis(
        &self,
        payload: &AnalyzeFeedbackPayload,
        engine: &str,
        model: &str,
        diagnosis: &str,
    ) -> anyhow::Result<FeedbackAnalysis> {
        let id = Uuid::new_v4();
        let created_at: DateTime<Utc> = sqlx::query_scalar(
            "insert into feedback_analysis
                 (id, version_id, user_id, section_id, section_heading, section_kind,
                  model, engine, diagnosis)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             returning created_at",
        )
        .bind(id)
        .bind(payload.version_id)
        .bind(payload.user_id)
        .bind(&payload.section_id)
        .bind(&payload.section_heading)
        .bind(payload.section_kind.label().to_lowercase())
        .bind(model)
        .bind(engine)
        .bind(diagnosis)
        .fetch_one(&self.pool)
        .await?;
        Ok(FeedbackAnalysis {
            id,
            version_id: payload.version_id,
            user_id: payload.user_id,
            section_id: payload.section_id.clone(),
            section_heading: payload.section_heading.clone(),
            section_kind: payload.section_kind,
            model: model.to_owned(),
            engine: engine.to_owned(),
            diagnosis: diagnosis.to_owned(),
            created_at,
        })
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
        .bind(jsonb(parse)?)
        .bind(jsonb(verdict)?)
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
    .bind(jsonb(payload)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn status(value: String) -> anyhow::Result<ResumeStatus> {
    ResumeStatus::parse(&value).ok_or_else(|| anyhow::anyhow!("bad resume status {value}"))
}

/// The heading and kind a diagnosis needs for one rejected reading. The
/// generated impression is not in the section map, so it gets a synthetic
/// heading that the model can still point at.
struct FeedbackTarget {
    id: String,
    heading: String,
    kind: SectionKind,
}

async fn feedback_target(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    version_id: Uuid,
    section_id: &str,
) -> anyhow::Result<FeedbackTarget> {
    if section_id == PROFILE_SUMMARY_ID {
        return Ok(FeedbackTarget {
            id: section_id.to_owned(),
            heading: "Profile impression".to_owned(),
            kind: SectionKind::Summary,
        });
    }
    let row = sqlx::query(
        "select section ->> 'heading' as heading, section ->> 'kind' as kind
         from versions,
              jsonb_array_elements(coalesce(versions.sections -> 'sections', '[]'::jsonb))
                  as section
         where versions.id = $1 and section ->> 'id' = $2",
    )
    .bind(version_id)
    .bind(section_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(FeedbackTarget {
            id: section_id.to_owned(),
            heading: section_id.to_owned(),
            kind: SectionKind::Other,
        });
    };
    let kind: String = row.try_get("kind")?;
    Ok(FeedbackTarget {
        id: section_id.to_owned(),
        heading: row.try_get::<Option<String>, _>("heading")?.unwrap_or_default(),
        kind: SectionKind::parse(&kind).unwrap_or(SectionKind::Other),
    })
}

fn run_status(value: String) -> anyhow::Result<RunStatus> {
    RunStatus::parse(&value).ok_or_else(|| anyhow::anyhow!("bad run status {value}"))
}

/// Serialize a value for a jsonb column, removing NUL codepoints first.
///
/// Postgres cannot represent U+0000 in `jsonb`: the text protocol encodes it as
/// the `\u0000` escape, and jsonb rejects that with SQLSTATE 0A000
/// ("unsupported Unicode escape sequence"). NULs are common in text pulled from
/// PDFs and DOCX numeric character references, and third-party callbacks can
/// send them too, so strip them at the one boundary every jsonb write crosses.
/// Every other character (including emoji and other non-ASCII text) is kept.
fn jsonb<T: Serialize>(value: &T) -> anyhow::Result<Json<serde_json::Value>> {
    let mut value = serde_json::to_value(value)?;
    strip_nul(&mut value);
    Ok(Json(value))
}

fn strip_nul(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => text.retain(|character| character != '\0'),
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_nul),
        serde_json::Value::Object(fields) => fields.values_mut().for_each(strip_nul),
        _ => {}
    }
}

fn vote_key(vote: SectionVote) -> &'static str {
    match vote {
        SectionVote::Up => "up",
        SectionVote::Down => "down",
    }
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
        store.save_parse(version, &parse, None, None).await.unwrap();
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
        store
            .save_parse(version_id, &parse, None, None)
            .await
            .unwrap();
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
        store.save_parse(second, &parse, None, None).await.unwrap();
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
        store
            .save_parse(version_id, &parse, None, None)
            .await
            .unwrap();
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
        store
            .save_parse(ada_version, &parse, None, None)
            .await
            .unwrap();
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
        store.save_parse(first, &parse, None, None).await.unwrap();
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
        store
            .save_parse(second, &second_parse, None, None)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn sections_are_stored_with_the_parse_and_votes_are_per_user() {
        use domain::{DetectedSection, SectionKind};

        let store = Ephemeral::start().await.unwrap();
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        sqlx::query(
            "insert into users (id, email) values ($1, 'owner@example.com'), ($2, 'other@example.com')",
        )
        .bind(owner)
        .bind(other)
        .execute(store.pool())
        .await
        .unwrap();
        let resume = Uuid::new_v4();
        let version = Uuid::new_v4();
        store
            .create_resume(owner, resume, version, "resume.pdf", domain::PDF, "file")
            .await
            .unwrap();
        assert!(
            !store
                .vote_section(
                    owner,
                    resume,
                    version,
                    PROFILE_SUMMARY_ID,
                    Some(SectionVote::Up),
                )
                .await
                .unwrap(),
            "a summary cannot be voted on before it exists"
        );
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        let map = SectionMap {
            engine: "test".to_owned(),
            model: "alias".to_owned(),
            sections: vec![DetectedSection {
                id: "experience-1".to_owned(),
                kind: SectionKind::Experience,
                heading: "Experience".to_owned(),
                body: "Built things.".to_owned(),
                confidence: 0.9,
                feedback: None,
            }],
        };
        let generated = ProfileSummary {
            text: "Ada is a computing pioneer.".to_owned(),
            engine: "test".to_owned(),
            model: "small-llm".to_owned(),
            feedback: None,
        };
        store
            .save_parse(version, &parse, Some(&map), Some(&generated))
            .await
            .unwrap();

        let detail = store.detail(owner, resume).await.unwrap().unwrap();
        assert_eq!(detail.versions[0].sections.as_ref(), Some(&map));
        assert_eq!(
            detail.versions[0].profile_summary.as_ref(),
            Some(&generated)
        );

        assert!(
            store
                .vote_section(
                    owner,
                    resume,
                    version,
                    "experience-1",
                    Some(SectionVote::Up)
                )
                .await
                .unwrap()
        );
        assert!(
            !store
                .vote_section(owner, resume, version, "skills-1", Some(SectionVote::Up))
                .await
                .unwrap(),
            "a section that was not detected cannot be voted on"
        );
        assert!(
            !store
                .vote_section(
                    other,
                    resume,
                    version,
                    "experience-1",
                    Some(SectionVote::Up)
                )
                .await
                .unwrap(),
            "another user's version is never writable"
        );

        let feedback = |user| {
            let store = &store.store;
            async move {
                store.detail(user, resume).await.unwrap().map(|detail| {
                    detail.versions[0].sections.as_ref().unwrap().sections[0].feedback
                })
            }
        };
        assert_eq!(feedback(owner).await, Some(Some(SectionVote::Up)));
        assert_eq!(feedback(other).await, None, "another user sees no version");

        store
            .vote_section(
                owner,
                resume,
                version,
                "experience-1",
                Some(SectionVote::Down),
            )
            .await
            .unwrap();
        assert_eq!(feedback(owner).await, Some(Some(SectionVote::Down)));
        store
            .vote_section(owner, resume, version, "experience-1", None)
            .await
            .unwrap();
        assert_eq!(feedback(owner).await, Some(None));

        assert!(
            store
                .vote_section(
                    owner,
                    resume,
                    version,
                    PROFILE_SUMMARY_ID,
                    Some(SectionVote::Up),
                )
                .await
                .unwrap()
        );
        assert!(
            !store
                .vote_section(
                    other,
                    resume,
                    version,
                    PROFILE_SUMMARY_ID,
                    Some(SectionVote::Up),
                )
                .await
                .unwrap()
        );
        let summary_feedback = || async {
            store.detail(owner, resume).await.unwrap().unwrap().versions[0]
                .profile_summary
                .as_ref()
                .unwrap()
                .feedback
        };
        assert_eq!(summary_feedback().await, Some(SectionVote::Up));
        store
            .vote_section(
                owner,
                resume,
                version,
                PROFILE_SUMMARY_ID,
                Some(SectionVote::Down),
            )
            .await
            .unwrap();
        assert_eq!(summary_feedback().await, Some(SectionVote::Down));
        store
            .vote_section(owner, resume, version, PROFILE_SUMMARY_ID, None)
            .await
            .unwrap();
        assert_eq!(summary_feedback().await, None);

        store
            .vote_section(
                owner,
                resume,
                version,
                "experience-1",
                Some(SectionVote::Up),
            )
            .await
            .unwrap();
        store
            .vote_section(
                owner,
                resume,
                version,
                PROFILE_SUMMARY_ID,
                Some(SectionVote::Up),
            )
            .await
            .unwrap();
        assert!(
            store
                .delete_resume(owner, resume, Some(version))
                .await
                .unwrap()
                .is_some()
        );
        let remaining: i64 = sqlx::query_scalar("select count(*) from section_feedback")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }

    /// Regression: Postgres rejects jsonb text that carries an unsupported
    /// Unicode escape sequence, surfacing from the database as
    /// "unsupported Unicode escape sequence". The shapes we store must never
    /// produce one, so round-trip every Unicode-heavy field through the real
    /// claim/save/callback paths and assert the database accepts it.
    #[tokio::test]
    async fn stored_unicode_round_trips_through_postgres_jsonb() {
        let store = Ephemeral::start().await.unwrap();
        let user = domain::DEMO_USER_ID;
        let resume_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        store
            .create_resume(user, resume_id, version_id, "uni.pdf", domain::PDF, "uni-key")
            .await
            .unwrap();

        let mut parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        parse.profile.name = Some("Zoë — Søren 🚀".to_owned());
        parse.profile.email = Some("ada@example.com".to_owned());
        parse.profile.sections = vec![domain::Section {
            heading: "Überschäft and Qualifikationën".to_owned(),
            body: "Löyens — built the café.\nTabs\tand\ttabs.\u{2028}Separator.".to_owned(),
            in_table: false,
            in_text_box: false,
            in_header: false,
        }];
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "alias".to_owned(),
            sections: vec![domain::DetectedSection {
                id: "experience-1".to_owned(),
                kind: domain::SectionKind::Experience,
                heading: "Experience".to_owned(),
                body: "Building café rocket software. 🚀".to_owned(),
                confidence: 0.9,
                feedback: None,
            }],
        };
        let summary = domain::ProfileSummary {
            text: "Ada’s résumé builds coffee rockets. 🚀".to_owned(),
            engine: "extractive".to_owned(),
            model: "section-extract-v1".to_owned(),
            feedback: None,
        };
        store
            .save_parse(version_id, &parse, Some(&sections), Some(&summary))
            .await
            .unwrap();
        store.fanout(version_id, &["stub"]).await.unwrap();
        let detail = store.detail(user, resume_id).await.unwrap().unwrap();
        let run = detail.versions[0].runs[0].clone();
        let verdict = Verdict {
            kept: vec!["email".to_owned()],
            dropped: vec![],
            score: 80,
            summary: "Zoë — Søren 🚀 keeps the café. 🚀".to_owned(),
            factors: vec![],
            report: None,
        };
        store
            .record_callback("stub", run.id, &run.id.to_string(), &parse, &verdict)
            .await
            .unwrap();
        let detail = store.detail(user, resume_id).await.unwrap().unwrap();
        let stored = detail
            .versions
            .iter()
            .find(|version| version.id == version_id)
            .and_then(|version| version.runs.first())
            .and_then(|run| run.verdict.clone())
            .expect("callback verdict stored");
        assert_eq!(stored.score, 80);
        assert!(stored.summary.contains('🚀'));
        let parts: Vec<&str> = stored
            .summary
            .split(|character: char| character.is_whitespace() || character == '—' || character == '\u{2028}')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(parts.first(), Some(&"Zoë"));
    }

    /// The real trigger for "unsupported Unicode escape sequence" is U+0000:
    /// serde_json writes it as `\u0000`, and Postgres jsonb rejects that escape
    /// (SQLSTATE 0A000). Lone surrogates fail earlier with a different error,
    /// so this pins the failure mode the fix has to neutralise.
    #[tokio::test]
    async fn postgresql_rejects_nul_escape_in_jsonb_text() {
        let store = Ephemeral::start().await.unwrap();
        let result = sqlx::query(r#"SELECT ('{"a":"\u0000"}')::jsonb"#)
            .fetch_one(store.pool())
            .await;
        let err = result.expect_err("jsonb must reject a NUL escape").to_string();
        assert!(
            err.contains("unsupported Unicode escape sequence"),
            "the error should name the jsonb rejection: {err}"
        );
    }

    /// Regression for the production failure: NULs in extracted or callback
    /// text must not reach jsonb. They are stripped on the way in, and every
    /// other Unicode character still round-trips intact.
    #[tokio::test]
    async fn nul_in_stored_text_is_stripped_before_jsonb() {
        let store = Ephemeral::start().await.unwrap();
        let user = domain::DEMO_USER_ID;
        let resume_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        store
            .create_resume(user, resume_id, version_id, "nul.pdf", domain::PDF, "nul-key")
            .await
            .unwrap();

        let mut parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        parse.profile.name = Some("Ada\0 Lovelace".to_owned());
        parse.profile.sections = vec![domain::Section {
            heading: "Experience\0".to_owned(),
            body: "Wrote the first\0 algorithm.".to_owned(),
            in_table: false,
            in_text_box: false,
            in_header: false,
        }];
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "alias".to_owned(),
            sections: vec![domain::DetectedSection {
                id: "experience-1".to_owned(),
                kind: domain::SectionKind::Experience,
                heading: "Experience".to_owned(),
                body: "Analytical engine\0 notes.".to_owned(),
                confidence: 0.9,
                feedback: None,
            }],
        };
        let summary = domain::ProfileSummary {
            text: "Ada\0 builds engines. 🚀".to_owned(),
            engine: "extractive".to_owned(),
            model: "section-extract-v1".to_owned(),
            feedback: None,
        };
        store
            .save_parse(version_id, &parse, Some(&sections), Some(&summary))
            .await
            .expect("NUL text must be stored, not rejected by jsonb");

        let detail = store.detail(user, resume_id).await.unwrap().unwrap();
        let stored = &detail.versions[0];
        let stored_parse = stored.parse.as_ref().expect("parse stored");
        assert_eq!(stored_parse.profile.name.as_deref(), Some("Ada Lovelace"));
        assert!(!stored_parse.profile.sections[0].body.contains('\0'));
        let stored_sections = stored.sections.as_ref().expect("sections stored");
        assert_eq!(stored_sections.sections[0].body, "Analytical engine notes.");
        let stored_summary = stored.profile_summary.as_ref().expect("summary stored");
        assert_eq!(stored_summary.text, "Ada builds engines. 🚀");
    }

    #[test]
    fn jsonb_serialization_strips_nul_and_keeps_other_unicode() {
        let value = serde_json::json!({
            "text": "café\0 🚀",
            "nested": ["a\0b", { "c": "d\0e" }],
            "number": 1,
        });
        let Json(stripped) = jsonb(&value).unwrap();
        assert_eq!(stripped["text"], "café 🚀");
        assert_eq!(stripped["nested"][0], "ab");
        assert_eq!(stripped["nested"][1]["c"], "de");
        assert_eq!(stripped["number"], 1);
    }

    /// A thumbs down queues the markdown build once, then a diagnosis for every
    /// down vote on the version. A later thumbs down on the same version queues
    /// only a diagnosis, and the stored history keeps each diagnosis.
    #[tokio::test]
    async fn thumbs_down_queues_markdown_then_one_diagnosis_per_vote() {
        use domain::{DetectedSection, SectionKind};

        let store = Ephemeral::start().await.unwrap();
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        sqlx::query(
            "insert into users (id, email) values ($1, 'owner@example.com'), ($2, 'other@example.com')",
        )
        .bind(owner)
        .bind(other)
        .execute(store.pool())
        .await
        .unwrap();
        let resume = Uuid::new_v4();
        let version = Uuid::new_v4();
        store
            .create_resume(owner, resume, version, "resume.pdf", domain::PDF, "file")
            .await
            .unwrap();
        let parse = ParseInput::empty(domain::LayoutConfidence::Inferred);
        let sections = SectionMap {
            engine: "test".to_owned(),
            model: "alias".to_owned(),
            sections: vec![DetectedSection {
                id: "experience-1".to_owned(),
                kind: SectionKind::Experience,
                heading: "Experience".to_owned(),
                body: "Built analytical engines.".to_owned(),
                confidence: 0.9,
                feedback: None,
            }],
        };
        let summary = domain::ProfileSummary {
            text: "A candidate.".to_owned(),
            engine: "extractive".to_owned(),
            model: "section-extract-v1".to_owned(),
            feedback: None,
        };
        store
            .save_parse(version, &parse, Some(&sections), Some(&summary))
            .await
            .unwrap();

        // `create_resume` enqueues `ResumeUploaded` and `save_parse` enqueues
        // `ResumeParsed`, so drain both before the thumbs down.
        for expected in [EventName::ResumeUploaded, EventName::ResumeParsed] {
            let claimed = store.claim().await.unwrap().unwrap();
            assert_eq!(claimed.name, expected);
            store.finish(claimed.id, claimed.token).await.unwrap();
        }
        assert!(store.claim().await.unwrap().is_none());

        // The first thumbs down queues the markdown build, not a diagnosis.
        store
            .vote_section(owner, resume, version, "experience-1", Some(SectionVote::Down))
            .await
            .unwrap();
        assert!(store.resume_markdown(version).await.unwrap().is_none());
        let pending = store.claim().await.unwrap().unwrap();
        assert_eq!(pending.name, EventName::GenerateMarkdown);
        assert!(store.claim().await.unwrap().is_none());
        store.finish(pending.id, pending.token).await.unwrap();

        // A second thumbs down on the same version, before the markdown exists,
        // queues another build rather than a diagnosis.
        store
            .vote_section(owner, resume, version, "experience-1", Some(SectionVote::Down))
            .await
            .unwrap();
        let second = store.claim().await.unwrap().unwrap();
        assert_eq!(second.name, EventName::GenerateMarkdown);
        store.finish(second.id, second.token).await.unwrap();

        // The worker builds the markdown once, then fans out to every pending
        // down vote. The build is idempotent: a second write is a no-op.
        let markdown = ResumeMarkdown {
            text: "# Ada\n\n## Experience\n\nBuilt analytical engines.".to_owned(),
            engine: "test".to_owned(),
            model: "alias".to_owned(),
        };
        assert!(store.save_resume_markdown(version, &markdown).await.unwrap());
        assert!(!store.save_resume_markdown(version, &markdown).await.unwrap());
        // The worker's markdown handler fans out to every pending down vote.
        store.enqueue_feedback_analyses(version).await.unwrap();
        let queued = store.claim().await.unwrap().unwrap();
        assert_eq!(queued.name, EventName::AnalyzeFeedback);
        let payload: AnalyzeFeedbackPayload =
            serde_json::from_value(queued.payload.clone()).unwrap();
        assert_eq!(payload.section_id, "experience-1");
        assert_eq!(payload.section_heading, "Experience");
        assert_eq!(payload.section_kind, SectionKind::Experience);
        assert_eq!(payload.user_id, owner);
        store.finish(queued.id, queued.token).await.unwrap();
        assert!(store.claim().await.unwrap().is_none());

        // Once the markdown exists, a thumbs down queues only a diagnosis.
        store
            .vote_section(owner, resume, version, "experience-1", Some(SectionVote::Down))
            .await
            .unwrap();
        let later = store.claim().await.unwrap().unwrap();
        assert_eq!(later.name, EventName::AnalyzeFeedback);
        let later_payload: AnalyzeFeedbackPayload =
            serde_json::from_value(later.payload.clone()).unwrap();
        assert_eq!(later_payload.section_id, "experience-1");
        assert_eq!(later_payload.section_heading, "Experience");
        assert_eq!(later_payload.section_kind, SectionKind::Experience);
        store.finish(later.id, later.token).await.unwrap();

        // The impression thumbs down is diagnosed too, with a synthetic heading.
        store
            .vote_section(owner, resume, version, domain::PROFILE_SUMMARY_ID, Some(SectionVote::Down))
            .await
            .unwrap();
        let impression_event = store.claim().await.unwrap().unwrap();
        assert_eq!(impression_event.name, EventName::AnalyzeFeedback);
        let impression: AnalyzeFeedbackPayload =
            serde_json::from_value(impression_event.payload.clone()).unwrap();
        assert_eq!(impression.section_id, domain::PROFILE_SUMMARY_ID);
        assert_eq!(impression.section_heading, "Profile impression");
        assert_eq!(impression.section_kind, SectionKind::Summary);
        store.finish(impression_event.id, impression_event.token).await.unwrap();

        // A different user's down vote is queued for its own diagnosis. The resume
        // belongs to that user, so the vote records and the worker diagnoses it.
        let other_resume = Uuid::new_v4();
        let other_version = Uuid::new_v4();
        store
            .create_resume(other, other_resume, other_version, "other.pdf", domain::PDF, "other-key")
            .await
            .unwrap();
        store
            .save_parse(
                other_version,
                &parse,
                Some(&sections),
                Some(&summary),
            )
            .await
            .unwrap();
        for expected in [EventName::ResumeUploaded, EventName::ResumeParsed] {
            let claimed = store.claim().await.unwrap().unwrap();
            assert_eq!(claimed.name, expected);
            store.finish(claimed.id, claimed.token).await.unwrap();
        }
        store
            .save_resume_markdown(other_version, &markdown)
            .await
            .unwrap();
        store
            .vote_section(other, other_resume, other_version, "experience-1", Some(SectionVote::Down))
            .await
            .unwrap();
        let other_event = store.claim().await.unwrap().unwrap();
        assert_eq!(other_event.name, EventName::AnalyzeFeedback);
        let other_payload: AnalyzeFeedbackPayload =
            serde_json::from_value(other_event.payload.clone()).unwrap();
        assert_eq!(other_payload.user_id, other);
        assert_eq!(other_payload.section_id, "experience-1");
        store.finish(other_event.id, other_event.token).await.unwrap();

        assert!(store.claim().await.unwrap().is_none());

        // Each stored diagnosis keeps its own row, keyed by user and section. The
        // detail view does not carry these to the user-facing frontend; the
        // worker reads them directly here.
        store
            .save_feedback_analysis(&impression, "extractive", "feedback-extract-v1", "Looks right.")
            .await
            .unwrap();
        store
            .save_feedback_analysis(
                &payload,
                "extractive",
                "feedback-extract-v1",
                "The heading is fine; the body stops too early.",
            )
            .await
            .unwrap();
        let analyses: Vec<String> = sqlx::query_scalar(
            "select section_id from feedback_analysis where version_id = $1",
        )
        .bind(version)
        .fetch_all(store.pool())
        .await
        .unwrap();
        assert_eq!(analyses.len(), 2);
        assert!(analyses.iter().any(|section| section == "experience-1"));
        assert!(
            analyses.iter().any(|section| section == domain::PROFILE_SUMMARY_ID)
        );
    }
}
