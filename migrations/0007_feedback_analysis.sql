-- A markdown rendering of the resume, built from the detected sections. It
-- exists so a feedback analysis model can read the same sections the user
-- saw, without re-parsing the original file. Generated once per version,
-- after the first thumbs down.
alter table versions add column resume_markdown jsonb;

-- One model diagnosis per thumbs down. A later thumbs down on the same
-- version stores a new row; the history is kept so the user can see every
-- diagnosis the model gave for every item they rejected.
create table feedback_analysis (
    id uuid primary key,
    version_id uuid not null references versions (id) on delete cascade,
    user_id uuid not null references users (id) on delete cascade,
    section_id text not null,
    section_heading text not null,
    section_kind text not null,
    model text not null,
    engine text not null,
    diagnosis text not null,
    created_at timestamptz not null default now()
);

create index feedback_analysis_by_version on feedback_analysis (version_id, created_at desc);
