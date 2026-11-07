import { expect, test } from "@playwright/test";

import { designed, installApi, reading } from "./api";

function completeReading() {
  return {
    ...reading,
    versions: [{ ...designed.versions[0], id: reading.versions[0].id, filename: "workshop.pdf" }],
  };
}

test("streams analysis results without polling and closes on completion", async ({ page }) => {
  const api = await installApi(page, { resumes: [reading] });
  await page.goto("/resumes/resume-reading");
  await expect.poll(async () => (await api.stats()).connections).toBe(1);
  await expect(page.getByLabel("Original resume").getByText("Original PDF resume")).toBeVisible();
  const before = (await api.stats()).detailReads;
  // Longer than the old two-second interval: a quiet stream must not poll.
  await page.waitForTimeout(2500);
  expect((await api.stats()).detailReads).toBe(before);
  await api.update({ resumes: [completeReading()], event: "AtsCallbackReceived" });
  await expect(page.getByLabel("Grade F", { exact: true })).toBeVisible();
  const greenline = page.getByRole("article").filter({ has: page.getByRole("heading", { name: "Greenline" }) });
  await expect(greenline.getByLabel("Pending")).toHaveCount(0);
  await expect.poll(async () => (await api.stats()).streams).toBe(0);
});

test("reloads authoritative state after reconnect even when events were missed", async ({ page }) => {
  const api = await installApi(page, { resumes: [reading] });
  await page.goto("/resumes/resume-reading");
  await expect.poll(async () => (await api.stats()).connections).toBe(1);
  // Only change state as the next stream connects, after the error handler's
  // refresh. No event is sent: the reconnect's open handler must reconcile it.
  await api.update({ onConnectResumes: [completeReading()], disconnect: true });
  await expect.poll(async () => (await api.stats()).connections).toBe(2);
  await expect(page.getByLabel("Grade F", { exact: true })).toBeVisible();
  await expect.poll(async () => (await api.stats()).streams).toBe(0);
});

test("closes the stream when navigating away and skips completed analyses", async ({ page }) => {
  const api = await installApi(page, { resumes: [reading, designed] });
  await page.goto("/resumes/resume-reading");
  await expect.poll(async () => (await api.stats()).streams).toBe(1);
  await page.goto("/resumes/resume-designed");
  await expect(page.getByLabel("Original resume")).toBeVisible();
  await expect.poll(async () => (await api.stats()).streams).toBe(0);
  expect((await api.stats()).connections).toBe(1);
});

test("updates reading rows in the library through SSE", async ({ page }) => {
  const api = await installApi(page, { resumes: [reading] });
  await page.goto("/");
  await expect.poll(async () => (await api.stats()).streams).toBe(1);
  await api.update({ resumes: [completeReading()], event: "AtsCallbackReceived" });
  await expect(page.getByRole("link", { name: /workshop.pdf/ })).toContainText("2 reads");
  await expect.poll(async () => (await api.stats()).streams).toBe(0);
});

test("protects the event stream on the frontend origin", async ({ page }) => {
  await installApi(page, { signedIn: false, resumes: [reading] });
  const denied = await page.request.get("/api/resumes/resume-reading/events");
  expect(denied.status()).toBe(401);
  await installApi(page, { resumes: [reading] });
  const missing = await page.request.get("/api/resumes/missing/events");
  expect(missing.status()).toBe(404);
});
