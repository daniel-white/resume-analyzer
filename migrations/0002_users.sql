create table users (
    id uuid primary key,
    email text not null unique,
    created_at timestamptz not null default now()
);

-- The token column is the SHA-256 of the 32-byte secret, never the secret.
create table magic_links (
    id uuid primary key,
    email text not null,
    token_hash bytea not null unique,
    expires_at timestamptz not null,
    used_at timestamptz
);

create table sessions (
    id uuid primary key,
    user_id uuid not null references users (id) on delete cascade,
    created_at timestamptz not null default now(),
    expires_at timestamptz not null
);

create index sessions_user_id_idx on sessions (user_id);
