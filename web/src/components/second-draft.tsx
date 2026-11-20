import { CheckSquare2, Scissors, Sparkles, SquarePen } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";

import { saveChecklistItem } from "~/fns/resumes.functions";
import { cn } from "~/lib/utils";
import type { ChecklistItem, ChecklistKind } from "~/server/resumes";

const GROUPS: {
  kind: ChecklistKind;
  title: string;
  intro: string;
  icon: ReactNode;
}[] = [
  {
    kind: "blank",
    title: "Fill the blanks",
    intro: "Your own wording, with room for the fact only you know.",
    icon: <SquarePen size={20} aria-hidden="true" />,
  },
  {
    kind: "promote",
    title: "Bring up the gold",
    intro: "Strong proof that is hiding too far down the page.",
    icon: <Sparkles size={20} aria-hidden="true" />,
  },
  {
    kind: "cut",
    title: "Cut the fluff",
    intro: "Lines that cost space without showing a result.",
    icon: <Scissors size={20} aria-hidden="true" />,
  },
];

/** The stored item plus the uncommitted text in its answer box. */
type Row = ChecklistItem & { draft: string };

function toRows(items: ChecklistItem[]): Row[] {
  return items.map((item) => ({ ...item, draft: item.answer ?? "" }));
}

/** Show the writer's answer where the generator left a `___` hole. */
function templateNodes(template: string, answer: string): ReactNode[] {
  const parts = template.split("___");
  const nodes: ReactNode[] = [];
  parts.forEach((part, index) => {
    nodes.push(part);
    if (index < parts.length - 1) {
      nodes.push(
        <strong key={`fill-${index}`} className="second-draft-fill">
          {answer.trim() || "___"}
        </strong>,
      );
    }
  });
  return nodes;
}

/**
 * A per-version writing checklist that behaves like a todo list: toggle an
 * item, write an optional note, and both survive a reload. Every suggestion is
 * grounded in the document, so the page only asks for facts the writer has.
 */
export function SecondDraft({
  items,
  resumeId,
  versionId,
  pending = false,
  onFocusSection,
}: {
  items: ChecklistItem[];
  resumeId: string;
  versionId: string;
  pending?: boolean;
  onFocusSection: (sectionId: string, heading: string) => void;
}) {
  const [rows, setRows] = useState<Row[]>(() => toRows(items));
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);

  // A refreshed route (or a different version) replaces the whole list.
  useEffect(() => {
    setRows(toRows(items));
    setError(null);
  }, [items]);

  const doneCount = rows.filter((row) => row.done).length;
  const percent = rows.length === 0 ? 0 : Math.round((doneCount / rows.length) * 100);

  function patch(id: string, changes: Partial<Row>) {
    setRows((current) => current.map((row) => (row.id === id ? { ...row, ...changes } : row)));
  }

  async function persist(row: Row, done: boolean, answer: string) {
    const before = { done: row.done, answer: row.answer, draft: row.draft };
    patch(row.id, { done, answer, draft: answer });
    setBusy((current) => ({ ...current, [row.id]: true }));
    setError(null);
    try {
      const result = await saveChecklistItem({
        data: { resumeId, versionId, itemId: row.id, done, answer },
      });
      if (!result.ok) {
        patch(row.id, before);
        setError(result.message);
        return;
      }
      const saved = result.item.answer ?? "";
      patch(row.id, { done: result.item.done, answer: saved, draft: saved });
    } catch {
      patch(row.id, before);
      setError("Could not save your checklist. Please try again.");
    } finally {
      setBusy((current) => {
        const next = { ...current };
        delete next[row.id];
        return next;
      });
    }
  }

  function toggle(row: Row) {
    if (busy[row.id]) return;
    void persist(row, !row.done, row.draft.trim());
  }

  function saveAnswer(row: Row) {
    const answer = row.draft.trim();
    if (busy[row.id] || answer === (row.answer ?? "")) return;
    void persist(row, row.done, answer);
  }

  return (
    <section className="second-draft" aria-labelledby="second-draft-heading">
      <div className="second-draft-heading">
        <span className="second-draft-icon"><CheckSquare2 size={20} aria-hidden="true" /></span>
        <div>
          <p className="second-draft-eyebrow">This version · Second draft</p>
          <h2 id="second-draft-heading">Your editing checklist</h2>
        </div>
      </div>

      {rows.length > 0 ? (
        <>
          <div className="second-draft-progress" aria-live="polite">
            <div className="second-draft-progress-track">
              <div className="second-draft-progress-fill" style={{ width: `${percent}%` }} />
            </div>
            <span className="second-draft-progress-label">
              {doneCount} of {rows.length} done
            </span>
          </div>

          {GROUPS.map((group) => {
            const groupItems = rows.filter((row) => row.kind === group.kind);
            if (groupItems.length === 0) return null;
            return (
              <div className={`second-draft-group second-draft-group--${group.kind}`} key={group.kind}>
                <div className="second-draft-group-heading">
                  <span className="second-draft-group-icon">{group.icon}</span>
                  <div>
                    <h3>{group.title}</h3>
                    <p>{group.intro}</p>
                  </div>
                </div>
                <ul className="second-draft-list">
                  {groupItems.map((row) => {
                    const sectionId = row.section_id;
                    const sectionHeading = row.section_heading;
                    return (
                      <li
                        className={cn("second-draft-item", row.done && "is-done")}
                        key={row.id}
                        data-testid="second-draft-item"
                        data-kind={row.kind}
                      >
                        <label className="second-draft-check">
                          <input
                            type="checkbox"
                            checked={row.done}
                            disabled={Boolean(busy[row.id])}
                            onChange={() => toggle(row)}
                            aria-label={row.title}
                          />
                          <span className="second-draft-label">{row.title}</span>
                        </label>
                        <div className="second-draft-body">
                          <p className="second-draft-detail">{row.detail}</p>
                          <p className="second-draft-why">{row.why}</p>
                          {row.template ? (
                            <p className="second-draft-preview">
                              {templateNodes(row.template, row.draft)}
                            </p>
                          ) : null}
                          {row.cut_text ? (
                            <p className="second-draft-cut">{row.cut_text}</p>
                          ) : null}
                          <textarea
                            className="second-draft-answer"
                            value={row.draft}
                            placeholder={row.placeholder}
                            aria-label={`Your note for: ${row.title}`}
                            rows={2}
                            onChange={(event) => patch(row.id, { draft: event.target.value })}
                            onBlur={() => saveAnswer(row)}
                          />
                          {sectionId && sectionHeading ? (
                            <button
                              type="button"
                              className="second-draft-focus"
                              onClick={() => onFocusSection(sectionId, sectionHeading)}
                            >
                              Show in {sectionHeading}
                            </button>
                          ) : null}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              </div>
            );
          })}
          {error ? <p className="second-draft-error" role="alert">{error}</p> : null}
          <p className="second-draft-note">
            Saved with this version. Your ticks and notes stay put if the résumé is read again.
          </p>
        </>
      ) : (
        <p className="second-draft-pending">
          {pending
            ? "Building your checklist from the sections and skills we found. It will appear here in a moment."
            : "No edits suggested for this version yet."}
        </p>
      )}
    </section>
  );
}
