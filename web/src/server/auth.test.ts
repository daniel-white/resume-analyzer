import { describe, expect, it } from "vitest";

import { magicLinkMessage, verifyMessage } from "./auth";

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
});
