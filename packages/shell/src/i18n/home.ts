// The strings only Home's compose box uses (New task, its pickers, setup and trial, en.home.json), ADR-0011: on a
// desktop Home is the first screen and they load with it, as before; on the phone (the task list first) they load
// when New task does, not in the first page's bundle. A module that uses one imports this file for its effect (the
// build keeps it: vite.config.ts moduleSideEffects), as ./lazy.ts does for en.lazy.json.
import { SHELL_STRINGS } from './en.ts';
import home from './en.home.json' with { type: 'json' };

Object.assign(SHELL_STRINGS as Record<string, string>, home);
