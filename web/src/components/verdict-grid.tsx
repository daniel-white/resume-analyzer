import type { SectionKind, VerdictCard } from "~/server/resumes";

import { VerdictCardView } from "./verdict-card";

export function VerdictGrid({ cards, provider, highlightKind }: { cards: VerdictCard[]; provider?: string | undefined; highlightKind?: SectionKind | null | undefined }) {
  return (
    <div className="grid">
      {cards.map((card) => (
        <VerdictCardView key={`${card.id}-${provider ?? ""}`} card={card} initiallySelected={card.id === provider} highlightKind={highlightKind} />
      ))}
    </div>
  );
}
