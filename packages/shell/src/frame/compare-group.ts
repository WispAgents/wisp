// The runs of one ask for the Compare dialog (F-comp-30), found from the run it was opened on. The dialog's id is an
// entity key (`<peer>:<id>`, from the overlay URL) while `entities.list` rows carry bare ids on a real peer (keyed in
// the mock): both resolve, and the rows come back keyed, as navigation, watches and commands need (audit B #3). Pure.
import { sessionKey } from './rows-view.ts';
import { runOrder } from './trial.ts';

const bare = (id: string): string => id.slice(id.indexOf(':') + 1);

interface GroupRow {
  readonly id: string;
  readonly created_at: number;
  readonly archived: boolean;
  readonly summary: Readonly<Record<string, unknown>>;
}

// one keyed copy per store row: an unchanged run is the same object next time, so its column doesn't re-render
const keyed = new WeakMap<object, GroupRow>();
function withKey<R extends GroupRow>(s: R, key: string): R {
  if (s.id === key) return s;
  const had = keyed.get(s);
  if (had?.id === key) return had as R;
  const out = { ...s, id: key };
  keyed.set(s, out);
  return out;
}

/** the unarchived runs sharing the opened run's `groupId`, in run order, each with its entity key as `id` */
export function compareGroup<R extends GroupRow>(rows: readonly R[], keys: readonly string[], id: string): R[] {
  const want = bare(id);
  const g = rows.find((s) => bare(s.id) === want)?.summary['groupId'];
  if (typeof g !== 'string' || !g) return [];
  return runOrder(rows.filter((s) => s.summary['groupId'] === g && !s.archived)).map((s) => withKey(s, sessionKey(keys, s.id)));
}
