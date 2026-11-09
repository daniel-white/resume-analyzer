-- A generated candidate profile for one parsed version. Provenance travels
-- with the text; the calling user's vote is hydrated from section_feedback.
alter table versions add column profile_summary jsonb;
