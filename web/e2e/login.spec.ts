import { expect, test } from "@playwright/test";

import { installApi, ready } from "./api";

test("sends a magic link and offers another email", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });

  await page.goto("/login");
  await ready(page);
  await page.getByLabel("Email").fill("ada@example.com");
  await page.getByRole("button", { name: "Email me a link" }).click();

  await expect(page.getByRole("heading", { name: "Check your email" })).toBeVisible();
  await expect(page.getByText("A sign-in link is on its way.")).toBeVisible();

  await page.getByRole("button", { name: "Use a different email" }).click();
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(page.getByLabel("Email")).toHaveValue("ada@example.com");
});

test("shows the email rejection from the API", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [], magicLinkStatus: 400 });

  await page.goto("/login");
  await ready(page);
  await page.getByLabel("Email").fill("ada@example.com");
  await page.getByRole("button", { name: "Email me a link" }).click();

  await expect(page.getByRole("alert")).toHaveText("That email does not look right.");
  await expect(page.getByRole("button", { name: "Email me a link" })).toBeEnabled();
});

test("follows a magic link into the library", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });

  await page.goto("/login/verify?token=fresh-token");

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByText("ada@example.com")).toBeVisible();
  await expect(page.getByText("No resumes yet.")).toBeVisible();
});

test("explains a link that did not sign in", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [], verifyStatus: 400 });

  await page.goto("/login/verify?token=expired-token");

  await expect(page.getByRole("heading", { name: "That link did not work" })).toBeVisible();
  await expect(page.getByText("That link has expired. Ask for another.")).toBeVisible();
  await page.getByRole("link", { name: "Ask for another link" }).click();
  await expect(page).toHaveURL(/\/login$/);
});

test("rejects a verify visit with no token", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });

  await page.goto("/login/verify");

  await expect(page.getByRole("heading", { name: "That link did not work" })).toBeVisible();
  await expect(page.getByText("That link did not sign you in.")).toBeVisible();
});

test("preserves an analysis email destination through sign-in", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  const id = "12345678-1234-1234-1234-123456789abc";
  const destination = `/resumes/${id}?version=${id}&provider=greenline`;
  await page.goto(destination);
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  expect(new URL(page.url()).searchParams.get("returnTo")).toBe(destination);
  await page.goto(`/login/verify?token=fresh-token&returnTo=${encodeURIComponent(destination)}`);
  await expect(page).toHaveURL(new RegExp(`/resumes/${id}\\?version=${id}&provider=greenline$`));
});

test("does not follow an external post-login destination", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/login/verify?token=fresh-token&returnTo=https%3A%2F%2Fevil.example");
  await expect(page).toHaveURL(/\/$/);
});
