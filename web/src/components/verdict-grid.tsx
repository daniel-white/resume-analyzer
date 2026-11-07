import type { VerdictCard } from "~/server/resumes";

import { VerdictCardView } from "./verdict-card";

export function VerdictGrid({ cards }: { cards: VerdictCard[] }) {
  return (
    <div className="grid">
      {cards.map((card) => (
        <VerdictCardView key={card.id} card={card} />
      ))}
    </div>
  );
}
