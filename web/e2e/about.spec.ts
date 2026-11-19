import { expect, test } from "@playwright/test";

import { installApi, ready } from "./api";

test("about page reports both builds, the license, and the dev indicator", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/about");
  await ready(page);

  await expect(page.getByRole("heading", { name: "About The Other Read" })).toBeVisible();
  await expect(page.getByText("LOCAL DEV BUILD")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);

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

  await expect(
    page.getByText("MIT License. Copyright © 2026 The Other Read contributors."),
  ).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});
