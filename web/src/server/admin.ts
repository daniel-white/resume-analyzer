import { apiFetch } from "./session";
import type { SectionKind } from "./resumes";

/// One stored model diagnosis. Handling is a flag on the row, never a delete.
export type AdminFeedbackAnalysis = {
  id: string;
  model: string;
  engine: string;
  diagnosis: string;
  created_at: string;
  handled_at?: string;
  handled_by?: string;
};

/// A current thumbs down and every diagnosis stored for it.
export type AdminFeedback = {
  version_id: string;
  user_id: string;
  user_email: string;
  resume_id: string;
  resume_title: string;
  version_number: number;
  filename: string;
  section_id: string;
  section_heading: string;
  section_kind: SectionKind;
  section_body?: string;
  voted_at: string;
  analyses: AdminFeedbackAnalysis[];
};

export type HandledResult = { ok: true } | { ok: false; message: string };

/// The caller is signed in, but their email domain is not configured for admin.
export class NotAdmin extends Error {
  constructor() {
    super("not an admin");
    this.name = "NotAdmin";
  }
}

export async function fetchAdminFeedback(
  baseUrl: string,
  cookie: string | undefined,
): Promise<AdminFeedback[]> {
  const result = await apiFetch(baseUrl, "/api/admin/feedback", cookie);
  if (!result.ok) {
    if (result.status === 403) throw new NotAdmin();
    throw new Error(`admin feedback returned ${result.status}`);
  }
  const body = (await result.value.json()) as { feedback?: AdminFeedback[] };
  return body.feedback ?? [];
}

export async function setAnalysisHandled(
  baseUrl: string,
  id: string,
  handled: boolean,
  cookie: string | undefined,
): Promise<HandledResult> {
  const result = await apiFetch(
    baseUrl,
    `/api/admin/feedback/analyses/${encodeURIComponent(id)}`,
    cookie,
    {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ handled }),
    },
  );
  if (!result.ok) {
    return {
      ok: false,
      message: result.status === 404
        ? "That diagnosis is no longer here. Refresh and try again."
        : "Could not update the diagnosis. Please try again.",
    };
  }
  return { ok: true };
}

/// Delete one stored diagnosis and ask the worker to regenerate it. The
/// model still has the version's markdown and raw file, so the new diagnosis
/// arrives as a fresh row.
export async function rerunAnalysis(
  baseUrl: string,
  id: string,
  cookie: string | undefined,
): Promise<HandledResult> {
  const result = await apiFetch(
    baseUrl,
    `/api/admin/feedback/analyses/${encodeURIComponent(id)}`,
    cookie,
    { method: "POST" },
  );
  if (!result.ok) {
    return {
      ok: false,
      message: result.status === 404
        ? "That diagnosis is no longer here. Refresh and try again."
        : "Could not rerun the diagnosis. Please try again.",
    };
  }
  return { ok: true };
}
