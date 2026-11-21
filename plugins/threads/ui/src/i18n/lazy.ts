// The strings only this package's lazy code uses (en.lazy.json), ADR-0011: they load with that code, not in the first
// page's bundle. A module that uses one imports this file for its effect, so they're in the catalogue before it runs.
import { catalogue } from '../i18n.ts';
import lazy from './en.lazy.json' with { type: 'json' };

Object.assign(catalogue as Record<string, string>, lazy);
