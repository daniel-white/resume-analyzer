-- Admins triage model diagnoses without deleting the evidence. Handling is a
-- flag on the analysis row: the diagnosis, the vote, and the history stay.
alter table feedback_analysis
    add column handled_at timestamptz,
    add column handled_by text;

-- The admin list mostly asks for open work, newest first.
create index feedback_analysis_open
    on feedback_analysis (created_at desc)
    where handled_at is null;
