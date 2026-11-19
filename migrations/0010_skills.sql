-- Evidence-backed skills for one parsed version. The map is optional: a
-- version stays usable when the skills model has not finished or is unavailable.
alter table versions add column skills jsonb;
