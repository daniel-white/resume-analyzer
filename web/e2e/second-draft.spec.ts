import { expect, test } from "@playwright/test";

import { designed, installApi, ready, type SeedChecklistItem, type SeedResume } from "./api";

function item(overrides: Partial<SeedChecklistItem> & Pick<SeedChecklistItem, "id" | "kind" | "title">): SeedChecklistItem {
  return {
    fingerprint: `fp-${overrides.id}`,
    detail: "Your line: \"Triaged patients in the busy emergency department.\"",
    why: "A duty rather than a result.",
    placeholder: "how many · how often · what changed",
    section_id: "experience-1",
    section_heading: "Experience",
    quote: "Triaged patients in the busy emergency department.",
    template: "Triaged patients — ___",
    cut_text: null,
    position: 0,
    done: false,
    answer: null,
    checked_at: null,
    created_at: "2026-11-07T18:00:00Z",
    ...overrides,
  };
}

const withChecklist: SeedResume = {
  ...designed,
  versions: [{
    ...designed.versions[0],
    checklist: [
      item({ id: "blank-1", kind: "blank", title: "Add a number to Triage" }),
      item({ id: "gold-1", kind: "promote", title: "Bring Triage up top", template: "Triage — ___" }),
      item({
        id: "cut-1",
        kind: "cut",
        title: "Cut fluff",
        template: null,
        quote: null,
        cut_text: "Hard-working team player with a passion for excellence.",
      }),
      item({
        id: "flag-1",
        kind: "flag",
        title: "Flag: Blaming a former manager",
        detail: "Found in Experience: \"My old manager never explained the process.\"",
        why: "It puts the focus on a person instead of your work.",
        placeholder: "Describe the gap without naming anyone.",
        quote: "My old manager never explained the process.",
        template: null,
        position: 3,
      }),
    ],
  }],
};

test("the second draft groups edits and keeps a tick and note across a reload", async ({ page }) => {
  await installApi(page, { resumes: [withChecklist] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);

  const panel = page.getByRole("region", { name: "Your editing checklist" });
  await expect(panel).toBeVisible();
  await expect(panel).toHaveCSS("border-width", "1px");
  await expect(panel).toHaveCSS("border-radius", "14px");
  await expect(panel.getByText("Fill the blanks")).toBeVisible();
  await expect(panel.getByText("Bring up the gold")).toBeVisible();
  await expect(panel.getByText("Cut the fluff")).toBeVisible();
  await expect(panel.getByText("Flag bad practices")).toBeVisible();
  const rows = panel.getByTestId("second-draft-item");
  await expect(rows).toHaveCount(4);
  await expect(panel).toContainText("0 of 4 done");

  const first = rows.first();
  await first.getByRole("checkbox").check();
  await expect(first).toHaveClass(/is-done/);
  await expect(panel).toContainText("1 of 4 done");

  const note = first.getByRole("textbox");
  await note.fill("40 patients per shift");
  await note.blur();
  await expect(first).toContainText("40 patients per shift");

  await page.reload();
  await ready(page);
  const reloaded = page.getByRole("region", { name: "Your editing checklist" }).getByTestId("second-draft-item").first();
  await expect(reloaded).toHaveClass(/is-done/);
  await expect(reloaded).toContainText("40 patients per shift");
  await expect(page.getByRole("region", { name: "Your editing checklist" })).toContainText("1 of 4 done");
});
