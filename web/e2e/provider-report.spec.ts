import { expect, test } from "@playwright/test";
import { designed, installApi } from "./api";

test("persisted provider reports render after reload without affecting older results", async ({ page }, testInfo) => {
  const resume = structuredClone(designed);
  const verdict = resume.versions[0].runs[0].verdict!;
  Object.assign(verdict, {
    report: {
      engine: "Plaintext Classic", engine_version: "3.8.2", reference: "GREENLINE-ABC123",
      outcome: "parsed_with_warnings", contact_fields: 2, sections_detected: 2,
      bullets_detected: 2, layout_confidence: "inferred",
      notes: ["PDF inspection is limited to embedded text; layout signals are inferred, not verified."],
    },
  });
  await installApi(page, { resumes: [resume] });
  await page.goto(`/resumes/${resume.id}?provider=greenline`);
  const card = page.getByRole("article", { name: "Greenline", exact: true });
  const report = card.getByRole("region", { name: "Simulated parser report" });
  await expect(report.getByText("Plaintext Classic")).toBeVisible();
  await expect(report.getByText("Parsed with warnings")).toBeVisible();
  await expect(report.getByText("Report GREENLINE-ABC123")).toBeVisible();
  await expect(report.getByText("PDF inference")).toBeVisible();
  await expect(report.getByText("2 / 3")).toBeVisible();
  await card.screenshot({ path: testInfo.outputPath("provider-report.png") });
  await page.reload();
  await expect(report.getByText("Report GREENLINE-ABC123")).toBeVisible();
  const legacy = page.getByRole("article", { name: "Levr", exact: true });
  await legacy.locator("summary").click();
  await expect(legacy.getByRole("region", { name: "Simulated parser report" })).toHaveCount(0);
  await expect(legacy.getByText("Score 70 of 100.")).toBeVisible();
});
