import { describe, expect, it } from "vitest";
import { hsPip, lolPip, rlPip } from "./pips";

const hs = {
  supported: true,
  enabled: false,
  install_dir: "C:/HS" as string | null,
  hdt_enabled: false,
  last_match: null as Record<string, unknown> | null,
};
const rl = { enabled: false, install_dir: "C:/RL" as string | null, player_name: "Djam", last_mismatch: "" };

describe("hsPip", () => {
  it("is on when tracking is enabled", () => expect(hsPip({ ...hs, enabled: true })).toBe("on"));
  it("is off when disabled with everything found", () => expect(hsPip(hs)).toBe("off"));
  it("needs setup when the install folder is unknown", () =>
    expect(hsPip({ ...hs, install_dir: null })).toBe("todo"));
  it("is off where Hearthstone does not exist", () =>
    expect(hsPip({ supported: false, enabled: false, install_dir: null })).toBe("off"));
  it("is off while the status is not loaded", () => expect(hsPip(null)).toBe("off"));
  it("needs setup when HDT reading is on and the last match's rating was not found", () =>
    expect(hsPip({ ...hs, enabled: true, hdt_enabled: true, last_match: { rating_missing: true } })).toBe("todo"));
  it("stays on when the rating was found", () =>
    expect(hsPip({ ...hs, enabled: true, hdt_enabled: true, last_match: { rating_after: 5644 } })).toBe("on"));
  it("ignores a missing rating once HDT reading is off", () =>
    expect(hsPip({ ...hs, enabled: true, hdt_enabled: false, last_match: { rating_missing: true } })).toBe("on"));
});

describe("rlPip", () => {
  it("is on when enabled and configured", () => expect(rlPip({ ...rl, enabled: true })).toBe("on"));
  it("is off when disabled and configured", () => expect(rlPip(rl)).toBe("off"));
  it("needs setup without a player name", () => expect(rlPip({ ...rl, player_name: "  " })).toBe("todo"));
  it("needs setup without an install folder", () => expect(rlPip({ ...rl, install_dir: null })).toBe("todo"));
  it("needs setup after a match that matched nobody, even when enabled", () =>
    expect(rlPip({ ...rl, enabled: true, last_mismatch: "A, B" })).toBe("todo"));
  it("is off while the status is not loaded", () => expect(rlPip(null)).toBe("off"));
});

describe("lolPip", () => {
  it("follows the switch, there is nothing to set up", () => {
    expect(lolPip({ enabled: true })).toBe("on");
    expect(lolPip({ enabled: false })).toBe("off");
    expect(lolPip(null)).toBe("off");
  });
});
