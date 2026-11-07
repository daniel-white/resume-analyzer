import { expect, test } from "@playwright/test";
import { designed, installApi, reading } from "./api";

const plugin = {
  id: "indy_hackers_hiring",
  name: "IndyHackersHiring",
  subtitle: "Tolerates tables and text boxes, loses columns and images. Simulated, modeled on publicly described parser behavior.",
};

for (const pending of [true, false]) {
  test(`IndyHackersHiring logo and disclosure remain visible when ${pending ? "pending" : "complete"}`, async ({ page }, testInfo) => {
    const resume = pending ? reading : {
      ...designed,
      versions: [{
        ...designed.versions[0],
        runs: [{
          id: "run-indy",
          plugin_id: plugin.id,
          status: "complete" as const,
          parse: designed.versions[0].parse,
          verdict: {
            kept: ["email", "headings"], dropped: ["columns", "image"], score: 65,
            summary: "Score 65 of 100. Keep the page to one column.",
            factors: [{ label: "two columns", delta: -18, advice: "Keep the page to one column." }],
          },
        }],
      }],
    };
    await installApi(page, { resumes: [resume] });
    await page.goto(`/resumes/${resume.id}`);
    const card = page.getByRole("article", { name: plugin.name, exact: true });
    await expect(card.getByRole("heading", { name: plugin.name })).toBeVisible();
    await expect(card.getByText(plugin.subtitle, { exact: true })).toBeVisible();
    const logo = card.locator('img[src="/marks/indy_hackers_hiring.png"]');
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
      await expect(card.getByText("two columns", { exact: true })).toBeVisible();
      await expect(card.getByText("-18", { exact: true })).toBeVisible();
    }
    await card.screenshot({ path: testInfo.outputPath("indy-hackers-hiring-card.png") });
  });
}
