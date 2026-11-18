// A plugin's id as a heading (QA r1 #28: All settings' groups were headed "automations", "budget"): wisp's own
// without `wisp.`, sentence case, dashes and underscores as spaces.
export const ownerTitle = (owner: string): string => {
  const w = owner.replace(/^wisp\./, '').replace(/[-_]+/g, ' ').trim();
  return w.charAt(0).toUpperCase() + w.slice(1);
};

/** A declared setting's title as a row shows it: manifests write "Label: what it does" (`budget.drainWatch`: "Drain
 *  watch: tell the phone when…"), so the label is the part before the first ": " (short, no full stop) and the help
 *  the rest, its first letter raised. A title without that shape is all label. */
export const titleParts = (title: string): { label: string; help: string } => {
  const i = title.indexOf(': ');
  if (i <= 0 || i > 48 || title.slice(0, i).includes('.')) return { label: title, help: '' };
  const help = title.slice(i + 2).trim();
  return { label: title.slice(0, i), help: help.charAt(0).toUpperCase() + help.slice(1) };
};
