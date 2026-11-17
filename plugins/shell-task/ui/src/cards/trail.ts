/** The receipt's parts as they follow the token count on its line: " · 90% cache · 3% of weekly". */
export const trailWords = (parts: readonly string[]): string => (parts.length ? ` · ${parts.join(' · ')}` : '');
