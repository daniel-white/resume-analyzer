import { expect, test } from "@playwright/test";

import { installApi, ready } from "./api";

test("about page reports both builds, the license, and the dev indicator", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/about");
  await ready(page);

  await expect(page.getByRole("heading", { name: "About The Other Read" })).toBeVisible();
  await expect(page.getByText("LOCAL DEV BUILD")).toBeVisible();

  const rows = page.locator(".about-rows > div");
  await expect(rows.nth(0).locator("dd")).toHaveText(/^\d+\.\d+\.\d+/);
  await expect(rows.nth(1).locator("dd")).toHaveText("4.5.6");
  await expect(rows.nth(2).locator("dd")).toHaveText(/^[0-9a-f]{7}$/);
  await expect(rows.nth(3).locator("dd")).toHaveText("abc1234");
  await expect(rows.nth(4).locator("dd")).toHaveText("local development");

  await expect(
    page.getByText("MIT License. Copyright © 2026 The Other Read contributors."),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: "OK", exact: true })).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Close and return to your résumés" }),
  ).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});
