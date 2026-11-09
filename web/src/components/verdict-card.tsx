import type { SectionKind, VerdictCard } from "~/server/resumes";
import { useState } from "react";

import { PluginCard } from "./plugin-card";

export function VerdictCardView({ card, initiallySelected = false, highlightKind }: { card: VerdictCard; initiallySelected?: boolean; highlightKind?: SectionKind | null }) {
  const [selected, setSelected] = useState(initiallySelected);
  return (
    <article aria-label={card.name}>
      <PluginCard
        plugin={{ id: card.id, name: card.name, bias: card.subtitle }}
        verdict={card.pending ? null : {
          score: card.score,
          kept: card.kept,
          dropped: card.missed,
          summary: card.summary,
          report: card.report,
          factors: card.factors.map((factor) => ({ ...factor, delta: Number(factor.delta), advice: factor.advice ?? "" })),
        }}
        reading={card.pending}
        parse={card.pending ? null : card.parse}
        selected={selected}
        onSelect={() => setSelected(!selected)}
        highlightKind={highlightKind}
      />
    </article>
  );
}
