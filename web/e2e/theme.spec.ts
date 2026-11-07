import { expect, test } from "@playwright/test";

import { designed, installApi, ready } from "./api";

test("carries the landing palette into sign-in and link recovery", async ({ page }, testInfo) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/login");
  await ready(page);
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(246, 245, 239)");
  await expect(page.getByRole("button", { name: "Email me a link" })).toHaveCSS("background-color", "rgb(209, 242, 103)");
  await expect(page.getByRole("heading", { name: "Sign in", exact: true })).toHaveCSS("font-family", /Georgia/);
  await expect(page.getByRole("link", { name: /The Other Read/ })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("sign-in-desktop.png"), fullPage: true, caret: "initial" });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("sign-in-mobile.png"), fullPage: true, caret: "initial" });
  await page.goto("/login/verify");
  await expect(page.getByRole("heading", { name: "That link did not work" })).toBeVisible();
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(246, 245, 239)");
});

test("keeps library, analysis, and destructive controls on the same theme", async ({ page }, testInfo) => {
  await installApi(page, { resumes: [designed] });
  await page.goto("/");
  await ready(page);
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(246, 245, 239)");
  await expect(page.locator(".upload-callout")).toHaveCSS("background-color", "rgb(209, 242, 103)");
  await page.screenshot({ path: testInfo.outputPath("library-desktop.png"), fullPage: true, caret: "initial" });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("library-mobile.png"), fullPage: true, caret: "initial" });
  await page.goto("/resumes/resume-designed");
  await ready(page);
  await expect(page.getByRole("heading", { name: "ada-designed.pdf" })).toBeVisible();
  await expect(page.locator(".version-upload")).toHaveCSS("background-color", "rgb(209, 242, 103)");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("analysis-mobile.png"), fullPage: true, caret: "initial" });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.screenshot({ path: testInfo.outputPath("analysis-desktop.png"), fullPage: true, caret: "initial" });
  await page.getByRole("button", { name: "Account menu" }).click();
  await expect(page.getByRole("menu")).toHaveCSS("color", "rgb(32, 37, 30)");
});
