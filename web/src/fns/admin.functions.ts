import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader } from "@tanstack/react-start/server";
import { redirect } from "@tanstack/react-router";

import { fetchAccount } from "../server/auth";
import {
  fetchAdminFeedback,
  setAnalysisHandled,
  NotAdmin,
  type AdminFeedback,
  type HandledResult,
} from "../server/admin";
import { SignedOut } from "../server/session";

export type { AdminFeedback, HandledResult };

export type AdminView = {
  email: string;
  feedback: AdminFeedback[];
};

function apiBase(): string {
  return process.env.API_URL ?? "http://127.0.0.1:8080";
}

function cookie(): string | undefined {
  return getRequestHeader("cookie");
}

function signIn(): never {
  throw redirect({ to: "/login", search: { returnTo: "/admin" } });
}

function notAdmin(): never {
  throw redirect({ to: "/" });
}

export const getAdminFeedback = createServerFn({ method: "GET" }).handler(
  async (): Promise<AdminView> => {
    try {
      const account = await fetchAccount(apiBase(), cookie());
      if (!account) signIn();
      if (!account.admin) notAdmin();
      const feedback = await fetchAdminFeedback(apiBase(), cookie());
      return { email: account.email, feedback };
    } catch (error) {
      if (error instanceof SignedOut) signIn();
      if (error instanceof NotAdmin) notAdmin();
      throw error;
    }
  },
);

export const setFeedbackAnalysisHandled = createServerFn({ method: "POST" })
  .validator((data: { id: string; handled: boolean }) => {
    if (!data.id) throw new Error("missing analysis");
    return { id: data.id, handled: data.handled === true };
  })
  .handler(async ({ data }): Promise<HandledResult> => {
    try {
      return await setAnalysisHandled(apiBase(), data.id, data.handled, cookie());
    } catch (error) {
      if (error instanceof SignedOut) signIn();
      throw error;
    }
  });
