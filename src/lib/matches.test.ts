import { describe, expect, it } from "vitest";
import { ago, hsSummary, ordinal, rlSummary } from "./matches";

const now = new Date("2026-09-24T12:00:00Z");

describe("ago", () => {
  it("says just now under a minute", () => {
    expect(ago("2026-09-24T11:59:30Z", now, "fr")).toBe("à l'instant");
    expect(ago("2026-09-24T11:59:30Z", now, "en")).toBe("just now");
  });
  it("counts minutes, hours, then days", () => {
    expect(ago("2026-09-24T11:48:00Z", now, "fr")).toBe("il y a 12\u00a0min");
    expect(ago("2026-09-24T09:00:00Z", now, "fr")).toBe("il y a 3\u00a0h");
    expect(ago("2026-09-23T10:00:00Z", now, "fr")).toBe("hier");
    expect(ago("2026-09-24T11:48:00Z", now, "en")).toBe("12 min. ago");
  });
  it("returns an empty string for an unreadable date", () => {
    expect(ago("nope", now, "fr")).toBe("");
  });
});

describe("ordinal", () => {
  it("writes French placements", () => {
    expect(ordinal(1, "fr")).toBe("1er");
    expect(ordinal(4, "fr")).toBe("4e");
  });
  it("writes English placements", () => {
    expect(ordinal(1, "en")).toBe("1st");
    expect(ordinal(2, "en")).toBe("2nd");
    expect(ordinal(3, "en")).toBe("3rd");
    expect(ordinal(8, "en")).toBe("8th");
  });
});

describe("hsSummary", () => {
  it("shows a Battlegrounds placement and the turns", () => {
    const s = hsSummary(
      { mode: "battlegrounds", result: "win", placement: 1, turns: 18, played_at: "2026-09-24T11:48:00Z" },
      now,
      "fr",
    );
    expect(s).toEqual({ title: "Champs de bataille · 1er", detail: "il y a 12\u00a0min · 18 tours", highlight: true });
  });
  it("shows a regular game's result", () => {
    const s = hsSummary(
      { mode: "constructed", result: "loss", turns: 9, played_at: "2026-09-24T11:48:00Z" },
      now,
      "en",
    );
    expect(s).toEqual({ title: "Regular game · Loss", detail: "12 min. ago · 9 turns", highlight: false });
  });
  it("never shows the hero's card id", () => {
    const s = hsSummary(
      { mode: "battlegrounds", result: "loss", placement: 5, hero_card_id: "BG36_HERO_000", played_at: "2026-09-24T11:48:00Z" },
      now,
      "fr",
    );
    expect(JSON.stringify(s)).not.toContain("BG36");
  });
  it("returns null without a payload", () => expect(hsSummary(null, now, "fr")).toBeNull());
});

describe("rlSummary", () => {
  it("puts the member's score first, on either side", () => {
    const orange = rlSummary(
      { result: "win", player_team: 1, team_blue_score: 1, team_orange_score: 3, team_size: 2, played_at: "2026-09-24T11:48:00Z" },
      now,
      "fr",
    );
    expect(orange).toEqual({ title: "Victoire · 3 - 1", detail: "2c2 · il y a 12\u00a0min", highlight: true });
    const blue = rlSummary(
      { result: "loss", player_team: 0, team_blue_score: 0, team_orange_score: 2, team_size: 3, played_at: "2026-09-24T11:48:00Z" },
      now,
      "en",
    );
    expect(blue).toEqual({ title: "Loss · 0 - 2", detail: "3v3 · 12 min. ago", highlight: false });
  });
  it("returns null without a payload", () => expect(rlSummary(null, now, "fr")).toBeNull());
});
