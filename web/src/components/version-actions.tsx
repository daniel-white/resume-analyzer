import { useId } from "react";
import { DropdownMenu } from "radix-ui";
import { Check, ChevronDown, FileClock, LoaderCircle, Upload } from "lucide-react";

import { useResumeDetail } from "./resume-detail-context";

/** Versions-only picker plus the upload action, both beside the résumé name. */
export function VersionActions() {
  const {
    view,
    versionId,
    busy,
    selectVersion,
    uploadVersion,
    clearError,
  } = useResumeDetail();
  const inputId = useId();
  const selected = view.versions.find((version) => version.id === versionId);

  return (
    <div className="resume-detail-version-actions" aria-busy={busy !== null}>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          className="version-trigger"
          aria-label="Version"
          disabled={busy !== null}
        >
          <FileClock size={14} aria-hidden="true" />
          <span>v{selected?.number ?? 1}</span>
          {selected && view.versions[0]?.id === selected.id ? (
            <span className="version-latest">Latest</span>
          ) : null}
          {busy === "select" ? <LoaderCircle className="busy-icon" size={14} aria-hidden="true" />
            : <ChevronDown size={14} aria-hidden="true" />}
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className="version-menu" align="end" sideOffset={6}>
            <DropdownMenu.Label className="version-menu-label">Version history</DropdownMenu.Label>
            <DropdownMenu.Separator className="version-menu-separator" />
            {view.versions.map((version, index) => (
              <DropdownMenu.Item
                key={version.id}
                className="version-menu-item"
                disabled={busy !== null}
                onSelect={() => void selectVersion(version.id)}
              >
                <span className="version-menu-copy">
                  <strong>v{version.number}{index === 0 ? " · Latest" : ""}</strong>
                  <small>{version.filename}</small>
                </span>
                {version.id === versionId ? <Check size={15} aria-hidden="true" /> : null}
              </DropdownMenu.Item>
            ))}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
      <label className="resume-detail-upload" htmlFor={inputId}>
        {busy === "upload" ? <LoaderCircle className="busy-icon" size={16} aria-hidden="true" />
          : <Upload size={16} aria-hidden="true" />}
        <span className="upload-long">Upload new version</span>
        <span className="upload-short">Upload</span>
        <input
          id={inputId}
          type="file"
          aria-label="Upload new version"
          accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
          disabled={busy !== null}
          onChange={(event) => void uploadVersion(event)}
          onFocus={clearError}
        />
      </label>
    </div>
  );
}
