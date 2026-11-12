import type { AdminFeedback } from "~/server/admin";

export type AdminFilter = "open" | "handled" | "all";

export type AdminStats = {
  items: number;
  open: number;
  handled: number;
  analyses: number;
  users: number;
  sections: number;
};

export function analysisHandled(analysis: { handled_at?: string }): boolean {
  return Boolean(analysis.handled_at);
}

/// An item is handled once it has at least one diagnosis and all are handled.
/// A downvote with no diagnosis yet stays open so it is not lost.
export function feedbackHandled(item: AdminFeedback): boolean {
  return item.analyses.length > 0 && item.analyses.every(analysisHandled);
}

export function feedbackOpen(item: AdminFeedback): boolean {
  return !feedbackHandled(item);
}

export function adminStats(feedback: AdminFeedback[]): AdminStats {
  const users = new Set(feedback.map((item) => item.user_id));
  const sections = new Set(
    feedback.map((item) => `${item.version_id}:${item.section_id}`),
  );
  return {
    items: feedback.length,
    open: feedback.filter(feedbackOpen).length,
    handled: feedback.filter(feedbackHandled).length,
    analyses: feedback.reduce((total, item) => total + item.analyses.length, 0),
    users: users.size,
    sections: sections.size,
  };
}

/// Filter by handling state and a free-text query over the people, files, and
/// diagnoses. The query is case-insensitive and matches any field.
export function filterFeedback(
  feedback: AdminFeedback[],
  filter: AdminFilter,
  query = "",
): AdminFeedback[] {
  const needle = query.trim().toLowerCase();
  return feedback.filter((item) => {
    if (filter === "open" && !feedbackOpen(item)) return false;
    if (filter === "handled" && !feedbackHandled(item)) return false;
    if (!needle) return true;
    const haystack = [
      item.user_email,
      item.resume_title,
      item.filename,
      item.section_heading,
      item.section_kind,
      item.section_body ?? "",
      ...item.analyses.map((analysis) => analysis.diagnosis),
    ];
    return haystack.some((value) => value.toLowerCase().includes(needle));
  });
}
