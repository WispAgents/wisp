// What Auto will run (./AutoPop.tsx) opens on a press: it loads apart from the first page (ADR-0011), fetched once the
// first screen is up (../controls/lazy-part.ts preloadControls).
import { lazyPart, lazyView } from '../controls/lazy-part.ts';

export const AutoPop = lazyView(
  lazyPart(() => import('./AutoPop.tsx')),
  (m) => m.AutoPop,
);
