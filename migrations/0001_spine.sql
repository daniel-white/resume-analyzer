create table resumes (
    id uuid primary key,
    user_id uuid not null,
    title text not null,
    created_at timestamptz not null default now()
);

create index resumes_by_user on resumes (user_id, created_at desc);

create table versions (
    id uuid primary key,
    resume_id uuid not null references resumes (id),
    number integer not null,
    filename text not null,
    media_type text not null,
    object_key text not null,
    status text not null,
    parse jsonb,
    created_at timestamptz not null default now(),
    constraint versions_number_positive check (number >= 1),
    constraint versions_status check (status in ('uploaded', 'parsed', 'running', 'complete')),
    unique (resume_id, number)
);

create index versions_by_resume on versions (resume_id, number desc);

create table outbox (
    id uuid primary key,
    seq bigserial unique,
    resume_id uuid not null references resumes (id),
    version_id uuid not null references versions (id),
    name text not null,
    payload jsonb not null,
    created_at timestamptz not null default now(),
    available_at timestamptz not null default now(),
    claimed_at timestamptz,
    claim_token uuid,
    dispatched_at timestamptz
);

create index outbox_pending on outbox (seq) where dispatched_at is null;

create table runs (
    id uuid primary key,
    version_id uuid not null references versions (id),
    plugin_id text not null,
    status text not null,
    parse jsonb,
    verdict jsonb,
    constraint runs_status check (status in ('requested', 'complete')),
    unique (version_id, plugin_id)
);

create table callback_keys (
    idempotency_key text primary key,
    plugin_id text not null,
    run_id uuid not null,
    received_at timestamptz not null default now()
);
