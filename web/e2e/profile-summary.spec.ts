import { expect, test } from "@playwright/test";

import { designed, installApi, reading, ready } from "./api";

test("shows the model impression and saves summary feedback", async ({ page }) => {
  await installApi(page, { resumes: [designed] });
  await page.goto("/resumes/resume-designed");
  await ready(page);

  await expect(page.getByRole("heading", { name: "Who the model thinks you are" })).toBeVisible();
  await expect(page.getByText("Ada Lovelace is a computing pioneer", { exact: false })).toBeVisible();

  const yes = page.getByRole("button", { name: "Yes, this summary is accurate" });
  const no = page.getByRole("button", { name: "No, this summary is not accurate" });
  await yes.click();
  await expect(yes).toHaveAttribute("aria-pressed", "true");
  await expect(no).toHaveAttribute("aria-pressed", "false");

  await page.reload();
  await ready(page);
  await expect(yes).toHaveAttribute("aria-pressed", "true");

  await no.click();
  await expect(no).toHaveAttribute("aria-pressed", "true");
  await expect(yes).toHaveAttribute("aria-pressed", "false");
  await no.click();
  await expect(no).toHaveAttribute("aria-pressed", "false");
});

test("replaces the pending state when parsing finishes", async ({ page }) => {
  const api = await installApi(page, { resumes: [reading] });
  await page.goto("/resumes/resume-reading");
  await ready(page);
  await expect(page.getByText("A candidate profile will appear here", { exact: false })).toBeVisible();
  await expect.poll(async () => (await api.stats()).connections).toBe(1);

  const completed = {
    ...reading,
    versions: [{ ...designed.versions[0], id: reading.versions[0].id, filename: "workshop.pdf" }],
  };
  await api.update({ resumes: [completed], event: "ResumeParsed" });
  await expect(page.getByText("Ada Lovelace is a computing pioneer", { exact: false })).toBeVisible();
  await expect(page.getByText("A candidate profile will appear here", { exact: false })).toHaveCount(0);
});
