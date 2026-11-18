// A declared setting's title as Settings shows it (All settings' rows, Settings' search): its label and its help, from
// the manifest's "Label: what it does". Shared so both say the same (audit A F12, F11). Pure.

/** A declared setting's title as a row shows it: manifests write "Label: what it does" (`budget.drainWatch`: "Drain
 *  watch: tell the phone when…"), so the label is the part before the first ": " outside parentheses (short, no
 *  sentence ending in it) and the help the rest, its first letter raised. A label followed by a parenthesis ("Lean
 *  Claude Code runs (no telemetry…): on, …", "…run on (task: the task's own)") keeps the parenthesis as help: a ": "
 *  inside one never splits (audit A F12). A title without that shape is all label. */
const raise = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);
const MAX = 48;
export const titleParts = (title: string): { label: string; help: string } => {
  // the first ": " and the first " (" at the top level (not inside parentheses)
  let depth = 0;
  let colon = -1;
  let open = -1;
  let close = -1;
  for (let i = 0; i < title.length; i++) {
    const ch = title[i];
    if (ch === '(') {
      if (depth === 0 && open < 0 && colon < 0 && title[i - 1] === ' ') open = i;
      depth++;
    } else if (ch === ')') {
      depth = Math.max(0, depth - 1);
      if (depth === 0 && open >= 0 && close < 0) close = i;
    } else if (depth === 0 && ch === ':' && title[i + 1] === ' ' && colon < 0) colon = i;
  }
  const sentence = (s: string): boolean => /\.(\s|$)/.test(s);
  // "Label (aside): rest" or "Label (aside)": the aside, then the rest, are the help
  if (open > 0 && close > open && (colon < 0 || colon === close + 1)) {
    const label = title.slice(0, open).trim();
    if (label.length > 0 && label.length <= MAX && !sentence(label)) {
      const aside = title.slice(open + 1, close).trim();
      const rest = colon === close + 1 ? title.slice(colon + 2).trim() : title.slice(close + 1).trim();
      return { label, help: [raise(aside), rest && raise(rest)].filter(Boolean).join('. ') };
    }
  }
  if (colon <= 0 || colon > MAX || sentence(title.slice(0, colon))) return { label: title, help: '' };
  return { label: title.slice(0, colon), help: raise(title.slice(colon + 2).trim()) };
};
