import DocViewer, { DocxRenderer, PDFRenderer } from "@iamjariwala/react-doc-viewer";
import "@iamjariwala/react-doc-viewer/dist/index.css";
import { memo, useEffect, useMemo, useRef } from "react";

const renderers = [PDFRenderer, DocxRenderer];

function ResumeDocument({
  uri,
  filename,
  mediaType,
}: {
  uri: string;
  filename: string;
  mediaType: string;
}) {
  const container = useRef<HTMLDivElement>(null);
  const documents = useMemo(
    () => [{ uri, fileName: filename, fileType: mediaType }],
    [uri, filename, mediaType],
  );

  useEffect(() => {
    const element = container.current;
    if (!element) return;
    // Both renderers insert links asynchronously, including after zooming.
    const updateLinks = () => {
      element.querySelectorAll<HTMLAnchorElement>("a[href]").forEach((link) => {
        link.target = "_blank";
        link.rel = "noopener noreferrer";
      });
    };
    updateLinks();
    const observer = new MutationObserver(updateLinks);
    observer.observe(element, { childList: true, subtree: true, attributes: true, attributeFilter: ["href"] });
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={container}>
      <DocViewer
        documents={documents}
        pluginRenderers={renderers}
        style={{ height: "min(78vh, 900px)", minHeight: 400 }}
        config={{
          header: { disableHeader: true },
          docx: { useOfficeOnlineViewer: false },
          pdfVerticalScrollByDefault: true,
          textSelection: { enableTextSelection: true },
          search: { enableSearch: true },
          password: { enablePasswordPrompt: true },
        }}
      />
    </div>
  );
}

// Analysis polling updates the surrounding page every two seconds. Do not
// restart document parsing when the file itself has not changed.
export default memo(ResumeDocument);
