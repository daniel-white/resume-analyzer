import { BackLink } from "./back-link";
import { Page } from "./page";
import { SiteHeader } from "./site-header";

export function NotFound() {
  return (
    <Page wide>
      <SiteHeader />
      <BackLink />
      <div className="missing">
        <h1>That resume is not here.</h1>
      </div>
    </Page>
  );
}
