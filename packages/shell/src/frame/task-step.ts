// The task page's next / previous task on phones (ADR-0065 step 13, design/mobile.md §A.4: a swipe on the head, its
// twins Next task / Previous task in the page's menu): the neighbours in the list's order, the order the list shows
// now (`ShellContext.listOrder`: its filter, its folded sections, its sort). No wrap at either end: the first task has
// no previous, the last no next. A task that left the list meanwhile (deleted on another device, filtered out, archived)
// keeps its place: the task that moved into its slot is next, the one before it previous. Pure.

export interface TaskSteps {
  readonly next?: string;
  readonly previous?: string;
}

/** `at`: where the task last sat in the order (undefined: never seen there, so no neighbours). */
export function taskSteps(order: readonly string[], key: string, at: number | undefined): TaskSteps {
  const i = order.indexOf(key);
  if (i >= 0) return pick(order[i + 1], order[i - 1]);
  if (at === undefined || at < 0) return {};
  return pick(order[at], at > 0 ? order[Math.min(at, order.length) - 1] : undefined);
}

/** Where the task sits now, else where it last sat. */
export const placeIn = (order: readonly string[], key: string, at: number | undefined): number | undefined => {
  const i = order.indexOf(key);
  return i >= 0 ? i : at;
};

const pick = (next: string | undefined, previous: string | undefined): TaskSteps => ({ ...(next !== undefined && { next }), ...(previous !== undefined && { previous }) });
