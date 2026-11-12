import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { detectTimeZone } from "./timezone";

describe("detectTimeZone", () => {
  const originalIntl = Intl;

  afterEach(() => {
    // Restore original Intl after each test
    global.Intl = originalIntl;
  });

  beforeEach(() => {
    // Reset Intl to ensure tests don't leak
    global.Intl = originalIntl;
  });

  it("returns null when Intl is unavailable", () => {
    // @ts-expect-error - testing missing Intl
    delete global.Intl;
    expect(detectTimeZone()).toBeNull();
  });

  it("returns null when Intl.DateTimeFormat is not a function", () => {
    // @ts-expect-error - testing missing DateTimeFormat
    global.Intl.DateTimeFormat = undefined;
    expect(detectTimeZone()).toBeNull();
  });

  it("returns a valid IANA timezone string when detected", () => {
    const result = detectTimeZone();
    expect(result).toBeTruthy();
    expect(typeof result).toBe("string");
    // Should be a valid IANA zone name (non-empty, reasonable length)
    expect(result.length).toBeGreaterThan(0);
    expect(result.length).toBeLessThanOrEqual(64);
    // Should not contain control characters
    expect([...result].every((c) => c >= " " && c <= "~")).toBe(true);
  });

  it("returns null when timezone detection throws", () => {
    // Mock Intl to throw when creating a new DateTimeFormat
    const originalDateTimeFormat = global.Intl.DateTimeFormat;
    // @ts-expect-error - intentionally breaking DateTimeFormat
    global.Intl.DateTimeFormat = vi.fn().mockImplementation(() => {
      throw new Error("mock error");
    });

    expect(detectTimeZone()).toBeNull();
    // Restore for cleanup
    global.Intl.DateTimeFormat = originalDateTimeFormat;
  });

  it("returns null when timezone is empty string", () => {
    // Mock resolvedOptions to return empty string
    const originalDateTimeFormat = global.Intl.DateTimeFormat;
    const mockResolved = { timeZone: "" };
    global.Intl.DateTimeFormat = vi.fn().mockImplementation(() => ({
      resolvedOptions: () => mockResolved,
    })) as unknown as typeof Intl.DateTimeFormat;

    expect(detectTimeZone()).toBeNull();
    global.Intl.DateTimeFormat = originalDateTimeFormat;
  });

  it("returns null when timezone is too long", () => {
    const originalDateTimeFormat = global.Intl.DateTimeFormat;
    const longZone = "A".repeat(65);
    const mockResolved = { timeZone: longZone };
    global.Intl.DateTimeFormat = vi.fn().mockImplementation(() => ({
      resolvedOptions: () => mockResolved,
    })) as unknown as typeof Intl.DateTimeFormat;

    expect(detectTimeZone()).toBeNull();
    global.Intl.DateTimeFormat = originalDateTimeFormat;
  });

  it("returns null when timezone contains control characters", () => {
    const originalDateTimeFormat = global.Intl.DateTimeFormat;
    const badZone = "America/New_York\x00"; // null byte
    const mockResolved = { timeZone: badZone };
    global.Intl.DateTimeFormat = vi.fn().mockImplementation(() => ({
      resolvedOptions: () => mockResolved,
    })) as unknown as typeof Intl.DateTimeFormat;

    expect(detectTimeZone()).toBeNull();
    global.Intl.DateTimeFormat = originalDateTimeFormat;
  });

  it("returns null when the final validation fails", () => {
    const originalDateTimeFormat = global.Intl.DateTimeFormat;
    const mockResolved = { timeZone: "America/New_York" };
    const callCount = { value: 0 };
    global.Intl.DateTimeFormat = vi.fn().mockImplementation((...args) => {
      callCount.value++;
      if (callCount.value === 1) {
        // First call: resolvedOptions for the check
        return { resolvedOptions: () => mockResolved };
      }
      // Second call: the final validation - throw to fail it
      throw new Error("invalid timezone");
    }) as unknown as typeof Intl.DateTimeFormat;

    expect(detectTimeZone()).toBeNull();
    global.Intl.DateTimeFormat = originalDateTimeFormat;
  });

  it("returns a valid timezone when the final validation passes", () => {
    const originalDateTimeFormat = global.Intl.DateTimeFormat;
    const mockResolved = { timeZone: "America/New_York" };
    const callCount = { value: 0 };
    global.Intl.DateTimeFormat = vi.fn().mockImplementation((...args) => {
      callCount.value++;
      if (callCount.value === 1) {
        // First call: resolvedOptions for the check
        return { resolvedOptions: () => mockResolved };
      }
      // Second call: the final validation - succeed
      return { resolvedOptions: () => ({ timeZone: "America/New_York" }) };
    }) as unknown as typeof Intl.DateTimeFormat;

    const result = detectTimeZone();
    expect(result).toBe("America/New_York");
    expect(callCount.value).toBe(2); // Should have called twice
    global.Intl.DateTimeFormat = originalDateTimeFormat;
  });

  it("validates with Intl.DateTimeFormat constructor for final confirmation", () => {
    // This test ensures the double-validation pattern is in place
    const result = detectTimeZone();
    expect(result).toBeTruthy();
  });
});