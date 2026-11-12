import { expect, test } from "@playwright/test";

import { installApi, ready } from "./api";

const openAnalysis = {
  id: "analysis-open",
  model: "llama-3.2",
  engine: "llama.cpp",
  diagnosis: "The heading is present, but the body reads as a fragment.",
  created_at: "2026-11-07T18:00:00Z",
};

const handledAnalysis = {
  id: "analysis-handled",
  model: "llama-3.2",
  engine: "llama.cpp",
  diagnosis: "The profile impression is too generic.",
  created_at: "2026-11-07T18:05:00Z",
  handled_at: "2026-11-07T19:00:00Z",
  handled_by: "ada@example.com",
};

const adminFeedback = [
  {
    version_id: "version-1",
    user_id: "user-ada",
    user_email: "ada@example.com",
    resume_id: "resume-1",
    resume_title: "ada.pdf",
    version_number: 1,
    filename: "ada.pdf",
    section_id: "experience",
    section_heading: "Experience",
    section_kind: "experience",
    section_body: "Built small systems.",
    voted_at: "2026-11-07T18:10:00Z",
    analyses: [openAnalysis],
  },
  {
    version_id: "version-2",
    user_id: "user-ada",
    user_email: "ada@example.com",
    resume_id: "resume-2",
    resume_title: "plain.pdf",
    version_number: 1,
    filename: "plain.pdf",
    section_id: "profile-summary",
    section_heading: "Profile impression",
    section_kind: "summary",
    section_body: "A builder of small systems.",
    voted_at: "2026-11-07T18:15:00Z",
    analyses: [handledAnalysis],
  },
];

test("admin dashboard triage: filters, handles, and reopens", async ({ page }) => {
  await installApi(page, {
    signedIn: true,
    admin: true,
    adminFeedback,
  });

  await page.goto("/admin");
  await ready(page);

  await expect(page.getByRole("heading", { name: "What the model got wrong" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open 1" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Handled 1" })).toBeVisible();
  await expect(page.getByRole("button", { name: "All 2" })).toBeVisible();

  await page.getByRole("button", { name: /All/ }).click();
  await expect(page.getByRole("heading", { name: "Experience" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Profile impression" })).toBeVisible();

  await page.getByRole("button", { name: /Open/ }).click();
  await expect(page.getByRole("heading", { name: "Experience" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Profile impression" })).not.toBeVisible();

  await page.getByRole("button", { name: /Handled/ }).click();
  await expect(page.getByRole("heading", { name: "Profile impression" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Experience" })).not.toBeVisible();

  await page.getByRole("button", { name: /All/ }).click();
  await page.getByLabel("Search feedback").fill("fragment");
  await expect(page.getByRole("heading", { name: "Experience" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Profile impression" })).not.toBeVisible();

  await page.getByLabel("Search feedback").fill("");
  await page.getByRole("button", { name: /All/ }).click();
  await expect(page.getByRole("heading", { name: "Experience" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Profile impression" })).toBeVisible();

  await page.getByRole("button", { name: /Mark handled/ }).first().click();
  await expect(page.getByRole("button", { name: /Reopen/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Handled/ })).toHaveText(/Handled\s*2/);

  await page.getByRole("button", { name: /Reopen/ }).first().click();
  await expect(page.getByRole("button", { name: /Mark handled/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Handled/ })).toHaveText(/Handled\s*1/);

  await page.getByRole("button", { name: /Rerun/ }).first().click();
  await expect(page.getByText("Rerun:")).toBeVisible();
  await expect(page.getByRole("button", { name: /Handled/ })).toHaveText(/Handled\s*1/);
});

test("non-admins cannot open the admin page", async ({ page }) => {
  await installApi(page, { signedIn: true, admin: false, resumes: [] });

  await page.goto("/admin");
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "Your résumés" })).toBeVisible();
});