// The overlays' phone halves (ADR-0065 build step 5, design/mobile.md §A.7), loaded apart: the browser never fetches
// them (not even on idle: `preload: false`), so its first page and its preload stay what they were (ADR-0011,
// ADR-0067). The phone apps load them with their look's stylesheet (platform/styles.ts), and a control's first press
// asks too, so a press almost always finds them there; one that doesn't answers in the same frame (the trigger
// pressed, the sheet's surface already drawn) and gains its phone behaviour as the chunk lands.
import { lazyPart } from './lazy-part.ts';

export const phoneOverlays = lazyPart(() => import('./phone.tsx'), { preload: false });
