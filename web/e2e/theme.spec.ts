import { expect, test } from "@playwright/test";

import { designed, installApi, ready } from "./api";

test("prevents UI selection but preserves textbox editing", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/login");
  await ready(page);
  await expect(page.locator("body")).toHaveCSS("user-select", "none");
  // WebKit computes descendants as auto but still uses the parent's none value.
  const heading = await page.locator(".sign h1").boundingBox();
  expect(heading).not.toBeNull();
  await page.mouse.move(heading!.x, heading!.y + heading!.height / 2);
  await page.mouse.down();
  await page.mouse.move(heading!.x + heading!.width, heading!.y + heading!.height / 2, { steps: 10 });
  await page.mouse.up();
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("");
  const email = page.locator(".sign input");
  await expect(email).toHaveCSS("user-select", "text");
  await email.fill("ada@example.com");
  await email.selectText();
  await page.keyboard.type("grace@example.com");
  await expect(email).toHaveValue("grace@example.com");
});

test("uses consistent editorial headings, readable sans copy, and technical mono", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/");
  await ready(page);
  await expect(page.locator(".hero-copy h1")).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".how-heading h2")).toHaveCSS("font-weight", "400");
  await expect(page.locator(".landing-steps h3").first()).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".hero-description")).toHaveCSS("font-family", /Arial/);
  await expect(page.locator(".landing-formats")).toHaveCSS("font-family", /Arial/);
  await expect(page.locator(".landing-no-account")).toHaveCSS("font-size", "12px");
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator(".landing-no-account")).toHaveCSS("font-size", "12px");

  await page.goto("/login");
  await expect(page.locator(".sign h1")).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".sign input")).toHaveCSS("font-family", /Arial/);
  await expect(page.locator(".sign input")).toHaveCSS("font-size", "16px");

  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);
  await ready(page);
  await expect(page.locator(".filename")).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".results-heading h2")).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".extract h2")).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".extract h2")).toHaveCSS("font-size", "28px");
  await expect(page.locator(".overview-stats > div:nth-child(2) strong")).toHaveCSS("font-family", /Arial/);
  await expect(page.locator(".overall-rating-value strong")).toHaveCSS("font-family", /Georgia/);
  await expect(page.locator(".stat-label").first()).toHaveCSS("font-family", /Arial/);
  await expect(page.locator(".viewer-heading h2")).toHaveCSS("font-family", /Arial/);
  await page.locator(".fold-summary").first().click();
  const factors = page.locator(".fold-body ul.border-t").first();
  await expect(factors).toHaveCSS("font-family", /Arial/);
  await expect(factors.locator("li > span:last-child").first()).toHaveCSS("font-family", /monospace/);
});

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
