// The computer's name key before its platform is known, apart from view.ts so the plugin's registration doesn't load
// the views' helpers (initial JS budget, ADR-0011).
/** The name registered before the OS is known (the pane title, the chip's bar label): never This Mac. */
export const STATIC_NAME_KEY = 'machine.name.computer';
