import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader } from "@tanstack/react-start/server";
import { redirect } from "@tanstack/react-router";

import { fetchAccount } from "../server/auth";
import {
  fetchLibrary,
  fetchTeardown,
  postResume,
  deleteResume,
  type DeleteResult,
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
  .validator((data: { id: string; versionId?: string }) => {
    if (!data.id) throw new Error("missing resume");
    return data;
  })
  .handler(async ({ data }): Promise<SignedInTeardown | { kind: "missing" }> => {
    try {
      const [view, account] = await Promise.all([
        fetchTeardown(apiBase(), data.id, cookie(), data.versionId),
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
      const resumeId = data.get("resumeId");
      return await postResume(apiBase(), file, cookie(), typeof resumeId === "string" ? resumeId : undefined);
    } catch (error) {
      if (error instanceof SignedOut) signedOut();
      throw error;
    }
  });

export const removeResume = createServerFn({ method: "POST" })
  .validator((data: { id: string; versionId?: string }) => {
    if (!data.id) throw new Error("missing resume");
    return data;
  })
  .handler(async ({ data }): Promise<DeleteResult> => {
    try {
      return await deleteResume(apiBase(), data.id, cookie(), data.versionId);
    } catch (error) {
      if (error instanceof SignedOut) signedOut();
      throw error;
    }
  });
