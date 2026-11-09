import { expect, test } from "@playwright/test";
import { installApi, ready } from "./api";

test("public landing uploads before login and keeps the file through verification", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/");
  await ready(page);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("You see a résumé.They see data.");
  await expect(page).toHaveURL(/\/$/);
  await page.getByLabel("Upload résumé").setInputFiles({ name: "next-chapter.pdf", mimeType: "application/pdf", buffer: Buffer.from("%PDF-1.4\n%%EOF") });
  await expect(page.getByText("Uploaded. No need to choose it again.")).toBeVisible();
  await page.getByLabel("Your email").fill("ada@example.com");
  await page.getByRole("button", { name: "Email me a link" }).click();
  await expect(page.getByRole("heading", { name: "Your next chapter is in your inbox." })).toBeVisible();
  await page.goto("/login/verify?token=fresh-token");
  await expect(page.getByRole("heading", { name: "Your résumés" })).toBeVisible();
  await expect(page.getByText("next-chapter.pdf")).toBeVisible();
});

test("rejects unsupported and oversized files without leaving the page", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/");
  await ready(page);
  await page.getByLabel("Upload résumé").setInputFiles({ name: "resume.txt", mimeType: "text/plain", buffer: Buffer.from("hello") });
  await expect(page.getByRole("alert")).toHaveText("Choose a PDF or DOCX résumé.");
  await page.getByLabel("Upload résumé").setInputFiles({ name: "large.pdf", mimeType: "application/pdf", buffer: Buffer.alloc(5 * 1024 * 1024 + 1) });
  await expect(page.getByRole("alert")).toHaveText("That file is over five megabytes.");
  await expect(page.getByRole("button", { name: "Choose my résumé" })).toBeEnabled();
});

test("mobile landing fits the viewport and offers an accessible upload action", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/");
  await ready(page);
  await page.getByRole("link", { name: "Give your résumé another read" }).click();
  await expect(page.getByRole("button", { name: "Choose my résumé" })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
});

test("accepts drag-and-drop and allows replacing the staged resume", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [] });
  await page.goto("/");
  await ready(page);
  const data = await page.evaluateHandle(() => {
    const transfer = new DataTransfer();
    transfer.items.add(new File(["%PDF-1.4\n%%EOF"], "dropped.pdf", { type: "application/pdf" }));
    return transfer;
  });
  await page.locator(".landing-upload").dispatchEvent("drop", { dataTransfer: data });
  await expect(page.getByText("dropped.pdf", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Choose a different résumé" }).click();
  await expect(page.getByRole("button", { name: "Choose my résumé" })).toBeEnabled();
});
