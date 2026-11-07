import type { VerdictCard } from "~/server/resumes";

const READING = "Reading the file";
const EMPTY = "\u2014";

export function VerdictCardView({ card }: { card: VerdictCard }) {
  const kept = card.pending ? [READING] : card.kept.length > 0 ? card.kept : [EMPTY];
  const missed = card.pending ? [READING] : card.missed.length > 0 ? card.missed : [EMPTY];

  return (
    <article className="card">
      <header>
        <div>
          <h2>{card.name}</h2>
          <p className="sub">{card.subtitle}</p>
        </div>
        {card.pending ? (
          <div className="pending" aria-label="Pending" />
        ) : (
          <p className={`score ${card.band}`}>{card.score}</p>
        )}
      </header>
      <div className="cols">
        <div>
          <h3>Kept</h3>
          <ul>
            {kept.map((item, index) => (
              <li key={`${item}-${index}`} className={card.pending ? "quiet" : undefined}>
                {item}
              </li>
            ))}
          </ul>
        </div>
        <div>
          <h3>Dropped</h3>
          <ul>
            {missed.map((item, index) => (
              <li key={`${item}-${index}`} className={card.pending ? "quiet" : undefined}>
                {item}
              </li>
            ))}
          </ul>
        </div>
      </div>
      {!card.pending && card.factors.length > 0 ? (
        <div className="factors">
          {card.factors.map((factor) => (
            <div key={`${factor.label}-${factor.delta}`}>
              <span>{factor.label}</span>
              <span>{factor.delta}</span>
            </div>
          ))}
        </div>
      ) : null}
    </article>
  );
}
