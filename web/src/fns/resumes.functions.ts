import { createServerFn } from "@tanstack/react-start";

import {
  fetchLibrary,
  fetchTeardown,
  postResume,
  type LibraryView,
  type Teardown,
  type UploadResult,
} from "../server/resumes";

export type { LibraryView, Teardown, UploadResult };

function apiBase(): string {
  return process.env.API_URL ?? "http://127.0.0.1:8080";
}

export const getLibrary = createServerFn({ method: "GET" }).handler(
  async (): Promise<LibraryView> => fetchLibrary(apiBase()),
);

export const getTeardown = createServerFn({ method: "GET" })
  .validator((id: string) => {
    if (!id) throw new Error("missing resume");
    return id;
  })
  .handler(async ({ data }): Promise<Teardown> => fetchTeardown(apiBase(), data));

export const uploadResume = createServerFn({ method: "POST" })
  .validator((data: FormData) => data)
  .handler(async ({ data }): Promise<UploadResult> => {
    const file = data.get("file");
    if (!(file instanceof File)) {
      return { ok: false, message: "Choose a PDF or DOCX." };
    }
    return postResume(apiBase(), file);
  });
