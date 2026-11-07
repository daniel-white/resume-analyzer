import { expect, test } from "@playwright/test";

import { designed, installApi, ready } from "./api";

const older = {
  ...designed.versions[0], id: "older-version", number: 1, filename: "older.pdf",
  runs: designed.versions[0].runs.map((run) => ({
    ...run, verdict: run.verdict ? { ...run.verdict, score: 95 } : null,
  })),
};
const versioned = {
  ...designed,
  versions: [{ ...designed.versions[0], id: "latest-version", number: 2 }, older],
};

test("selects version-specific results and preserves selection on reload", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  const greenline = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Greenline" }) });
  await expect(greenline.getByLabel("Grade F", { exact: true })).toBeVisible();
  await page.getByLabel("Version", { exact: true }).selectOption("older-version");
  await expect(page).toHaveURL(/version=older-version/);
  await expect(page.getByRole("heading", { name: "older.pdf" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Download original" })).toHaveAttribute("download", "older.pdf");
  await expect(greenline.getByLabel("Grade A", { exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByLabel("Version", { exact: true })).toHaveValue("older-version");
  await expect(greenline.getByLabel("Grade A", { exact: true })).toBeVisible();
});

test("uploads a new version of the same resume and can return to old results", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByLabel("Upload new version").setInputFiles({
    name: "revised.pdf", mimeType: "application/pdf", buffer: Buffer.from("%PDF-1.4 revised"),
  });
  await expect(page).toHaveURL(new RegExp(`/resumes/${designed.id}\\?version=`));
  await expect(page.getByRole("heading", { name: "revised.pdf" })).toBeVisible();
  await expect(page.getByLabel("Version", { exact: true }).locator("option")).toHaveCount(2);
  await expect(page.getByLabel("Pending").first()).toBeVisible();
  await page.getByLabel("Version", { exact: true }).selectOption(designed.versions[0].id);
  await expect(page.getByRole("heading", { name: designed.versions[0].filename })).toBeVisible();
  await expect(page.getByLabel("Grade F", { exact: true })).toBeVisible();
});

test("cancels deletion, then deletes only the selected version", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=older-version`);
  await ready(page);
  await page.getByRole("button", { name: "Delete this version" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("older.pdf");
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByLabel("Version", { exact: true }).locator("option")).toHaveCount(2);
  await page.getByRole("button", { name: "Delete this version" }).click();
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page.getByRole("heading", { name: designed.versions[0].filename })).toBeVisible();
  await expect(page.getByLabel("Version", { exact: true }).locator("option")).toHaveCount(1);
  await page.reload();
  await expect(page.getByLabel("Version", { exact: true }).locator("option")).toHaveCount(1);
});

test("deletes all versions and returns to the library", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByRole("button", { name: "Delete all versions" }).click();
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByText("Nothing stored yet.")).toBeVisible();
});

test("deleting the last version removes the resume", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByRole("button", { name: "Delete this version" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("last version");
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByText("Nothing stored yet.")).toBeVisible();
});

test("a missing version does not show another version's results", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=deleted-version`);
  await expect(page.getByRole("heading", { name: "That resume is not here." })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Greenline" })).toHaveCount(0);
});
