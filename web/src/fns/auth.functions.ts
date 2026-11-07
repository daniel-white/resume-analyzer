import { createServerFn } from "@tanstack/react-start";
import { getRequestHeader, setResponseHeader } from "@tanstack/react-start/server";
import { redirect } from "@tanstack/react-router";

import {
  fetchAccount,
  postLogout,
  requestMagicLink,
  verifyMagicLink,
  type Account,
  type MagicLinkResult,
} from "../server/auth";
import { SignedOut } from "../server/session";
import { resumeReturnTo } from "../lib/return-to";

export type { Account, MagicLinkResult };

function apiBase(): string {
  return process.env.API_URL ?? "http://127.0.0.1:8080";
}

function cookie(): string | undefined {
  return getRequestHeader("cookie");
}

function copySetCookie(setCookie: string | null): void {
  if (!setCookie) return;
  setResponseHeader("set-cookie", setCookie);
}

export const getAccount = createServerFn({ method: "GET" }).handler(
  async (): Promise<Account | null> => {
    try {
      return await fetchAccount(apiBase(), cookie());
    } catch (error) {
      if (error instanceof SignedOut) return null;
      throw error;
    }
  },
);

export const sendMagicLink = createServerFn({ method: "POST" })
  .validator((data: { email: string; returnTo?: string }) => ({ email: data.email, returnTo: resumeReturnTo(data.returnTo) }))
  .handler(async ({ data }): Promise<MagicLinkResult> =>
    requestMagicLink(apiBase(), data.email, cookie(), data.returnTo),
  );

export const verifyLink = createServerFn({ method: "POST" })
  .validator((data: { token: string; returnTo?: string }) => ({ token: data.token, returnTo: resumeReturnTo(data.returnTo) }))
  .handler(async ({ data }): Promise<{ ok: false; message: string }> => {
    const result = await verifyMagicLink(apiBase(), data.token, cookie());
    if (!result.ok) return result;
    copySetCookie(result.setCookie);
    throw redirect({
      href: data.returnTo ?? "/",
      headers: { "set-cookie": result.setCookie },
    });
  });

export const signOut = createServerFn({ method: "POST" }).handler(async (): Promise<void> => {
  const cleared = await postLogout(apiBase(), cookie());
  copySetCookie(cleared ?? "session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0");
});
