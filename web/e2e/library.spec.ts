import { expect, test } from "@playwright/test";

import { designed, installApi, reading, ready, stored } from "./api";

test("asks a signed-out visitor to sign in", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });

  await page.goto("/");

  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(page.getByText(/email you a link\. No password\./)).toBeVisible();
});

test("lists a scored resume and a file still being read", async ({ page }) => {
  await installApi(page, { resumes: [designed, reading] });

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Drop a PDF or DOCX" })).toBeVisible();
  await expect(page.getByText("ada@example.com")).toBeVisible();
  await expect(page.getByRole("link", { name: /ada-designed\.pdf/ })).toBeVisible();
  await expect(page.getByText("2 reads")).toBeVisible();
  await expect(page.getByText("F–C-")).toBeVisible();
  await expect(page.getByRole("link", { name: /workshop\.pdf/ })).toContainText("Reading");
});

test("shows an empty library until a file is stored", async ({ page }) => {
  await installApi(page, { resumes: [] });

  await page.goto("/");

  await expect(page.getByText("Nothing stored yet.")).toBeVisible();
  await expect(page.getByText("Each system reads the file itself. Five megabytes.")).toBeVisible();
});

test("opens a scored teardown from the library", async ({ page }) => {
  await installApi(page, { resumes: [designed, reading] });

  await page.goto("/");
  await ready(page);
  await page.getByRole("link", { name: /ada-designed\.pdf/ }).click();

  await expect(page).toHaveURL(/\/resumes\/resume-designed$/);
  await expect(page.getByRole("heading", { name: "ada-designed.pdf" })).toBeVisible();
  await expect(page.getByText(/Contact in the header/)).toBeVisible();
  await expect(page.getByLabel("What we read")).toContainText("Ada Lovelace");
  const greenline = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Greenline" }) });
  await expect(greenline.getByRole("heading", { name: "Greenline" })).toBeVisible();
  await expect(greenline.getByLabel("Grade F", { exact: true })).toBeVisible();
  await greenline.locator("summary").click();
  await expect(greenline.getByText("header contact")).toBeVisible();
  await expect(greenline.getByText("contact in header", { exact: true })).toBeVisible();
  await expect(greenline.getByText("-25", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Levr" })).toBeVisible();
});

test("keeps a reading file on the teardown without scores", async ({ page }) => {
  await installApi(page, { resumes: [reading] });

  await page.goto("/resumes/resume-reading");

  await expect(page.getByRole("heading", { name: "workshop.pdf" })).toBeVisible();
  await expect(page.getByText("Two systems are reading this file.")).toBeVisible();
  await expect(page.getByText("Reading the file").first()).toBeVisible();
  await expect(page.getByLabel("Pending").first()).toBeVisible();
});

test("says when a resume is missing", async ({ page }) => {
  await installApi(page, { resumes: [designed] });

  await page.goto("/resumes/missing");

  await expect(page.getByRole("heading", { name: "That resume is not here." })).toBeVisible();
  await page.getByRole("link", { name: "Your resumes" }).click();
  await expect(page).toHaveURL(/\/$/);
});

test("uploads a file and lands on its teardown", async ({ page }) => {
  await installApi(page, { resumes: [stored] });

  await page.goto("/");
  await ready(page);
  await expect(page.getByRole("heading", { name: "Drop another file" })).toBeVisible();
  await expect(page.getByRole("link", { name: /just-added\.docx/ })).toContainText(
    "Stored. Waiting for the first read.",
  );
  await page.locator('input[type="file"]').setInputFiles({
    name: "dropped.pdf",
    mimeType: "application/pdf",
    buffer: Buffer.from("%PDF-1.4 playwright"),
  });

  await expect(page).toHaveURL(/\/resumes\/resume-uploaded$/);
  await expect(page.getByRole("heading", { name: "dropped.pdf" })).toBeVisible();
  await expect(page.getByText("Two systems are reading this file.")).toBeVisible();
});

test("signs out and returns to the login page", async ({ page }) => {
  await installApi(page, { resumes: [designed] });

  await page.goto("/");
  await ready(page);
  await page.getByRole("button", { name: "Sign out" }).click();

  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});
