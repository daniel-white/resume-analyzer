import DocViewer, { DocxRenderer, PDFRenderer } from "@iamjariwala/react-doc-viewer";
import "@iamjariwala/react-doc-viewer/dist/index.css";
import { memo, useMemo } from "react";

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
  const documents = useMemo(
    () => [{ uri, fileName: filename, fileType: mediaType }],
    [uri, filename, mediaType],
  );

  return (
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
  );
}

// Analysis polling updates the surrounding page every two seconds. Do not
// restart document parsing when the file itself has not changed.
export default memo(ResumeDocument);
