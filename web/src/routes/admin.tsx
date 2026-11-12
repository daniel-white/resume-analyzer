import { createFileRoute } from "@tanstack/react-router";

import { AdminFeedbackDashboard } from "~/components/admin-feedback";
import { Page } from "~/components/page";
import { SiteHeader } from "~/components/site-header";
import { getAdminFeedback } from "~/fns/admin.functions";

export const Route = createFileRoute("/admin")({
  head: () => ({
    meta: [
      { title: "Feedback triage — The Other Read" },
      {
        name: "description",
        content: "Review downvoted résumé sections and the model diagnoses stored for them.",
      },
    ],
  }),
  loader: () => getAdminFeedback(),
  component: AdminPage,
});

function AdminPage() {
  const { email, feedback, timezone } = Route.useLoaderData();
  return (
    <Page wide>
      <SiteHeader email={email} admin />
      <AdminFeedbackDashboard email={email} feedback={feedback} timezone={timezone} />
    </Page>
  );
}
