import { expect, test, type Locator, type Page } from "@playwright/test";

import { designed, installApi, openTab, ready } from "./api";

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

function versionTrigger(page: Page) {
  return page.getByRole("button", { name: "Version", exact: true });
}

async function openVersionMenu(page: Page) {
  const trigger = versionTrigger(page);
  await expect(trigger).toBeEnabled();
  await trigger.click();
  await expect(page.getByRole("menu")).toBeVisible();
}

async function expectVersionCount(page: Page, count: number) {
  await openVersionMenu(page);
  await expect(page.getByRole("menu").getByRole("menuitem")).toHaveCount(count);
  await page.keyboard.press("Escape");
}

async function selectVersion(page: Page, filename: string) {
  await openVersionMenu(page);
  await page.getByRole("menuitem").filter({ hasText: filename }).click();
}

async function expandComparison(comparison: Locator) {
  if (await comparison.locator("details").getAttribute("open") === null) {
    await comparison.locator("summary").click();
  }
}

test("comparison is optional and only compares forward in time", async ({ page }, testInfo) => {
  const third = { ...older, id: "third-version", number: 3, filename: "third.pdf",
    runs: older.runs.map((run) => ({ ...run, verdict: run.verdict ? { ...run.verdict, score: 95 } : null })) };
  await installApi(page, { resumes: [{ ...versioned, versions: [third, ...versioned.versions] }] });
  await page.goto(`/resumes/${designed.id}?version=latest-version`);
  await ready(page);
  await openTab(page, "ATS insights");
  const comparison = page.getByRole("region", { name: "Compare versions", exact: true });
  await expect(comparison.getByText("Compare versions", { exact: true })).toBeVisible();
  await expect(comparison.getByRole("table")).not.toBeVisible();
  await comparison.locator("summary").click();
  await expect(comparison.getByLabel("From version")).toHaveValue("older-version");
  await expect(comparison.getByLabel("To version")).toHaveValue("latest-version");
  await expect(comparison.getByLabel("From version").locator("option[value='third-version']")).toHaveCount(0);
  await expect(comparison.getByLabel("To version").locator("option[value='older-version']")).toHaveCount(0);
  await expect(comparison.getByRole("status")).toContainText("Down 42.5 points");
  await expect(comparison.getByRole("status")).toContainText("not a setback");
  await expect(comparison.getByRole("row", { name: /Greenline/ })).toContainText("Down 60 points");
  await comparison.getByLabel("From version").selectOption("latest-version");
  await expect(comparison.getByLabel("To version")).toHaveValue("third-version");
  await expect(comparison.getByRole("status")).toContainText("Up 42.5 points");
  await expect(comparison.getByRole("status")).toContainText("Nice progress");
  await comparison.getByLabel("From version").selectOption("older-version");
  await expect(comparison.getByRole("status")).toContainText("Unchanged");
  await expect(comparison.getByRole("status")).toContainText("holding steady");
  await comparison.screenshot({ path: testInfo.outputPath("version-comparison.png") });
  await comparison.locator("summary").click();
  await expect(comparison.getByRole("table")).not.toBeVisible();
  await page.reload();
  await expect(comparison.getByRole("table")).not.toBeVisible();
  await selectVersion(page, "older.pdf");
  await expect(page).toHaveURL(/\/ats-insights/);
  await expect(page).toHaveURL(/version=older-version/);
  await comparison.locator("summary").click();
  await expect(comparison.getByLabel("From version")).toHaveValue("older-version");
  await expect(comparison.getByLabel("To version")).toHaveValue("latest-version");
});

test("pending comparison results are not counted as a score drop", async ({ page }) => {
  const pending = { ...designed.versions[0], id: "pending-version", number: 2, status: "running" as const, runs: [] };
  const api = await installApi(page, { resumes: [{ ...versioned, versions: [pending, older] }] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await openTab(page, "ATS insights");
  const comparison = page.getByRole("region", { name: "Compare versions", exact: true });
  await comparison.locator("summary").click();
  await expect(comparison.getByRole("status")).toContainText("Based on 0 systems");
  await expect(comparison.getByRole("status")).toContainText("Keep going");
  await expect(comparison.getByRole("row", { name: /Greenline/ })).toContainText("Awaiting results");
  await expect.poll(async () => (await api.stats()).streams).toBe(1);
  const readsBeforeUpdate = (await api.stats()).detailReads;
  await api.update({ resumes: [versioned], event: "AtsCallbackReceived" });
  await expect.poll(async () => (await api.stats()).detailReads).toBeGreaterThan(readsBeforeUpdate);
  await expandComparison(comparison);
  await expect(comparison.getByRole("status")).toContainText("Down 42.5 points");
});

test("one version offers an encouraging comparison empty state", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await openTab(page, "ATS insights");
  const comparison = page.getByRole("region", { name: "Compare versions", exact: true });
  await comparison.locator("summary").click();
  await expect(comparison).toContainText("great starting point");
});

test("selects version-specific results and preserves selection on reload", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await openTab(page, "ATS insights");
  const greenline = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Greenline" }) });
  await expect(greenline.getByLabel("Grade F", { exact: true })).toBeVisible();
  await selectVersion(page, "older.pdf");
  await expect(page).toHaveURL(/version=older-version/);
  await expect(page).toHaveURL(/\/ats-insights/);
  await expect(versionTrigger(page)).toContainText("v1");
  await expect(page.getByRole("heading", { name: designed.title, exact: true })).toBeVisible();
  await expect(page.getByText("File: older.pdf", { exact: true })).toBeVisible();
  await expect(page.getByRole("link", { name: "Download original" })).toHaveAttribute("download", "older.pdf");
  await expect(greenline.getByLabel("Grade A", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Overall rating", { exact: true })).toHaveText("A");
  await expect(page.getByLabel("Grade summary", { exact: true })).toContainText("Excellent");
  await page.reload();
  await expect(versionTrigger(page)).toContainText("v1");
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
  await expect(page.getByRole("heading", { name: designed.title, exact: true })).toBeVisible();
  await expect(page.getByText("File: revised.pdf", { exact: true })).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: "New version uploaded." })).toBeVisible();
  await expectVersionCount(page, 2);
  await openTab(page, "ATS insights");
  await expect(page.getByLabel("Pending").first()).toBeVisible();
  await selectVersion(page, designed.versions[0].filename);
  await expect(page.getByRole("heading", { name: designed.versions[0].filename })).toBeVisible();
  await expect(page.getByRole("article", { name: "Greenline", exact: true }).getByLabel("Grade F", { exact: true })).toBeVisible();
});

