import { expect, test } from "@playwright/test";

import { installApi, ready } from "./api";

test("about page uses tabs for build details, license, and release notes", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/about");
  await ready(page);

  await expect(page.getByRole("heading", { name: "About The Other Read" })).toBeVisible();
  await expect(page.getByText("LOCAL DEV BUILD")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);

  for (const name of ["General", "Changelog", "License", "Build"]) {
    await expect(page.getByRole("tab", { name })).toBeVisible();
  }
  await expect(page.getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");

  await page.getByRole("tab", { name: "Build" }).click();
  const rows = page.locator(".about-rows > div");
  await expect(rows).toHaveCount(2);

  const frontend = rows.nth(0);
  await expect(frontend.locator("dt")).toHaveText("Frontend");
  await expect(frontend.locator(".about-version")).toHaveText(/^\d+\.\d+\.\d+/);
  await expect(frontend.locator("code")).toHaveText(/^[0-9a-f]{7}$/);
  await expect(frontend.locator(".about-build-state")).toHaveText("local development");

  const backend = rows.nth(1);
  await expect(backend.locator("dt")).toHaveText("Backend");
  await expect(backend.locator(".about-version")).toHaveText("4.5.6");
  await expect(backend.locator("code")).toHaveText("abc1234");
  await expect(backend.locator(".about-build-state")).toHaveText("local development");

  await page.getByRole("tab", { name: "License" }).click();
  await expect(
    page.getByText("MIT License. Copyright © 2026 The Other Read contributors."),
  ).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});

test("changelog tab shows curated, reference-free notes for this version", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/about");
  await ready(page);

  await page.getByRole("tab", { name: "Changelog" }).click();
  const panel = page.getByRole("tabpanel", { name: "Changelog" });
  await expect(panel.getByRole("heading", { level: 2, name: /Version \d+\.\d+\.\d+/ })).toBeVisible();

  const text = await panel.innerText();
  expect(text).not.toMatch(/#[0-9]+/);
  expect(text).not.toMatch(/\b[0-9a-f]{7,40}\b/i);
  expect(text).not.toMatch(/https?:\/\//);

  const items = panel.getByRole("listitem");
  if ((await items.count()) === 0) {
    await expect(panel.getByText("No user-facing changes in this release.")).toBeVisible();
  } else {
    await expect(items.first()).toBeVisible();
  }
});
