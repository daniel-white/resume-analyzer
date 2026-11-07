-- Old results are not emailed retroactively. New callbacks start an 8s batch window.
alter table runs add column notification_due_at timestamptz;
alter table runs add column notified_at timestamptz;
alter table versions add column notification_lease_until timestamptz;
create index runs_pending_notifications on runs (version_id, notification_due_at)
    where notification_due_at is not null and notified_at is null;
