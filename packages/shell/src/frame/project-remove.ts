// The ask before removing a project, apart from rows-view.ts: only the Projects sheet (a lazy part) says it, so its
// strings stay in the lazy half and off the first screen (ADR-0105). DOM-free.
import '../i18n/lazy.ts';

/** The ask before removing a project (v1 projects.js: "Remove “x” from wisp? Its tasks and files are kept.", audit
 *  C2): its tasks keep running and stay in the list, and nothing on disk is touched. */
export function projectRemoveAsk(name: string, tasks: number, t: (k: string, v?: Readonly<Record<string, string | number>>) => string): { title: string; body: string; ok: string } {
  return {
    title: t('shell.projects.removeAsk.title', { name }),
    body: tasks > 1 ? t('shell.projects.removeAsk.body', { count: tasks }) : t(tasks ? 'shell.projects.removeAsk.bodyOne' : 'shell.projects.removeAsk.bodyNone'),
    ok: t('shell.projects.removeAsk.ok'),
  };
}
