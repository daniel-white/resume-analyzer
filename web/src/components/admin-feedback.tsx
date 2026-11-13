import { useEffect, useState } from "react";
import {
  Check,
  FileText,
  RotateCcw,
  Search,
  ShieldCheck,
  ThumbsDown,
  Zap,
} from "lucide-react";

import { getAdminFeedback, rerunFeedbackAnalysis, setFeedbackAnalysisHandled } from "~/fns/admin.functions";
import {
  adminStats,
  analysisHandled,
  feedbackOpen,
  filterFeedback,
  type AdminFilter,
} from "~/lib/admin";
import { sectionKindLabel } from "~/lib/sections";
import { cn } from "~/lib/utils";
import type { AdminFeedback, AdminFeedbackAnalysis } from "~/server/admin";

const FILTERS: { id: AdminFilter; label: string }[] = [
  { id: "open", label: "Open" },
  { id: "handled", label: "Handled" },
  { id: "all", label: "All" },
];

export function AdminFeedbackDashboard({
  email,
  feedback,
  timezone,
}: {
  email: string;
  feedback: AdminFeedback[];
  timezone?: string | null;
}) {
  const [items, setItems] = useState(feedback);
  const [filter, setFilter] = useState<AdminFilter>("open");
  const [query, setQuery] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setItems(feedback);
    setError(null);
  }, [feedback]);

  const stats = adminStats(items);
  const visible = filterFeedback(items, filter, query);

  async function toggle(analysis: AdminFeedbackAnalysis) {
    if (busyId) return;
    const handled = !analysisHandled(analysis);
    setBusyId(analysis.id);
    setError(null);
    try {
      const result = await setFeedbackAnalysisHandled({
        data: { id: analysis.id, handled },
      });
      if (!result.ok) {
        setError(result.message);
        return;
      }
      setItems((current) => markHandled(current, analysis.id, handled, email));
    } catch {
      setError("Could not update the diagnosis. Please try again.");
    } finally {
      setBusyId(null);
    }
  }

  async function rerun(analysis: AdminFeedbackAnalysis) {
    if (busyId) return;
    setBusyId(analysis.id);
    setError(null);
    try {
      const result = await rerunFeedbackAnalysis({ data: { id: analysis.id } });
      if (!result.ok) {
        setError(result.message);
        return;
      }
      setItems((current) => removeAnalysis(current, analysis.id));
      // The worker enqueues a new diagnosis asynchronously; poll for it.
      const poll = window.setInterval(() => {
        void refresh();
      }, 1500);
      window.setTimeout(() => window.clearInterval(poll), 12000);
    } catch {
      setError("Could not rerun the diagnosis. Please try again.");
    } finally {
      setBusyId(null);
    }
  }

  async function refresh() {
    try {
      const view = await getAdminFeedback();
      setItems(view.feedback);
    } catch {
      // A poll failure is not worth surfacing; the next poll or navigation
      // will pick up the new diagnosis.
    }
  }

  return (
    <>
      <div className="library-heading admin-heading">
        <p className="eyebrow">ADMIN · FEEDBACK TRIAGE</p>
        <h1>What the model got wrong</h1>
        <p>
          Every current thumbs down, with the diagnoses stored for it. Marking a
          diagnosis handled keeps the row and its history; a later up vote hides
          the item without deleting anything.
        </p>
      </div>

      <dl className="admin-stats" aria-label="Feedback summary">
        <Stat label="Open" value={stats.open} tone="open" />
        <Stat label="Handled" value={stats.handled} tone="handled" />
        <Stat label="Diagnoses" value={stats.analyses} />
        <Stat label="People" value={stats.users} />
        <Stat label="Readings" value={stats.sections} />
      </dl>

      <div className="admin-toolbar">
        <div className="admin-filters" role="group" aria-label="Filter feedback">
          {FILTERS.map((option) => (
            <button
              key={option.id}
              type="button"
              className={cn("admin-filter", filter === option.id && "is-active")}
              aria-pressed={filter === option.id}
              onClick={() => setFilter(option.id)}
            >
              {option.label}
              <span>{countFor(option.id, stats)}</span>
            </button>
          ))}
        </div>
        <label className="admin-search" aria-label="Search feedback">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            placeholder="Search people, files, sections, or diagnoses"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
      </div>

      <p className="admin-count" role="status">
        {visible.length === 1 ? "1 item" : `${visible.length} items`}
        {query.trim() ? ` matching “${query.trim()}”` : ""}
      </p>

      {error ? <p className="admin-error" role="alert">{error}</p> : null}

      {visible.length === 0 ? (
        <div className="admin-empty">
          <ThumbsDown size={22} aria-hidden="true" />
          <h2>{items.length === 0 ? "No downvotes yet" : "Nothing matches"}</h2>
          <p>
            {items.length === 0
              ? "When someone rejects a section, it appears here with the model's diagnosis."
              : "Try another filter or clear the search."}
          </p>
        </div>
      ) : (
        <div className="admin-feedback-list">
          {visible.map((item) => (
            <FeedbackCard
              key={`${item.version_id}:${item.section_id}`}
              item={item}
              busyId={busyId}
              timezone={timezone}
              onToggle={(analysis) => void toggle(analysis)}
              onRerun={(analysis) => void rerun(analysis)}
            />
          ))}
        </div>
      )}
    </>
  );
}

