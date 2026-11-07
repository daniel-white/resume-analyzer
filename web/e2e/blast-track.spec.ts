import { expect, test } from "@playwright/test";
import { designed, installApi, reading } from "./api";

for (const pending of [true, false]) {
  test(`BlastTrack logo and simulation disclosure when ${pending ? "pending" : "complete"}`, async ({ page }, testInfo) => {
    const resume = pending ? reading : {
      ...designed,
      versions: [{
        ...designed.versions[0],
        runs: [{
          id: "run-blast-track",
          plugin_id: "blast_track",
          status: "complete" as const,
          parse: designed.versions[0].parse,
          verdict: {
            kept: ["email", "table text"],
            dropped: ["column reading order", "date precision"],
            score: 55,
            summary: "Score 55 of 100. Put experience in one column to preserve reading order.",
            factors: [{ label: "column reading order", delta: -18, advice: "Put experience in one column to preserve reading order." }],
          },
        }],
      }],
    };
    await installApi(page, { resumes: [resume] });
    await page.goto(`/resumes/${resume.id}`);
    const card = page.getByRole("article", { name: "BlastTrack", exact: true });
    await expect(card.getByRole("heading", { name: "BlastTrack" })).toBeVisible();
    await expect(card.getByText("Keeps contact and table text, needs linear chronology. Simulated, modeled on publicly described parser behavior.", { exact: true })).toBeVisible();
    const logo = card.locator('img[src="/marks/blast_track.png"]');
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
      await expect(card.getByText("column reading order", { exact: true }).first()).toBeVisible();
      await expect(card.getByText("-18", { exact: true })).toBeVisible();
    }
    await card.screenshot({ path: testInfo.outputPath("blast-track-card.png") });
  });
}
