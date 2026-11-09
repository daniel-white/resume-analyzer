import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader } from "@tanstack/react-start/server";
import { redirect } from "@tanstack/react-router";

import { fetchAccount } from "../server/auth";
import {
  fetchLibrary,
  fetchTeardown,
  postResume,
  stageResume,
  deleteResume,
  renameResume,
  voteSection,
  type RenameResult,
  type DeleteResult,
  type VoteResult,
  type LibraryView,
  type Teardown,
  type UploadResult,
} from "../server/resumes";
import { SignedOut } from "../server/session";
import { resumeReturnTo } from "../lib/return-to";

export type { LibraryView, Teardown, UploadResult };

export type SignedInLibrary = LibraryView & { email: string };
export type SignedInTeardown = Exclude<Teardown, { kind: "missing" }> & { email: string };

function apiBase(): string {
  return process.env.API_URL ?? "http://127.0.0.1:8080";
}

function cookie(): string | undefined {
  return getRequestHeader("cookie");
}

function signedOut(returnTo?: string): never {
  throw redirect({ to: "/login", search: { returnTo: resumeReturnTo(returnTo) } });
}

export const getLibrary = createServerFn({ method: "GET" }).handler(
  async (): Promise<SignedInLibrary | null> => {
    try {
      const account = await fetchAccount(apiBase(), cookie());
      if (!account) return null;
      const library = await fetchLibrary(apiBase(), cookie());
      return { ...library, email: account.email };
    } catch (error) {
      if (error instanceof SignedOut) return null;
      throw error;
    }
  },
);

export const stageUpload = createServerFn({ method: "POST" })
  .validator((data: FormData) => data)
  .handler(async ({ data }) => {
    const file = data.get("file");
    if (!(file instanceof File)) return { ok: false as const, message: "Choose a PDF or DOCX." };
    return stageResume(apiBase(), file, cookie());
  });

export const getTeardown = createServerFn({ method: "GET" })
  .validator((data: { id: string; versionId?: string; provider?: string }) => {
    if (!data.id) throw new Error("missing resume");
    return data;
  })
  .handler(async ({ data }): Promise<SignedInTeardown | { kind: "missing" }> => {
    const search = new URLSearchParams();
    if (data.versionId) search.set("version", data.versionId);
    if (data.provider) search.set("provider", data.provider);
    const returnTo = `/resumes/${data.id}${search.size ? `?${search}` : ""}`;
    try {
      const [view, account] = await Promise.all([
        fetchTeardown(apiBase(), data.id, cookie(), data.versionId),
        fetchAccount(apiBase(), cookie()),
      ]);
      if (!account) signedOut(returnTo);
      if (view.kind === "missing") return view;
      return { ...view, email: account.email };
    } catch (error) {
      if (error instanceof SignedOut) signedOut(returnTo);
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

export const updateResumeName = createServerFn({ method: "POST" })
  .validator((data: { id: string; title: string }) => {
    if (!data.id || typeof data.title !== "string") throw new Error("missing resume name");
    return data;
  })
  .handler(async ({ data }): Promise<RenameResult> => {
    try {
      return await renameResume(apiBase(), data.id, data.title, cookie());
    } catch (error) {
      if (error instanceof SignedOut) signedOut(`/resumes/${data.id}`);
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

export const voteResumeSection = createServerFn({ method: "POST" })
  .validator((data: {
    resumeId: string;
    versionId: string;
    sectionId: string;
    vote: "up" | "down" | null;
  }) => {
    if (!data.resumeId || !data.versionId || !data.sectionId) {
      throw new Error("missing section");
    }
    return data;
  })
  .handler(async ({ data }): Promise<VoteResult> => {
    try {
      return await voteSection(
        apiBase(),
        data.resumeId,
        data.versionId,
        data.sectionId,
        data.vote,
        cookie(),
      );
    } catch (error) {
      if (error instanceof SignedOut) signedOut(`/resumes/${data.resumeId}`);
      throw error;
    }
  });
