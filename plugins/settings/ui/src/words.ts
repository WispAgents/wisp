// A plugin's id as a heading (QA r1 #28: All settings' groups were headed "automations", "budget"): wisp's own
// without `wisp.`, sentence case, dashes and underscores as spaces.
export const ownerTitle = (owner: string): string => {
  const w = owner.replace(/^wisp\./, '').replace(/[-_]+/g, ' ').trim();
  return w.charAt(0).toUpperCase() + w.slice(1);
};

export { titleParts } from '@wisp/shell/pure';