function FeedbackCard({
  item,
  busyId,
  onToggle,
  onRerun,
  timezone,
}: {
  item: AdminFeedback;
  busyId: string | null;
  onToggle: (analysis: AdminFeedbackAnalysis) => void;
  onRerun: (analysis: AdminFeedbackAnalysis) => void;
  timezone?: string | null;
}) {
  const open = feedbackOpen(item);
  return (
    <article className="admin-feedback">
      <header className="admin-feedback-head">
        <div>
          <p className="admin-feedback-kind">
            {sectionKindLabel(item.section_kind)} · {item.user_email}
          </p>
          <h2>{item.section_heading}</h2>
          <p className="admin-feedback-source">
            <FileText size={14} aria-hidden="true" />
            {item.resume_title} · version {item.version_number} · {item.filename}
          </p>
        </div>
        <span className={cn("admin-status", open ? "is-open" : "is-handled")}>
          {open ? "Open" : "Handled"}
        </span>
      </header>

      {item.section_body ? (
        <blockquote className="admin-rejected">{item.section_body}</blockquote>
      ) : (
        <p className="admin-rejected is-missing">
          The rejected text is no longer in the stored reading.
        </p>
      )}

      <div className="admin-analyses">
        <h3>
          Model diagnoses
          <span>{item.analyses.length}</span>
        </h3>
        {item.analyses.length === 0 ? (
          <p className="admin-analysis-pending">
            No diagnosis stored yet. The model may still be reading this section.
          </p>
        ) : (
          item.analyses.map((analysis) => {
            const handled = analysisHandled(analysis);
            const busy = busyId === analysis.id;
            return (
              <div key={analysis.id} className={cn("admin-analysis", handled && "is-handled")}>
                <div className="admin-analysis-meta">
                  <span>{analysis.engine} · {analysis.model}</span>
                  <time dateTime={analysis.created_at}>{formatDate(analysis.created_at, timezone)}</time>
                </div>
                <p>{analysis.diagnosis}</p>
                <div className="admin-analysis-actions">
                  {handled ? (
                    <span className="admin-handled-note">
                      <ShieldCheck size={14} aria-hidden="true" />
                      {analysis.handled_by ? `Handled by ${analysis.handled_by}` : "Handled"}
                      {analysis.handled_at ? ` · ${formatDate(analysis.handled_at, timezone)}` : ""}
                    </span>
                  ) : (
                    <span className="admin-handled-note is-open">Waiting for review</span>
                  )}
                  <span className="admin-analysis-buttons">
                    <button
                      type="button"
                      className={cn("admin-rerun-button", busy && "is-busy")}
                      disabled={busyId !== null}
                      onClick={() => onRerun(analysis)}
                      title="Delete this diagnosis and ask the model to regenerate it"
                    >
                      <Zap size={14} aria-hidden="true" />
                      {busy ? "Rerunning" : "Rerun"}
                    </button>
                    <button
                      type="button"
                      className={cn("admin-handle-button", handled && "is-reopen")}
                      disabled={busyId !== null}
                      onClick={() => onToggle(analysis)}
                    >
                      {handled ? <RotateCcw size={14} aria-hidden="true" /> : <Check size={14} aria-hidden="true" />}
                      {busy ? "Saving" : handled ? "Reopen" : "Mark handled"}
                    </button>
                  </span>
                </div>
              </div>
            );
          })
        )}
      </div>

      <footer className="admin-feedback-foot">
        Downvoted {formatDate(item.voted_at, timezone)}
      </footer>
    </article>
  );
}

function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: "open" | "handled";
}) {
  return (
    <div className={cn("admin-stat", tone && `is-${tone}`)}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function countFor(filter: AdminFilter, stats: ReturnType<typeof adminStats>): number {
  if (filter === "open") return stats.open;
  if (filter === "handled") return stats.handled;
  return stats.items;
}

function markHandled(
  items: AdminFeedback[],
  id: string,
  handled: boolean,
  email: string,
): AdminFeedback[] {
  const now = new Date().toISOString();
  return items.map((item) => ({
    ...item,
    analyses: item.analyses.map((analysis) =>
      analysis.id === id
        ? {
            ...analysis,
            handled_at: handled ? now : undefined,
            handled_by: handled ? email : undefined,
          }
        : analysis,
    ),
  }));
}

/// Drop one diagnosis from the local list after a rerun. The worker enqueues
/// a new diagnosis; the list refreshes from the server when it lands.
function removeAnalysis(items: AdminFeedback[], id: string): AdminFeedback[] {
  return items
    .map((item) => ({
      ...item,
      analyses: item.analyses.filter((analysis) => analysis.id !== id),
    }))
    .filter((item) => item.analyses.length > 0);
}

function formatDate(value: string, timezone?: string | null): string {
  const date = new Date(value);
  const options: Intl.DateTimeFormatOptions = {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
    timeZone: timezone ?? "UTC",
  };
  // Build the string from parts so the output is identical on the server and
  // the client regardless of which ICU version each side runs. Node may emit
  // "Nov 7, 2026, 6:00 PM" while Chrome emits "Nov 7, 2026 at 6:00 PM" for the
  // same options; assembling ourselves avoids that hydration mismatch.
  const parts = new Intl.DateTimeFormat("en-US", options).formatToParts(date);
  const get = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  const hour = get("hour");
  const minute = get("minute");
  const dayPeriod = get("dayPeriod");
  const time = dayPeriod ? `${hour}:${minute} ${dayPeriod}` : `${hour}:${minute}`;
  const datePart = `${get("month")} ${get("day")}, ${get("year")}`;
  const formatted = `${datePart}, ${time}`;
  // Show "(UTC)" when the backend could not determine the user's timezone.
  return timezone ? formatted : `${formatted} (UTC)`;
}
