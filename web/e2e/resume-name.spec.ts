import { expect, test } from "@playwright/test";

import { designed, installApi, ready } from "./api";

test("names a new set from its upload and lets the user rename it", async ({ page }, testInfo) => {
  await installApi(page, { resumes: [] });
  await page.goto("/");
  await ready(page);
  await page.locator('input[type="file"]').setInputFiles({
    name: "engineering.pdf", mimeType: "application/pdf", buffer: Buffer.from("%PDF-1.4 resume"),
  });
  await expect(page.getByRole("heading", { name: "engineering.pdf", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Rename résumé" }).click();
  await expect(page.getByLabel("Give your résumé a name")).toBeFocused();
  await expect(page.getByLabel("Give your résumé a name")).toHaveValue("engineering.pdf");
  await page.getByLabel("Give your résumé a name").fill("  Engineering roles  ");
  await page.getByRole("button", { name: "Save name" }).click();
  await expect(page.getByRole("heading", { name: "Engineering roles", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename résumé" })).toBeFocused();
  await expect(page.getByRole("link", { name: "Download original" })).toHaveAttribute("download", "engineering.pdf");
  await page.screenshot({ path: testInfo.outputPath("renamed-set.png"), fullPage: true });
  await page.reload();
  await ready(page);
  await expect(page.getByRole("heading", { name: "Engineering roles", exact: true })).toBeVisible();
  await page.getByLabel("Upload new version").setInputFiles({
    name: "revised.pdf", mimeType: "application/pdf", buffer: Buffer.from("%PDF-1.4 revised"),
  });
  await expect(page.getByText("File: revised.pdf", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Engineering roles", exact: true })).toBeVisible();
  await page.goto("/");
  await expect(page.getByRole("link", { name: /Engineering roles/ })).toContainText("revised.pdf");
  await expect(page.getByRole("button", { name: "Delete Engineering roles", exact: true })).toBeVisible();
});

test("cancels edits and rejects blank or overlong names", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByRole("button", { name: "Rename résumé" }).click();
  await page.getByLabel("Give your résumé a name").fill("Discard this");
  await page.getByLabel("Give your résumé a name").press("Escape");
  await expect(page.getByRole("heading", { name: designed.title, exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Rename résumé" }).click();
  await expect(page.getByLabel("Give your résumé a name")).toHaveValue(designed.title);
  for (const name of ["   ", "x".repeat(201)]) {
    await page.getByLabel("Give your résumé a name").fill(name);
    await page.getByRole("button", { name: "Save name" }).click();
    await expect(page.getByRole("alert")).toContainText("between 1 and 200 characters");
  }
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.getByRole("heading", { name: designed.title, exact: true })).toBeVisible();
});

test("keeps the name editor usable on mobile", async ({ page }, testInfo) => {
  await installApi(page, { resumes: [designed] });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await page.getByRole("button", { name: "Rename résumé" }).click();
  await page.getByLabel("Give your résumé a name").fill("My next chapter");
  await expect(page.getByRole("button", { name: "Save name" })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("resume-name-editor-mobile.png"), fullPage: true });
  await page.getByRole("button", { name: "Save name" }).click();
  await expect(page.getByRole("heading", { name: "My next chapter", exact: true })).toBeVisible();
});
