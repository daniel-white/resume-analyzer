export type BuildInfo = {
  version: string;
  commit: string;
  dev: boolean;
};

// Injected by Vite at build time. Available in both the browser bundle and
// the Nitro server bundle so `/about` works during SSR.
export const frontendBuild: BuildInfo = {
  version: __FRONTEND_VERSION__,
  commit: __FRONTEND_COMMIT__,
  dev: __DEV_BUILD__,
};

function isSemVer(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(value)
  );
}

function isCommit(value: unknown): value is string {
  return typeof value === "string" && /^(?:unknown|[0-9a-f]{7})$/i.test(value);
}

function isBuildInfo(value: unknown): value is BuildInfo {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const candidate: Record<string, unknown> = value as Record<string, unknown>;
  return (
    isSemVer(candidate["version"]) &&
    isCommit(candidate["commit"]) &&
    typeof candidate["dev"] === "boolean"
  );
}

export async function fetchBackendBuild(baseUrl: string): Promise<BuildInfo | null> {
  try {
    const response = await fetch(new URL("/api/version", baseUrl));
    if (!response.ok) {
      return null;
    }
    const body: unknown = await response.json();
    return isBuildInfo(body) ? body : null;
  } catch {
    return null;
  }
}
