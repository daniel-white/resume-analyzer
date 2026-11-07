import { BackLink } from "./back-link";
import { Page } from "./page";

export function NotFound() {
  return (
    <Page wide>
      <header className="top">
        <div className="wordmark">Resume analyzer</div>
      </header>
      <BackLink />
      <div className="missing">
        <h1>That resume is not here.</h1>
      </div>
    </Page>
  );
}
