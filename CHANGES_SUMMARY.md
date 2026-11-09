# Summary of Changes for Non-Resume Detection

This implementation addresses the request to:
- Shut down the process when the summary model determines that an uploaded file does not look like a resume
- Provide an error state in the UI for the other page sections - hiding them as needed
- Still allow the version selection

## Changes Made

### 1. Backend (crates/app/src/worker.rs)
- **Status**: The summary generation in the worker.rs already correctly handles "not a resume" cases. When `summarizer.generate()` returns a profile summary with text starting with "This does not look like a résumé." it stores this as the profile summary. The summary model determines non-resume by analyzing the resume content.
- **Key**: The summary model in `crates/models/summary/src/lib.rs` already implements this logic in the `extract()` function and through the Ollama model response validation.

### 2. Frontend - Route and Component Changes

#### `web/src/routes/resumes/$id.tsx`
- Added `IS_NOT_RESUME_PREFIX` constant (same as in profile-summary)
- Added `isNotResume` state derived from `view.profileSummary?.text?.startsWith(IS_NOT_RESUME_PREFIX)`
- Pass `isNotResume` prop to `ResumeOverview` component
- Conditionally render sections based on `isNotResume`:
  - `ResumeComparison` - HIDDEN when not a resume
  - `LiveAnalysis` - HIDDEN when not a resume  
  - `ResumeStructure` - HIDDEN when not a resume
  - Analysis results section (ATS) - HIDDEN when not a resume
  - `ResumeVersions` - ALWAYS VISIBLE (version selection remains accessible)
  - `ProfileSummaryCard` - ALWAYS VISIBLE (shows "Not a résumé" error state)

#### `web/src/components/resume-overview.tsx`
- Added `isNotResume` prop to `ResumeOverview` component
- Conditionally display:
  - Analysis state and scoring when resume
  - Warning notice when not a resume: "This file does not appear to be a résumé. Upload a document with work experience, education, skills, or projects for analysis."

### 3. CSS Styles

#### `web/src/styles/app.css`
- Added `.analysis-state.not-resume-notice` style with destructive color for the warning notice

## User Experience Flow

### When a file is correctly identified as a resume:
- All sections visible: Overview, ATS results, comparison, structure, live analysis
- Version selection available via ResumeVersions component
- Normal analysis flow

### When a file is identified as "not a resume":
- ProfileSummaryCard shows: "Not a résumé" with message to upload proper document
- ResumeOverview shows warning notice
- **Hidden**: ResumeComparison, LiveAnalysis, ResumeStructure, ATS results
- **Visible**: ResumeOverview (with warning), ResumeVersions (version selection still accessible)
- User can still select different versions (version selection continues to work)
- User can upload a new version (via ResumeVersions upload button)

## Benefits

1. **Clear error state**: Users immediately see they're not seeing a resume analysis
2. **Clean UI**: Unrelated analysis sections are hidden when not applicable
3. **Continued functionality**: Version selection and navigation remain available
4. **Helpful guidance**: Clear message about what kind of documents are accepted
5. **Backward compatibility**: Resume processing works exactly as before when files are valid

## Technical Notes

- The summary model already correctly identifies non-resume files
- The UI correctly propagates this state throughout the page
- Changes are minimal and focused on conditional rendering
- No breaking changes to existing functionality