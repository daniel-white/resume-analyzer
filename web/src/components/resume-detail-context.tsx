import { isRedirect, useNavigate, useRouter, useRouterState } from "@tanstack/react-router";
import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ChangeEvent,
  type ReactNode,
} from "react";

import { CheckCircle2, LoaderCircle, AlertTriangle } from "lucide-react";

import { removeResume, uploadResume, type SignedInTeardown } from "~/fns/resumes.functions";

export type ResumeDetailBusy = "select" | "upload" | "delete" | null;

export const IS_NOT_RESUME_PREFIX = "This does not look like a résumé.";

type ResumeDetailValue = {
  view: SignedInTeardown;
  resumeId: string;
  versionId: string;
  versionNumber: number;
  provider?: string | undefined;
  isResume: boolean;
  busy: ResumeDetailBusy;
  notice: string | null;
  error: string | null;
  clearError: () => void;
  clearNotice: () => void;
  selectVersion: (id: string) => Promise<void>;
  uploadVersion: (event: ChangeEvent<HTMLInputElement>) => Promise<void>;
  deleteVersion: () => Promise<void>;
};

const ResumeDetailContext = createContext<ResumeDetailValue | null>(null);

type ResumeDetailPath =
  | "/resumes/$id"
  | "/resumes/$id/structure"
  | "/resumes/$id/skills"
  | "/resumes/$id/ats-insights"
  | "/resumes/$id/ideas";

function resumeDetailPath(pathname: string): ResumeDetailPath {
  if (pathname.endsWith("/structure")) return "/resumes/$id/structure";
  if (pathname.endsWith("/skills")) return "/resumes/$id/skills";
  if (pathname.endsWith("/ats-insights")) return "/resumes/$id/ats-insights";
  if (pathname.endsWith("/ideas")) return "/resumes/$id/ideas";
  return "/resumes/$id";
}

/** Shared version state and actions for the résumé detail layout and its tab routes. */
export function useResumeDetail(): ResumeDetailValue {
  const value = useContext(ResumeDetailContext);
  if (!value) throw new Error("useResumeDetail must be used inside ResumeDetailProvider");
  return value;
}

/** Upload, delete and version-switch outcomes, shared across tab routes. */
export function ResumeDetailFeedback() {
  const { busy, notice, error } = useResumeDetail();
  return (
    <div className="resume-detail-feedback" aria-live="polite">
      {busy ? (
        <p className="version-feedback" role="status">
          <LoaderCircle className="busy-icon" size={15} aria-hidden="true" />
          {busy === "upload" ? "Uploading…" : busy === "delete" ? "Deleting…" : "Loading version…"}
        </p>
      ) : null}
      {notice ? (
        <p className="version-feedback success" role="status"><CheckCircle2 size={15} aria-hidden="true" />{notice}</p>
      ) : null}
      {error ? (
        <p className="version-feedback error" role="alert"><AlertTriangle size={15} aria-hidden="true" />{error}</p>
      ) : null}
    </div>
  );
}

export function ResumeDetailProvider({
  view,
  provider,
  children,
}: {
  view: SignedInTeardown;
  provider?: string | undefined;
  children: ReactNode;
}) {
  const navigate = useNavigate();
  const router = useRouter();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const [busy, setBusy] = useState<ResumeDetailBusy>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const selected = view.versions.find((version) => version.id === view.versionId);
  const versionNumber = selected?.number ?? 1;
  const isResume = !(view.profileSummary?.text?.startsWith(IS_NOT_RESUME_PREFIX) ?? false);

  async function handleError(cause: unknown, message: string) {
    if (isRedirect(cause)) await navigate({ to: "/login" });
    else setError(message);
  }

  function searchFor(version: string) {
    return provider ? { version, provider } : { version };
  }

  async function selectVersion(id: string) {
    if (busy || id === view.versionId) return;
    setBusy("select");
    setNotice(null);
    setError(null);
    try {
      await navigate({
        to: resumeDetailPath(pathname),
        params: { id: view.resumeId },
        search: searchFor(id),
      });
    } catch (cause) {
      await handleError(cause, "Could not load that version. Please try again.");
    } finally {
      setBusy(null);
    }
  }

  async function uploadVersion(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || busy) return;
    setBusy("upload");
    setNotice(null);
    setError(null);
    try {
      const data = new FormData();
      data.set("file", file, file.name);
      data.set("resumeId", view.resumeId);
      const result = await uploadResume({ data });
      if (!result.ok) {
        setError(result.message);
        return;
      }
      await navigate({
        to: resumeDetailPath(pathname),
        params: { id: view.resumeId },
        search: searchFor(result.version_id),
      });
      setNotice("New version uploaded. Previous versions are unchanged.");
    } catch (cause) {
      await handleError(cause, "Upload failed. Please try again.");
    } finally {
      setBusy(null);
    }
  }

  async function deleteVersion() {
    if (busy) return;
    setBusy("delete");
    setNotice(null);
    setError(null);
    try {
      const result = await removeResume({
        data: { id: view.resumeId, versionId: view.versionId },
      });
      if (!result.ok) {
        setError(result.message);
        return;
      }
      if (view.versions.length === 1) {
        await navigate({ to: "/" });
      } else {
        const remaining = view.versions.find((version) => version.id !== view.versionId);
        if (remaining) {
          await navigate({
            to: resumeDetailPath(pathname),
            params: { id: view.resumeId },
            search: searchFor(remaining.id),
            replace: true,
          });
        }
        setNotice(`Version ${versionNumber} deleted.`);
      }
      await router.invalidate();
    } catch (cause) {
      await handleError(cause, "Could not delete the résumé. Please try again.");
    } finally {
      setBusy(null);
    }
  }

  const value = useMemo<ResumeDetailValue>(() => ({
    view,
    resumeId: view.resumeId,
    versionId: view.versionId,
    versionNumber,
    provider,
    isResume,
    busy,
    notice,
    error,
    clearError: () => setError(null),
    clearNotice: () => setNotice(null),
    selectVersion,
    uploadVersion,
    deleteVersion,
  }), [view, provider, pathname, busy, notice, error, versionNumber, isResume]);

  return <ResumeDetailContext.Provider value={value}>{children}</ResumeDetailContext.Provider>;
}
