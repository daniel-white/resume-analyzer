import { describe, expect, it } from "vitest";

import { readSetCookie } from "./session";

describe("session cookie", () => {
  it("reads the first Set-Cookie and leaves an empty header unset", () => {
    const headers = new Headers();
    headers.append("set-cookie", "session=abc.def; HttpOnly; SameSite=Lax; Path=/");
    expect(readSetCookie(headers)).toBe("session=abc.def; HttpOnly; SameSite=Lax; Path=/");
    expect(readSetCookie(new Headers())).toBeNull();
  });
});
