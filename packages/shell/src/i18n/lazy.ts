// The strings only this package's lazy code uses (en.lazy.json), ADR-0011: they load with that code, not in the first
// page's bundle. A module that uses one imports this file for its effect (the build keeps it: vite.config.ts
// moduleSideEffects), so they're in the catalogue the client reads before it runs. Which keys go here is the build's to
// say: a key only lazily loaded modules of this package use.
import { SHELL_STRINGS } from './en.ts';
import lazy from './en.lazy.json' with { type: 'json' };

Object.assign(SHELL_STRINGS as Record<string, string>, lazy);
