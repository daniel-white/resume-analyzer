/// Browser-reported IANA timezone, captured when a user requests a magic
/// link and carried onto their session. It lets SSR-rendered timestamps use
/// the same zone the browser will, so a page does not shift timezones after
/// hydration. Unknown or absent values fall back to UTC on the client.
///
/// The value is validated twice: here, with `Intl.DateTimeFormat`, and again
/// on the server, which treats anything it cannot trust as absent.
export function detectTimeZone(): string | null {
  if (typeof Intl === "undefined" || typeof Intl.DateTimeFormat !== "function") {
    return null;
  }
  let zone: string;
  try {
    zone = Intl.DateTimeFormat("en-US", { timeZone: "America/New_York" }).resolvedOptions().timeZone;
  } catch {
    return null;
  }
  if (!zone || zone.length === 0 || zone.length > 64) {
    return null;
  }
  // Reject anything with control characters; the server does the same check.
  if ([...zone].some((c) => c < " " || c > "~")) {
    return null;
  }
  // Final confirmation: the zone must actually be usable by the runtime.
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: zone });
  } catch {
    return null;
  }
  return zone;
}