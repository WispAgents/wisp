// The strings the first page carries (one): the band's own load with it (i18n/lazy.ts). Apart from i18n.ts so the index
// holds no lookup code (ADR-0011, ADR-0105: the factory UI stays off the desktop's first screen).
import en from './i18n/en.json' with { type: 'json' };

export const catalogue: Readonly<Record<string, string>> = en;
