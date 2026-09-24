import { describe, expect, it } from "vitest";
import { pickLang, translate, type Key } from "./i18n";

describe("pickLang", () => {
  it("picks French for any French locale", () => {
    expect(pickLang("fr")).toBe("fr");
    expect(pickLang("fr-FR")).toBe("fr");
    expect(pickLang("fr-CA")).toBe("fr");
  });
  it("falls back to English otherwise", () => {
    expect(pickLang("en-US")).toBe("en");
    expect(pickLang("de-DE")).toBe("en");
    expect(pickLang(undefined)).toBe("en");
  });
});

describe("translate", () => {
  it("returns the text in the requested language", () => {
    expect(translate("fr", "tab.settings")).toBe("Réglages");
    expect(translate("en", "tab.settings")).toBe("Settings");
  });
  it("replaces every parameter", () => {
    expect(translate("fr", "home.gamesCount", { n: "10 871" })).toBe("10 871 jeux reconnus");
    expect(translate("en", "rl.teams", { n: 2 })).toBe("2v2");
    expect(translate("fr", "rl.teams", { n: 3 })).toBe("3c3");
  });
  it("returns the key itself for an unknown key", () => {
    expect(translate("fr", "nope.nope" as Key)).toBe("nope.nope");
  });
});
