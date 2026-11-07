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

test("shows an empty library until a file is stored", async ({ page }, testInfo) => {
  await installApi(page, { resumes: [] });

  await page.goto("/");

  await expect(page.getByText("Nothing stored yet.")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
  await expect(page.getByRole("heading", { name: "A little clarity before you apply." })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("empty-library-desktop.png"), fullPage: true, caret: "initial" });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("empty-library-mobile.png"), fullPage: true, caret: "initial" });
  await expect(page.getByText("Each system reads the file itself. Five megabytes.")).toBeVisible();
});

test("accepts a dropped resume from the empty library", async ({ page }) => {
  await installApi(page, { resumes: [] });
  await page.goto("/");
  await ready(page);
  const files = await page.evaluateHandle(() => {
    const transfer = new DataTransfer();
    transfer.items.add(new File(["%PDF-1.4 dropped"], "dropped.pdf", { type: "application/pdf" }));
    return transfer;
  });
  await page.locator(".drop").dispatchEvent("drop", { dataTransfer: files });
  await expect(page.getByRole("heading", { name: "dropped.pdf" })).toBeVisible();
  await files.dispose();
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
  await expect(page.getByLabel("Original resume").getByText("Original PDF resume")).toBeVisible({ timeout: 20_000 });
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
  await expect(page.getByText("Three systems are reading this file.")).toBeVisible();
  await expect(page.getByText("Reading the file").first()).toBeVisible();
  await expect(page.getByLabel("Pending").first()).toBeVisible();
  await expect(page.getByLabel("Original resume").getByText("Original PDF resume")).toBeVisible({ timeout: 20_000 });
});

test("renders the original DOCX even before parsing finishes", async ({ page }) => {
  await installApi(page, { resumes: [stored] });
  await page.goto("/resumes/resume-stored");
  const preview = page.getByLabel("Original resume");
  await expect(preview.getByText("Original DOCX resume")).toBeVisible({ timeout: 20_000 });
  await expect(preview.locator("iframe")).toHaveCount(0);
  const download = page.waitForEvent("download");
  await preview.getByRole("link", { name: "Download original" }).click();
  expect((await download).suggestedFilename()).toBe("just-added.docx");
});

test("shows a useful error when the protected file request fails", async ({ page }) => {
  await installApi(page, { resumes: [stored] });
  await page.route("**/api/resumes/*/versions/*/file", (route) => route.fulfill({ status: 401 }));
  await page.goto("/resumes/resume-stored");
  await expect(page.getByLabel("Original resume").getByRole("alert")).toContainText("Your session has expired");
});

test("protects file responses on the frontend origin", async ({ page }) => {
  await installApi(page, { resumes: [stored] });
  const path = "/api/resumes/resume-stored/versions/resume-stored-v1/file";
  const original = await page.request.get(path);
  expect(original.status()).toBe(200);
  expect(original.headers()["content-type"]).toBe(
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  );
  expect(original.headers()["cache-control"]).toBe("private, no-store");
  expect((await original.body()).subarray(0, 2).toString()).toBe("PK");

  const missing = await page.request.get("/api/resumes/missing/versions/resume-stored-v1/file");
  expect(missing.status()).toBe(404);
  expect(missing.headers()["cache-control"]).toBe("private, no-store");

  await installApi(page, { signedIn: false, resumes: [stored] });
  const signedOut = await page.request.get(path);
  expect(signedOut.status()).toBe(401);
  expect(signedOut.headers()["cache-control"]).toBe("private, no-store");
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
  await expect(page.getByText("Three systems are reading this file.")).toBeVisible();
});

test("signs out and returns to the login page", async ({ page }) => {
  await installApi(page, { resumes: [designed] });

  await page.goto("/");
  await ready(page);
  await page.getByRole("button", { name: "Account menu" }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();

  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});
