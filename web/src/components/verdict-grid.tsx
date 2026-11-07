import type { VerdictCard } from "~/server/resumes";

import { VerdictCardView } from "./verdict-card";

export function VerdictGrid({ cards, provider }: { cards: VerdictCard[]; provider?: string }) {
  return (
    <div className="grid">
      {cards.map((card) => (
        <VerdictCardView key={`${card.id}-${provider ?? ""}`} card={card} initiallySelected={card.id === provider} />
      ))}
    </div>
  );
}
