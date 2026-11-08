export function FactorList({
  factors,
}: {
  factors: { label: string; delta: number; advice?: string }[];
}) {
  return (
    <ul className="flex flex-col border-t pt-3 font-sans text-sm leading-6">
      {factors.map((factor) => (
        <li key={factor.label} className="flex justify-between gap-3 py-1">
          <span className="min-w-0">
            <span className="block">{factor.label}</span>
            {factor.advice ? (
              <span className="mt-0.5 block text-sm leading-6 text-muted-foreground">
                {factor.advice}
              </span>
            ) : null}
          </span>
          <span className="shrink-0 font-mono text-xs tabular-nums">
            {factor.delta > 0 ? `+${factor.delta}` : factor.delta}
          </span>
        </li>
      ))}
    </ul>
  );
}
