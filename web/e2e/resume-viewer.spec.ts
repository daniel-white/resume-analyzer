import { expect, test } from "@playwright/test";

import { designed, installApi } from "./api";

test("PDF toolbar remains clickable after scrolling the document", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);

  const viewer = page.getByLabel("Original resume");
  await expect(viewer.getByText("Original PDF resume")).toBeVisible();
  const zoomIn = viewer.getByRole("button", { name: "Zoom in", exact: true });
  await zoomIn.click();
  await zoomIn.click();
  await expect(viewer.locator(".rdv-toolbar-zoom-label")).toHaveText("120%");

  const content = viewer.locator(".rdv-pdf-main-content");
  await content.evaluate((element) => { element.scrollTop = 150; });
  await expect.poll(() => content.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  await expect.poll(() => viewer.evaluate((element) => {
    const toolbar = element.querySelector(".rdv-pdf-controls")!.getBoundingClientRect();
    const text = element.querySelector(".textLayer")!.getBoundingClientRect();
    return text.top < toolbar.top && text.bottom > toolbar.bottom;
  })).toBe(true);

  const zoom = viewer.locator(".rdv-toolbar-zoom-label");
  const before = await zoom.textContent();
  expect(await zoomIn.evaluate((element) => {
    const rect = element.getBoundingClientRect();
    return element.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
  })).toBe(true);
  await zoomIn.click({ timeout: 3000 });
  await expect(zoom).not.toHaveText(before!);
  await viewer.getByRole("button", { name: "Search", exact: true }).click({ timeout: 3000 });
  await expect(viewer.locator(".rdv-search-bar input")).toBeVisible();
});
