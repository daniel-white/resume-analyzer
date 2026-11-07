import { apiFetch, readSetCookie } from "./session";

export type Account = {
  email: string;
};

export type MagicLinkResult = { ok: true } | { ok: false; message: string };

export type VerifyResult =
  | { ok: true; setCookie: string }
  | { ok: false; message: string };

type ErrorBody = {
  error?: string;
};

export function magicLinkMessage(status: number, body: string): string {
  const code = errorCode(body);
  if (status === 400 && code === "email") return "That email does not look right.";
  if (status === 400 && code === "upload") return "That upload has expired. Please choose your resume again.";
  if (status === 502 || code === "mail") return "The sign-in email did not send.";
  return "The sign-in email did not send.";
}

export function verifyMessage(status: number, body: string): string {
  const code = errorCode(body);
  if (status === 400 && code === "link") {
    return "That link has expired. Ask for another.";
  }
  return "That link did not sign you in.";
}

export async function requestMagicLink(
  baseUrl: string,
  email: string,
  cookie?: string,
  returnTo?: string,
  uploadId?: string,
): Promise<MagicLinkResult> {
  const trimmed = email.trim();
  if (!trimmed) return { ok: false, message: "Enter your email." };
  const response = await fetch(new URL("/api/auth/magic-link", baseUrl), {
    method: "POST",
    headers: jsonHeaders(cookie),
    body: JSON.stringify({ email: trimmed, return_to: returnTo, upload_id: uploadId }),
  });
  if (response.status === 202) return { ok: true };
  return { ok: false, message: magicLinkMessage(response.status, await response.text()) };
}

export async function verifyMagicLink(
  baseUrl: string,
  token: string,
  cookie?: string,
): Promise<VerifyResult> {
  if (!token) return { ok: false, message: "That link did not sign you in." };
  const response = await fetch(new URL("/api/auth/magic-link/verify", baseUrl), {
    method: "POST",
    headers: jsonHeaders(cookie),
    body: JSON.stringify({ token }),
  });
  if (!response.ok) {
    return { ok: false, message: verifyMessage(response.status, await response.text()) };
  }
  const setCookie = readSetCookie(response.headers);
  if (!setCookie) return { ok: false, message: "That link did not sign you in." };
  return { ok: true, setCookie };
}

export async function fetchAccount(
  baseUrl: string,
  cookie: string | undefined,
): Promise<Account | null> {
  const result = await apiFetch(baseUrl, "/api/me", cookie);
  if (!result.ok) throw new Error(`account returned ${result.status}`);
  const body = (await result.value.json()) as { email?: string };
  if (!body.email) return null;
  return { email: body.email };
}

export async function postLogout(
  baseUrl: string,
  cookie: string | undefined,
): Promise<string | null> {
  const headers = new Headers();
  if (cookie) headers.set("cookie", cookie);
  const response = await fetch(new URL("/api/auth/logout", baseUrl), {
    method: "POST",
    headers,
  });
  return readSetCookie(response.headers);
}

function jsonHeaders(cookie?: string): Headers {
  const headers = new Headers({ "content-type": "application/json" });
  if (cookie) headers.set("cookie", cookie);
  return headers;
}

function errorCode(body: string): string | null {
  try {
    const parsed = JSON.parse(body) as ErrorBody;
    return typeof parsed.error === "string" ? parsed.error : null;
  } catch {
    return null;
  }
}
