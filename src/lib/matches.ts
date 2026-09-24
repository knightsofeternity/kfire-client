import { translate, type Key, type Lang } from "./i18n";
import type { MatchPayload } from "./types";

export type Summary = { title: string; detail: string; highlight: boolean };

/** "il y a 12 min", "12 min. ago"; "à l'instant" under a minute. */
export function ago(iso: string, now: Date, lang: Lang): string {
  const at = new Date(iso).getTime();
  if (Number.isNaN(at)) return "";
  const s = Math.round((now.getTime() - at) / 1000);
  if (s < 60) return translate(lang, "ago.now");
  const f = new Intl.RelativeTimeFormat(lang, { numeric: "auto", style: "short" });
  if (s < 3600) return f.format(-Math.floor(s / 60), "minute");
  if (s < 86400) return f.format(-Math.floor(s / 3600), "hour");
  return f.format(-Math.floor(s / 86400), "day");
}

const EN_SUFFIX: Record<string, string> = { one: "st", two: "nd", few: "rd", other: "th" };

/** 1er / 4e, 1st / 2nd / 3rd / 8th. */
export function ordinal(n: number, lang: Lang): string {
  if (lang === "fr") return n === 1 ? "1er" : `${n}e`;
  return `${n}${EN_SUFFIX[new Intl.PluralRules("en", { type: "ordinal" }).select(n)]}`;
}

function resultLabel(result: unknown, lang: Lang): string {
  const key = `result.${String(result)}` as Key;
  const s = translate(lang, key);
  return s === key ? String(result) : s;
}

/** Hearthstone. The hero is left out: the client has no hero names, and a raw
 *  card id would read worse than nothing. */
export function hsSummary(p: MatchPayload | null, now: Date, lang: Lang): Summary | null {
  if (!p) return null;
  const bg = p.mode === "battlegrounds";
  const mode = translate(lang, bg ? "hs.mode.battlegrounds" : "hs.mode.constructed");
  const placement = typeof p.placement === "number" ? p.placement : null;
  const outcome = bg && placement ? ordinal(placement, lang) : resultLabel(p.result, lang);
  const parts = [ago(String(p.played_at ?? ""), now, lang)];
  if (typeof p.turns === "number") parts.push(translate(lang, "hs.turns", { n: p.turns }));
  return {
    title: `${mode} · ${outcome}`,
    detail: parts.filter(Boolean).join(" · "),
    highlight: bg && placement !== null ? placement <= 4 : p.result === "win",
  };
}

/** Rocket League. Team 0 is blue, 1 is orange; the member's score comes first. */
export function rlSummary(p: MatchPayload | null, now: Date, lang: Lang): Summary | null {
  if (!p) return null;
  const blue = Number(p.team_blue_score ?? 0);
  const orange = Number(p.team_orange_score ?? 0);
  const [mine, theirs] = p.player_team === 1 ? [orange, blue] : [blue, orange];
  const parts = [
    translate(lang, "rl.teams", { n: Number(p.team_size ?? 0) }),
    ago(String(p.played_at ?? ""), now, lang),
  ];
  return {
    title: `${resultLabel(p.result, lang)} · ${mine} - ${theirs}`,
    detail: parts.filter(Boolean).join(" · "),
    highlight: p.result === "win",
  };
}
