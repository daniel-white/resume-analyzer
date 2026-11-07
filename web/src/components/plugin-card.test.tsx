import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PluginCard } from "./plugin-card";

describe("IndyHackersHiring card", () => {
  for (const reading of [true, false]) {
    it(`keeps its decorative mark, name and disclosure when ${reading ? "pending" : "complete"}`, () => {
      const bias = "Tolerates tables and text boxes, loses columns and images. Simulated, modeled on publicly described parser behavior.";
      const html = renderToStaticMarkup(<PluginCard
        plugin={{ id: "indy_hackers_hiring", name: "IndyHackersHiring", bias }}
        reading={reading} selected={false} onSelect={() => {}}
        verdict={reading ? null : { score: 65, kept: ["email"], dropped: ["columns"], factors: [] }}
      />);
      expect(html).toContain('src="/marks/indy_hackers_hiring.png"');
      expect(html).toContain('alt=""');
      expect(html).toContain("IndyHackersHiring");
      expect(html).toContain(bias);
      expect(html.includes('aria-label="Pending"')).toBe(reading);
    });
  }
});
