// Grok's mark (v1 web/logos/grok.svg, vendored from @lobehub/icons): the plugin carries its own image, a file the build emits beside the bundle (vite.config.ts never inlines it), so the marks stay out of the first page's JS (budget, ADR-0011). A mono mark: dark on a light OS, light on a dark one (v1 drew it in the text colour).
// `#mono`: a one-colour mark, drawn in the app theme's ink (packages/ui/src/css/reset.css; WebKit picks an <img> SVG's
// scheme from the OS, not the page)
export const LOGO = `${new URL('./logo.svg', import.meta.url).href}#mono`;
