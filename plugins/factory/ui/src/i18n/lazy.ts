// The strings only this package's lazy code uses (en.lazy.json, ADR-0011): they load with the band, not in the first
// page's bundle. A module that uses one imports this file for its effect.
import { catalogue } from '../i18n.ts';
import lazy from './en.lazy.json' with { type: 'json' };

Object.assign(catalogue as Record<string, string>, lazy);
