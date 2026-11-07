export class SignedOut extends Error {
  constructor() {
    super("signed out");
    this.name = "SignedOut";
  }
}

export type ApiResult<T> =
  | { ok: true; value: T }
  | { ok: false; status: number; body: string };

export function readSetCookie(headers: Headers): string | null {
  if (typeof headers.getSetCookie === "function") {
    const [first] = headers.getSetCookie();
    if (first) return first;
  }
  return headers.get("set-cookie");
}

export async function apiFetch(
  baseUrl: string,
  path: string,
  cookie: string | undefined,
  init: RequestInit = {},
): Promise<ApiResult<Response>> {
  const headers = new Headers(init.headers);
  if (cookie) headers.set("cookie", cookie);
  const response = await fetch(new URL(path, baseUrl), { ...init, headers });
  if (response.status === 401) throw new SignedOut();
  if (!response.ok) {
    return { ok: false, status: response.status, body: await response.text() };
  }
  return { ok: true, value: response };
}
