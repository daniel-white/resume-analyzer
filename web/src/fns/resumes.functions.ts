import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader } from "@tanstack/react-start/server";
import { redirect } from "@tanstack/react-router";

import { fetchAccount } from "../server/auth";
import {
  fetchLibrary,
  fetchTeardown,
  postResume,
  type LibraryView,
  type Teardown,
  type UploadResult,
} from "../server/resumes";
import { SignedOut } from "../server/session";

export type { LibraryView, Teardown, UploadResult };

export type SignedInLibrary = LibraryView & { email: string };
export type SignedInTeardown = Exclude<Teardown, { kind: "missing" }> & { email: string };

function apiBase(): string {
  return process.env.API_URL ?? "http://127.0.0.1:8080";
}

function cookie(): string | undefined {
  return getRequestHeader("cookie");
}

function signedOut(): never {
  throw redirect({ to: "/login" });
}

export const getLibrary = createServerFn({ method: "GET" }).handler(
  async (): Promise<SignedInLibrary> => {
    try {
      const [library, account] = await Promise.all([
        fetchLibrary(apiBase(), cookie()),
        fetchAccount(apiBase(), cookie()),
      ]);
      if (!account) signedOut();
      return { ...library, email: account.email };
    } catch (error) {
      if (error instanceof SignedOut) signedOut();
      throw error;
    }
  },
);

export const getTeardown = createServerFn({ method: "GET" })
  .validator((id: string) => {
    if (!id) throw new Error("missing resume");
    return id;
  })
  .handler(async ({ data }): Promise<SignedInTeardown | { kind: "missing" }> => {
    try {
      const [view, account] = await Promise.all([
        fetchTeardown(apiBase(), data, cookie()),
        fetchAccount(apiBase(), cookie()),
      ]);
      if (!account) signedOut();
      if (view.kind === "missing") return view;
      return { ...view, email: account.email };
    } catch (error) {
      if (error instanceof SignedOut) signedOut();
      throw error;
    }
  });

export const uploadResume = createServerFn({ method: "POST" })
  .validator((data: FormData) => data)
  .handler(async ({ data }): Promise<UploadResult> => {
    const file = data.get("file");
    if (!(file instanceof File)) {
      return { ok: false, message: "Choose a PDF or DOCX." };
    }
    try {
      return await postResume(apiBase(), file, cookie());
    } catch (error) {
      if (error instanceof SignedOut) signedOut();
      throw error;
    }
  });
