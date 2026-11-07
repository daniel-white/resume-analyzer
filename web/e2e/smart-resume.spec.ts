import { expect, test } from "@playwright/test";

import { designed, installApi, reading, stored } from "./api";

const plugin = {
  id: "smart_resume",
  name: "SmartResume",
  subtitle: "Keeps table text and headers, loses text boxes. Simulated, modeled on publicly described parser behavior.",
};

for (const pending of [true, false]) {
  test(`SmartResume logo and disclosure remain visible when ${pending ? "pending" : "complete"}`, async ({ page }, testInfo) => {
    const resume = pending ? reading : {
      ...designed,
      versions: [{
        ...designed.versions[0],
        runs: [{
          id: "run-smart-resume",
          plugin_id: plugin.id,
          status: "complete" as const,
          parse: designed.versions[0].parse,
          verdict: {
            kept: ["email", "table text", "headings"],
            dropped: ["text box"],
            score: 65,
            summary: "Score 65 of 100. Take the text out of the text box.",
            factors: [{ label: "text box", delta: -20, advice: "Take the text out of the text box." }],
          },
        }],
      }],
    };
    await installApi(page, { resumes: [resume] });
    await page.goto(`/resumes/${resume.id}`);
    await expect(page.getByLabel("Original resume").getByText("Original PDF resume")).toBeVisible({ timeout: 20_000 });
    const card = page.getByRole("article").filter({ has: page.getByRole("heading", { name: plugin.name }) });
    await expect(card.getByRole("heading", { name: plugin.name })).toBeVisible();
    await expect(card.getByText(plugin.subtitle, { exact: true })).toBeVisible();
    const logo = card.locator('img[src="/marks/smart_resume.png"]');
    await expect(logo).toBeVisible();
    await expect(logo).toHaveAttribute("alt", "");
    await expect.poll(() => logo.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(true);
    const box = await logo.boundingBox();
    expect(box?.width).toBeGreaterThan(0);
    expect(box?.width).toBeLessThan(160);
    if (pending) {
      await expect(card.getByLabel("Pending")).toBeVisible();
    } else {
      await card.locator("summary").click();
      await expect(card.getByText("text box", { exact: true }).first()).toBeVisible();
      await expect(card.getByText("-20", { exact: true })).toBeVisible();
    }
    await card.screenshot({ path: testInfo.outputPath("smart-resume-card.png") });
  });
}

test("SmartResume and the original DOCX viewer render together", async ({ page }) => {
  await installApi(page, { resumes: [stored] });
  await page.goto(`/resumes/${stored.id}`);
  const preview = page.getByLabel("Original resume");
  await expect(preview.getByText("Original DOCX resume")).toBeVisible();
  await expect(preview.getByRole("link", { name: "Download original" })).toBeVisible();
  const card = page.getByRole("article").filter({ has: page.getByRole("heading", { name: plugin.name }) });
  await expect(card.getByRole("heading", { name: plugin.name })).toBeVisible();
  await expect(card.getByText(plugin.subtitle, { exact: true })).toBeVisible();
  await expect(card.getByLabel("Pending")).toBeVisible();
});
