import { describe, expect, it, vi } from "vitest";

import { magicLinkMessage, requestMagicLink, verifyMessage } from "./auth";

describe("sign-in messages", () => {
  it("names a bad email and a mail failure", () => {
    expect(magicLinkMessage(400, JSON.stringify({ error: "email" }))).toBe(
      "That email does not look right.",
    );
    expect(magicLinkMessage(502, JSON.stringify({ error: "mail" }))).toBe(
      "The sign-in email did not send.",
    );
    expect(magicLinkMessage(500, "nope")).toBe("The sign-in email did not send.");
  });

  it("names an expired link", () => {
    expect(verifyMessage(400, JSON.stringify({ error: "link" }))).toBe(
      "That link has expired. Ask for another.",
    );
    expect(verifyMessage(500, "")).toBe("That link did not sign you in.");
  });

  it("passes the analysis destination to the sign-in email API", async () => {
    const destination = "/resumes/12345678-1234-1234-1234-123456789abc?provider=greenline";
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 202 }));
    try {
      expect(await requestMagicLink("http://localhost:8080", " ada@example.com ", undefined, destination)).toEqual({ ok: true });
      const request = fetchMock.mock.calls[0][1];
      expect(JSON.parse(request!.body as string)).toEqual({ email: "ada@example.com", return_to: destination });
    } finally {
      fetchMock.mockRestore();
    }
  });
});
