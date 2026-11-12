-- A browser-reported IANA timezone, captured when a user requests a magic
-- link and carried onto their session. It lets SSR-rendered timestamps use
-- the same zone the browser will, so a page does not shift timezones after
-- hydration. Unknown or absent values fall back to UTC on the client.
alter table magic_links add column timezone text;
alter table sessions add column timezone text;