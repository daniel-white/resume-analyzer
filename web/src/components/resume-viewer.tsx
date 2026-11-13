import { Component, lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import { Download, FileText } from "lucide-react";
import { DocumentSkeleton } from "./resume-loading";

// The renderer uses browser APIs, so import it only after the file loads on the client.
const ResumeDocument = lazy(() => import("./resume-document"));

class PreviewBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    return this.state.failed
      ? <p role="alert" className="p-4">This file could not be previewed. You can download the original above.</p>
      : this.props.children;
  }
}

export function ResumeViewer({
  file,
  filename,
}: {
  file: { url: string; mediaType: string };
  filename: string;
}) {
  const [uri, setUri] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let objectUrl: string | undefined;
    setUri(null);
    setError(null);

    async function load() {
      try {
        const response = await fetch(file.url, {
          credentials: "same-origin",
          cache: "no-store",
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error(response.status === 401
            ? "Your session has expired. Sign in again to preview this file."
            : "This file could not be loaded.");
        }
        const blob = await response.blob();
        if (controller.signal.aborted) return;
        objectUrl = URL.createObjectURL(blob);
        setUri(objectUrl);
      } catch (cause) {
        if (!controller.signal.aborted) {
          setError(cause instanceof Error ? cause.message : "This file could not be loaded.");
        }
      }
    }
    void load();
    return () => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [file.url]);

  const resumeKind = file.mediaType === "application/pdf"
    ? "PDF"
    : file.mediaType === "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
      ? "DOCX"
      : "file";

  return (
    <section className="resume-viewer overflow-hidden rounded-xl border border-black/10 bg-white text-black" aria-label="Original résumé">
      <div className="viewer-heading">
        <h2><FileText size={17} aria-hidden="true" /> Original {resumeKind} résumé</h2>
        <a href={uri ?? file.url} download={filename}><Download size={15} aria-hidden="true" /> Download original</a>
      </div>
      {error ? <p role="alert" className="p-4">{error}</p> : uri ? (
        <PreviewBoundary>
          <Suspense fallback={<DocumentSkeleton />}>
            <ResumeDocument uri={uri} filename={filename} mediaType={file.mediaType} />
          </Suspense>
        </PreviewBoundary>
      ) : <DocumentSkeleton label="Loading original file…" />}
    </section>
  );
}
