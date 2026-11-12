// Only resume detail routes and the exact admin page are valid post-login
// destinations. Never accept an external URL (including protocol-relative
// URLs) from search or form input.
const LOCAL_ORIGIN = "https://resume.local";
const RESUME_PATH = /^\/resumes\/[0-9a-f-]{36}$/i;

export function safeReturnTo(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  if (value === "/admin") return value;
  if (!/^\/resumes\/[0-9a-f-]{36}(?:\?|$)/i.test(value)) return undefined;
  try {
    const url = new URL(value, LOCAL_ORIGIN);
    if (url.origin !== LOCAL_ORIGIN || !RESUME_PATH.test(url.pathname)) return undefined;
    return url.pathname + url.search;
  } catch {
    return undefined;
  }
}
