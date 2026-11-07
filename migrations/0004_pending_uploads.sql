-- Anonymous files are private and expire after one hour. Only a verified
-- magic link bound to this upload can promote it into an account.
create table pending_uploads (
    id uuid primary key,
    filename text not null,
    media_type text not null,
    object_key text not null,
    expires_at timestamptz not null default now() + interval '1 hour'
);
alter table magic_links add column pending_upload_id uuid;
create index pending_uploads_expiry on pending_uploads (expires_at);
