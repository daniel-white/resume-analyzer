import { expect, test } from "@playwright/test";

import { designed, designedSkills, installApi, openTab, ready, type SeedFrame } from "./api";

const waiting = {
  ...designed,
  id: "resume-waiting",
  versions: [{ ...designed.versions[0], id: "waiting-version", skills: null }],
};

test("stored skills filter by contextual nature and focus their evidence", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await openTab(page, "Skills");
  const cloud = page.getByRole("region", { name: "Skills in this résumé" });
  await expect(cloud).toBeVisible();
  // The evidence map sits in the same bordered, padded box as the model impression.
  await expect(cloud).toHaveCSS("border-width", "1px");
  await expect(cloud).toHaveCSS("border-radius", "14px");
  await expect(cloud.getByTestId("skill-tag")).toHaveCount(6);

  await cloud.getByRole("button", { name: /^People/ }).click();
  await expect(cloud.getByTestId("skill-tag")).toHaveCount(1);
  await expect(cloud.getByTestId("skill-tag")).toHaveAttribute("data-nature", "people");

  await cloud.getByRole("button", { name: /^All/ }).click();
  await expect(cloud.getByTestId("skill-tag")).toHaveCount(6);

  await cloud.getByRole("button", { name: /Analytical engines/ }).click();
  const evidence = cloud.getByRole("region", { name: "Evidence for Analytical engines" });
  await expect(evidence).toContainText("Analytical Engines");
  await expect(evidence).toContainText("Tools · Technical");
  await expect(evidence).toContainText("From Skills");
  await expect(evidence.getByRole("button")).toHaveCount(0);
});

test("version comparison shows skills gained and growing in importance", async ({ page }) => {
  const older = {
    ...designed.versions[0],
    id: "older-version",
    number: 1,
    filename: "older.pdf",
    skills: {
      ...designedSkills,
      skills: designedSkills.skills
        .filter((skill) => skill.canonical !== "stakeholder-education")
        .map((skill) => skill.canonical === "risk-analysis" ? { ...skill, importance: 40 } : skill),
    },
  };
  const versioned = {
    ...designed,
    versions: [{ ...designed.versions[0], id: "latest-version", number: 2 }, older],
  };
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=latest-version`);
  await ready(page);
  await openTab(page, "ATS insights");
  const comparison = page.getByRole("region", { name: "Compare versions", exact: true });
  await comparison.locator("summary").click();
  await expect(comparison).toContainText("Skill signal changes");
  const gained = comparison.getByTestId("skill-change").filter({ hasText: "Stakeholder education" });
  await expect(gained).toBeVisible();
  await expect(gained).toHaveAttribute("title", "People skill");
  await expect(comparison.getByTestId("skill-change").filter({ hasText: "Risk analysis" })).toContainText("+40");
});

test("skills pop in one at a time as stream frames arrive and push the cloud wider", async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 900 });
  const api = await installApi(page, { resumes: [waiting] });
  await page.goto(`/resumes/${waiting.id}`);
  await ready(page);
  await expect.poll(async () => (await api.stats()).streams).toBe(1);
  await openTab(page, "Skills");
  const cloud = page.getByRole("region", { name: "Skills in this résumé" });
  const tags = cloud.getByTestId("skill-tag");
  await expect(tags).toHaveCount(0);

  const skillFrame = (seq: number, name: string, value: Record<string, unknown>): SeedFrame => ({
    resume_id: waiting.id,
    version_id: "waiting-version",
    flow: "skills",
    section_id: null,
    seq,
    event: { type: "field", name, value },
  });
  await api.update({ frames: [
    skillFrame(1, "skill", {
      name: "Triage and patient assessment",
      category: "method", nature: "hard", prominence: 5, confidence: 90,
    }),
  ] });
  await expect(tags).toHaveCount(1);
  await expect(tags.first()).toHaveAttribute("data-live", "true");
  await expect(tags.first()).toHaveAttribute("data-nature", "hard");
  await expect(tags.first()).toHaveCSS("animation-name", "skill-pop-in");
  await expect(cloud).toContainText("Arriving now");
  const before = await cloud.locator(".skill-tags").boundingBox();

  await api.update({ frames: [
    skillFrame(2, "skill", {
      name: "Patient education and family guidance",
      category: "practice", nature: "people", prominence: 4, confidence: 82,
    }),
    skillFrame(3, "skill", {
      name: "Medication reconciliation across shifts",
      category: "method", nature: "hard", prominence: 4, confidence: 78,
    }),
    skillFrame(4, "skill", {
      name: "Infection control protocols",
      category: "domain", nature: "domain", prominence: 3, confidence: 70,
    }),
    skillFrame(5, "skill", {
      name: "Uncertain guess",
      category: "other", nature: "soft", prominence: 1, confidence: 19,
    }),
  ] });
  await expect(tags).toHaveCount(4);
  await expect(cloud).not.toContainText("Uncertain guess");
  await expect(tags.nth(1)).toHaveAttribute("data-nature", "people");
  await expect.poll(async () => (await cloud.locator(".skill-tags").boundingBox())?.height ?? 0)
    .toBeGreaterThan(before?.height ?? 0);
});
