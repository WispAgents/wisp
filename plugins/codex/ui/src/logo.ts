// Codex's mark (v1 web/logos/codex-color.svg): the plugin carries its own image, a file the build emits beside the bundle (vite.config.ts never inlines it), so the marks stay out of the first page's JS (budget, ADR-0011).
export const LOGO = new URL('./logo.svg', import.meta.url).href;
