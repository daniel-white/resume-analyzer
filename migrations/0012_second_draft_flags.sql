-- Bad-practice flags from the same model read that produced the skills. Each
-- flag is grounded in a stored section before it reaches this column.
alter table versions add column writing_issues jsonb;

-- A flag is a checklist item like the other grounded edits, but the writer is
-- rewriting bad wording instead of filling, promoting, or cutting a line.
alter table version_checklist_items drop constraint version_checklist_items_kind;
alter table version_checklist_items
    add constraint version_checklist_items_kind
    check (kind in ('blank', 'promote', 'cut', 'flag'));
