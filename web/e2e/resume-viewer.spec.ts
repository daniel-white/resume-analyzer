import { expect, test } from "@playwright/test";

import { designed, installApi, stored } from "./api";

for (const [format, resume] of [["PDF", designed], ["DOCX", stored]] as const) {
  test(`${format} preview only offers the heading download link`, async ({ page }) => {
    await installApi(page, { resumes: [resume] });
    await page.goto(`/resumes/${resume.id}`);

    const viewer = page.getByLabel("Original resume");
    await expect(viewer.getByText(`Original ${format} resume`)).toBeVisible({ timeout: 20_000 });
    await expect(viewer.getByRole("button", { name: "Zoom in", exact: true })).toBeVisible();
    await expect(viewer.locator("#react-doc-viewer")).toHaveCSS("user-select", "text");
    await expect(viewer.getByRole("button", { name: "Zoom in", exact: true })).toHaveCSS("user-select", "none");
    await expect(viewer.getByRole("button", { name: /download/i })).toHaveCount(0);
    // Exercise asynchronously inserted links in both renderer trees.
    await viewer.locator("#react-doc-viewer").evaluate((element) => {
      const link = document.createElement("a");
      link.href = "https://example.com/portfolio";
      link.textContent = "Portfolio";
      element.appendChild(link);
    });
    const portfolio = viewer.getByRole("link", { name: "Portfolio" });
    await expect(portfolio).toHaveAttribute("target", "_blank");
    await expect(portfolio).toHaveAttribute("rel", "noopener noreferrer");
    if (format === "PDF") {
      const documentLink = viewer.locator('.annotationLayer a[href="https://example.com/resume"]');
      await expect(documentLink).toHaveAttribute("target", "_blank");
      await expect(documentLink).toHaveAttribute("rel", "noopener noreferrer");
    }
    await expect(viewer.getByRole("link", { name: "Download original" })).toBeVisible();
    const download = page.waitForEvent("download");
    await viewer.getByRole("link", { name: "Download original" }).click();
    expect((await download).suggestedFilename()).toBe(resume.versions[0].filename);
  });
}

test("PDF toolbar remains clickable after scrolling the document", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto(`/resumes/${designed.id}`);

  const viewer = page.getByLabel("Original resume");
  await expect(viewer.getByText("Original PDF resume")).toBeVisible({ timeout: 20_000 });
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
