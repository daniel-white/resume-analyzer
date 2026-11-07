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
  await expect(page.getByLabel("Overall rating", { exact: true })).toContainText("A");
  await expect(page.getByLabel("Overall rating", { exact: true })).toContainText("Excellent");
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
  await expect(page.getByRole("status").filter({ hasText: "New version uploaded." })).toBeVisible();
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
  await expect(page.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("button", { name: "Delete this version" })).toBeFocused();
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
  await page.getByRole("button", { name: "More delete options" }).click();
  await page.getByRole("menuitem", { name: "Delete all versions" }).click();
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByText("No resumes yet.")).toBeVisible();
});

test("deleting the last version removes the resume", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByRole("button", { name: "Delete this version" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("last version");
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByText("No resumes yet.")).toBeVisible();
});

test("deletes all versions from a library row without opening the resume", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto("/");
  await ready(page);
  const trigger = page.getByRole("button", { name: `Delete ${designed.versions[0].filename}`, exact: true });
  await trigger.click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("alertdialog")).toContainText("all its versions");
  await expect(page.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(trigger).toBeFocused();
  await expect(page.getByRole("link", { name: new RegExp(designed.versions[0].filename) })).toBeVisible();
  await trigger.click();
  await page.getByRole("button", { name: "Delete all versions", exact: true }).click();
  await expect(page.getByText("No resumes yet.")).toBeVisible();
  await page.reload();
  await expect(page.getByText("No resumes yet.")).toBeVisible();
});

test("a missing version does not show another version's results", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=deleted-version`);
  await expect(page.getByRole("heading", { name: "That resume is not here." })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Greenline" })).toHaveCount(0);
});

test("uses the native customizable picker and dismisses confirmation with Escape", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  const picker = page.getByLabel("Version", { exact: true });
  await expect(page.getByText("Loading original file…", { exact: true })).toHaveCount(0);
  await expect(picker).toHaveCSS("appearance", "base-select");
  await picker.click();
  await expect(picker).toHaveJSProperty("value", "latest-version");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/version=older-version/);
  await page.getByRole("button", { name: "More delete options" }).click();
  await page.getByRole("menuitem", { name: "Delete all versions" }).click();
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "More delete options" })).toBeFocused();
});

test("keeps the detail page and confirmation within a mobile viewport", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await expect(page.getByLabel("Analysis overview")).toContainText("Grade range");
  await expect(page.getByLabel("Analysis overview")).toContainText("F–C-");
  await expect(page.getByLabel("Overall rating", { exact: true })).toContainText("F");
  await expect(page.getByLabel("Overall rating", { exact: true })).toContainText("Needs work");
  await expect(page.getByRole("menuitem", { name: "Delete all versions" })).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("resume-details-mobile.png"), fullPage: true, caret: "initial" });
  await page.getByRole("button", { name: "Delete this version" }).click();
  const bounds = await page.getByRole("alertdialog").boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("confirmation-mobile.png"), caret: "initial" });
});

test("shows a document skeleton while the original file is loading", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  let release!: () => void;
  const hold = new Promise<void>((resolve) => { release = resolve; });
  await page.route("**/api/resumes/*/versions/*/file", async (route) => {
    await hold;
    await route.continue();
  });
  await page.goto(`/resumes/${designed.id}`);
  await expect(page.getByRole("status").filter({ hasText: "Loading original file…" })).toBeVisible();
  await expect(page.locator(".skeleton-document")).toBeVisible();
  release();
  await expect(page.getByText("Loading original file…", { exact: true })).toHaveCount(0);
});