test("cancels deletion, then deletes only the selected version", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=older-version`);
  await ready(page);
  const remove = page.getByRole("button", { name: "Delete this version" });
  await remove.click();
  await expect(page.getByRole("alertdialog")).toContainText("older.pdf");
  await expect(page.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(remove).toBeFocused();
  await expectVersionCount(page, 2);
  await remove.click();
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page.getByRole("heading", { name: designed.versions[0].filename })).toBeVisible();
  await expectVersionCount(page, 1);
  await page.reload();
  await expectVersionCount(page, 1);
});

test("offers only the scoped delete action on the detail page", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  const remove = page.getByRole("button", { name: "Delete this version" });
  await expect(remove).toBeVisible();
  await expect(page.getByRole("button", { name: "More delete options" })).toHaveCount(0);
  await expect(page.getByRole("menuitem", { name: "Delete all versions" })).toHaveCount(0);
  await versionTrigger(page).click();
  const menu = page.getByRole("menu");
  await expect(menu).toContainText("Version history");
  await expect(menu).not.toContainText("Delete");
  await page.keyboard.press("Escape");
});

test("deleting the last version removes the resume", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByRole("button", { name: "Delete this version" }).click();
  await expect(page.getByRole("alertdialog")).toContainText("last version");
  await page.getByRole("button", { name: "Confirm deletion" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByText("No résumés yet.")).toBeVisible();
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
  await expect(page.getByText("No résumés yet.")).toBeVisible();
  await page.reload();
  await expect(page.getByText("No résumés yet.")).toBeVisible();
});

test("a missing version does not show another version's results", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=deleted-version`);
  await expect(page.getByRole("heading", { name: "That résumé is not here." })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Greenline" })).toHaveCount(0);
});

test("uses a keyboard-friendly versions menu and dismisses confirmation with Escape", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  const trigger = versionTrigger(page);
  await expect(trigger).toContainText("v2");
  await expect(trigger).not.toHaveCSS("appearance", "base-select");
  await expect(page.getByText("Loading original file…", { exact: true })).toHaveCount(0);
  await trigger.focus();
  await page.keyboard.press("Enter");
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem")).toHaveCount(2);
  await page.keyboard.press("ArrowDown");
  await expect(menu.getByRole("menuitem").filter({ hasText: "older.pdf" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/version=older-version/);
  const remove = page.getByRole("button", { name: "Delete this version" });
  await remove.click();
  await expect(page.getByRole("alertdialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(remove).toBeFocused();
});

test("tabs are reloadable paths that keep search and the shared preview", async ({ page }) => {
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}?version=older-version&provider=greenline`);
  await ready(page);
  const tabs = page.getByRole("navigation", { name: "Analysis sections" });
  await expect(tabs.getByRole("link")).toHaveCount(4);
  await expect(tabs).not.toContainText("Impression");
  await openTab(page, "Structure");
  await expect(page).toHaveURL(/\/structure\?version=older-version&provider=greenline/);
  await expect(page.getByLabel("Résumé details", { exact: true })).toBeVisible();
  await expect(page.getByLabel("Original résumé")).toBeVisible();
  await openTab(page, "Skills");
  await expect(page).toHaveURL(/\/skills\?version=older-version&provider=greenline/);
  await expect(page.getByRole("region", { name: "Skills in this résumé" })).toBeVisible();
  await openTab(page, "ATS insights");
  await expect(page).toHaveURL(/\/ats-insights\?version=older-version&provider=greenline/);
  await expect(page.getByRole("heading", { name: "ATS insights" })).toBeVisible();
  await page.reload();
  await expect(page).toHaveURL(/\/ats-insights\?version=older-version&provider=greenline/);
  await expect(page.getByRole("heading", { name: "ATS insights" })).toBeVisible();
  await expect(page.getByLabel("Original résumé")).toBeVisible();
});

test("keeps the detail page and confirmation within a mobile viewport", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installApi(page, { resumes: [versioned] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await expect(page.getByRole("link", { name: /View original/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "More delete options" })).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("resume-details-mobile.png"), fullPage: true, caret: "initial" });
  await openTab(page, "ATS insights");
  await expect(page.getByLabel("Grade summary")).toContainText("Grade range");
  await expect(page.getByLabel("Grade summary")).toContainText("F–C-");
  await expect(page.getByLabel("Overall rating", { exact: true })).toHaveText("F");
  await expect(page.getByLabel("Grade summary", { exact: true })).toContainText("Needs work");
  const comparison = page.getByRole("region", { name: "Compare versions", exact: true });
  await comparison.locator("summary").click();
  await expect(comparison.getByRole("table")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("ats-insights-mobile.png"), fullPage: true, caret: "initial" });
  await openTab(page, "Overview");
  await page.getByRole("button", { name: "Delete this version" }).click();
  const bounds = await page.getByRole("alertdialog").boundingBox();
  if (!bounds) throw new Error("confirmation dialog has no bounds");
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
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
