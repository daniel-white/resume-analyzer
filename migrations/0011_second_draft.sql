-- A per-version writing checklist. Items are generated from stored sections,
-- skills and layout, so every suggestion points at text the writer can find.
-- `done` and `answer` are the reader's own state and survive regeneration.
create table version_checklist_items (
    id uuid primary key,
    version_id uuid not null references versions (id) on delete cascade,
    fingerprint text not null,
    kind text not null constraint version_checklist_items_kind check (kind in ('blank', 'promote', 'cut')),
    title text not null,
    detail text not null,
    why text not null,
    placeholder text not null,
    section_id text,
    section_heading text,
    quote text,
    template text,
    cut_text text,
    position integer not null,
    done boolean not null default false,
    answer text,
    checked_at timestamptz,
    created_at timestamptz not null default now(),
    constraint version_checklist_items_identity unique (version_id, fingerprint)
);

create index version_checklist_items_order
    on version_checklist_items (version_id, position, id);
