-- Our own section reading, separate from the verification parse. The engine
-- and model names travel with the map so the UI can disclose what produced it.
alter table versions add column sections jsonb;

-- One vote per user, version, and detected section. A changed vote updates the
-- row; clearing a vote deletes it. Deleting a version removes its votes.
create table section_feedback (
    version_id uuid not null references versions (id) on delete cascade,
    user_id uuid not null references users (id) on delete cascade,
    section_id text not null,
    vote text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (version_id, user_id, section_id),
    constraint section_feedback_vote check (vote in ('up', 'down'))
);
