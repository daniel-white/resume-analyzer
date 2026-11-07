// Only resume detail routes are valid post-login destinations. Never accept an
// external URL (including protocol-relative URLs) from search or form input.
export function resumeReturnTo(value: unknown): string | undefined {
  if (typeof value !== "string" || !/^\/resumes\/[0-9a-f-]{36}(?:\?|$)/i.test(value)) return undefined;
  try {
    const url = new URL(value, "https://resume.local");
    if (url.origin !== "https://resume.local" || !/^\/resumes\/[0-9a-f-]{36}$/i.test(url.pathname)) return undefined;
    return url.pathname + url.search;
  } catch {
    return undefined;
  }
}
