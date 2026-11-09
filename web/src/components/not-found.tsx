import { BackLink } from "./back-link";
import { Page } from "./page";
import { PublicHeader } from "./public-header";

export function NotFound() {
  return (
    <Page wide>
      <PublicHeader />
      <BackLink />
      <div className="missing">
        <h1>That résumé is not here.</h1>
        <p>It may have been removed, or the link may be incomplete. Head back to your résumés for another read.</p>
      </div>
    </Page>
  );
}
