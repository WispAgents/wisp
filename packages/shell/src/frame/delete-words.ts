// What Delete task says it does (audit G13): a factory run's stations' tasks, and their clean worktrees, go with it.
// Pure; only the delete dialog reads it.
import { runFactory } from './run-factory.ts';
import '../i18n/lazy.ts';

export function deleteWords(summary: { readonly factory?: unknown } | undefined): 'shell.delete.body' | 'shell.delete.bodyFactory' {
  return runFactory(summary) === undefined ? 'shell.delete.body' : 'shell.delete.bodyFactory';
}
