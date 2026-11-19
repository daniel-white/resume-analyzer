import { ArrowRight, Tags } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { liveKey, seedRun } from "~/lib/generation";
import {
  CATEGORY_LABELS,
  NATURES,
  NATURE_LABELS,
  filterCloud,
  liveCloud,
  needsEvidence,
  storedCloud,
  tagStyle,
  type CloudSkill,
} from "~/lib/skills";
import { cn } from "~/lib/utils";
import type { SectionMap, SkillNature, SkillSet } from "~/server/resumes";
import { GenerationProgress } from "./generation-progress";
import { useLiveRun } from "./live-analysis";

// Layout reads box positions before paint, but the page also renders on the server.
const useMeasureEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;

/**
 * Each streamed skill arrives as its own SSE frame. New tags pop in and the
 * neighbours slide to their new places (a small FLIP animation) so the cloud
 * visibly makes room instead of snapping to a new layout. The nature chips are
 * a second, job-agnostic axis over the same tags.
 */
export function SkillCloud({
  skills,
  sections,
  versionId,
  pending = false,
  onFocusSection,
}: {
  skills: SkillSet | null;
  sections: SectionMap | null;
  versionId: string;
  pending?: boolean;
  onFocusSection: (sectionId: string, heading: string) => void;
}) {
  const live = useLiveRun(liveKey("skills", versionId));
  const streaming = Boolean(live && !live.done && !live.error);
  const liveTags = liveCloud(live?.skills ?? []);
  const stored = storedCloud(skills);
  const tags = streaming && liveTags.length > 0 ? liveTags : stored;
  const progressRun = live ?? (pending && stored.length === 0 ? seedRun(liveKey("skills", versionId)) : undefined);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [filter, setFilter] = useState<SkillNature | "all">("all");
  const nodes = useRef<Map<string, HTMLElement>>(new Map());
  const positions = useRef<Map<string, DOMRect>>(new Map());
  const visible = filterCloud(tags, filter);

  useMeasureEffect(() => {
    if (typeof window === "undefined") return;
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const next = new Map<string, DOMRect>();
    for (const [key, node] of nodes.current) {
      const rect = node.getBoundingClientRect();
      const previous = positions.current.get(key);
      const moved = previous !== undefined
        && (Math.abs(previous.left - rect.left) > 0.5 || Math.abs(previous.top - rect.top) > 0.5);
      if (!still && moved && typeof node.animate === "function") {
        node.animate(
          [
            { transform: `translate(${previous.left - rect.left}px, ${previous.top - rect.top}px)` },
            { transform: "translate(0, 0)" },
          ],
          { duration: 340, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
        );
      }
      next.set(key, rect);
    }
    positions.current = next;
  }, [visible]);

  const selected = skills?.skills.find(
    (skill) => (skill.canonical || skill.name.toLowerCase()) === selectedKey,
  );
  const unproven = (skills?.skills ?? []).filter((skill) => needsEvidence(skill, sections));

  function toggle(tag: CloudSkill) {
    if (tag.live) return;
    setSelectedKey((current) => (current === tag.key ? null : tag.key));
  }

  return (
    <section
      className={cn("card skill-cloud", streaming && "is-streaming")}
      aria-labelledby="skill-cloud-heading"
    >
      <div className="skill-cloud-heading">
        <span className="skill-cloud-icon"><Tags size={20} aria-hidden="true" /></span>
        <div>
          <p className="skill-cloud-eyebrow">
            {streaming ? "Arriving now" : "Evidence map"}
          </p>
          <h2 id="skill-cloud-heading">Skills in this résumé</h2>
        </div>
      </div>
      {progressRun ? <GenerationProgress run={progressRun} compact /> : null}
      {tags.length > 0 ? (
        <>
          <div className="skill-filters" role="group" aria-label="Filter skills by nature">
            <button
              type="button"
              data-testid="skill-filter"
              className={cn("skill-filter", filter === "all" && "is-active")}
              aria-pressed={filter === "all"}
              onClick={() => setFilter("all")}
            >
              All <span className="skill-filter-count">{tags.length}</span>
            </button>
            {NATURES.map((item) => {
              const count = tags.filter((tag) => tag.nature === item).length;
              return (
                <button
                  type="button"
                  key={item}
                  data-testid="skill-filter"
                  className={cn("skill-filter", `skill-filter--${item}`, filter === item && "is-active")}
                  aria-pressed={filter === item}
                  onClick={() => setFilter(item)}
                  disabled={count === 0}
                >
                  {NATURE_LABELS[item]} <span className="skill-filter-count">{count}</span>
                </button>
              );
            })}
          </div>
          {visible.length > 0 ? (
            <ul className="skill-tags" aria-label="Extracted skills">
              {visible.map((tag) => {
                const label = `${tag.name}, ${CATEGORY_LABELS[tag.category].toLowerCase()}, ${NATURE_LABELS[tag.nature].toLowerCase()}, importance ${tag.importance} of 100, confidence ${tag.confidence} percent`;
                return (
                  <li
                    key={tag.key}
                    ref={(node) => {
                      if (node) nodes.current.set(tag.key, node);
                      else nodes.current.delete(tag.key);
                    }}
                  >
                    {tag.live ? (
                      <span
                        className={cn("skill-tag", "is-live", `skill-tag--${tag.nature}`)}
                        data-testid="skill-tag"
                        data-nature={tag.nature}
                        data-live="true"
                        style={tagStyle(tag)}
                        title={label}
                      >
                        {tag.name}
                      </span>
                    ) : (
                      <button
                        type="button"
                        className={cn(
                          "skill-tag",
                          `skill-tag--${tag.category}`,
                          `skill-tag--${tag.nature}`,
                          selectedKey === tag.key && "is-selected",
                        )}
                        data-testid="skill-tag"
                        data-nature={tag.nature}
                        style={tagStyle(tag)}
                        aria-pressed={selectedKey === tag.key}
                        aria-label={label}
                        onClick={() => toggle(tag)}
                      >
                        {tag.name}
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          ) : (
            <p className="skill-pending">No skills in this group.</p>
          )}
          <p className="skill-note">
            {streaming
              ? "Tags appear as the model finds them. Quotes are checked before anything is saved."
              : "Size shows importance in this résumé; lighter text shows lower confidence. Filter by how each skill shows up."}
          </p>
          {!streaming && unproven.length > 0 ? (
            <p className="skill-note skill-note--warn">
              Needs a concrete example: {unproven.map((skill) => skill.name).join(", ")}.
            </p>
          ) : null}
          {selected ? (
            <div className="skill-evidence" role="region" aria-label={`Evidence for ${selected.name}`}>
              <p className="skill-evidence-heading">
                Where this skill shows up
                <span className="skill-evidence-kind">
                  {CATEGORY_LABELS[selected.category]} · {NATURE_LABELS[selected.nature]}
                </span>
              </p>
              <ul>
                {selected.evidence.map((item, index) => {
                  const section = sections?.sections.find((entry) => entry.id === item.section_id);
                  const heading = section?.heading ?? "résumé details";
                  return (
                    <li key={`${item.section_id}-${index}`}>
                      <blockquote>{item.quote}</blockquote>
                      <button type="button" onClick={() => onFocusSection(item.section_id, heading)}>
                        Show in {heading} <ArrowRight size={14} aria-hidden="true" />
                      </button>
                    </li>
                  );
                })}
              </ul>
            </div>
          ) : null}
        </>
      ) : (
        <p className="skill-pending">
          {progressRun
            ? "Reading your résumé for skills. Tags will appear here one at a time."
            : "No skills were found in this version yet."}
        </p>
      )}
    </section>
  );
}
