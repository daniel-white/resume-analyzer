export type Check = "ok" | "error";

export type HealthReport = {
  status: Check;
  checks: {
    postgres: Check;
    s3: Check;
    smtp: Check;
  };
};

export const unreachable: HealthReport = {
  status: "error",
  checks: { postgres: "error", s3: "error", smtp: "error" },
};

export function summarize(report: HealthReport): string {
  const { checks } = report;
  return `${report.status}: postgres ${checks.postgres}, s3 ${checks.s3}, smtp ${checks.smtp}`;
}

export async function fetchHealth(baseUrl: string): Promise<HealthReport> {
  try {
    const response = await fetch(new URL("/health", baseUrl));
    return (await response.json()) as HealthReport;
  } catch {
    return unreachable;
  }
}
