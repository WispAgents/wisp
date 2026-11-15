// The phone apps' Stage (PhoneStage.tsx), loaded apart from the first page (ADR-0011): the browser never asks for it
// (not even on idle: `preload: false`). The phone app loads it with its look (platform/styles.ts) before the first
// render, so Stage draws it in that render; one that came first draws it as the chunk lands.
import { lazyPart } from '../controls/lazy-part.ts';

export const phoneStage = lazyPart(() => import('./PhoneStage.tsx'), { preload: false });

export const loadPhoneStage = (): Promise<void> => phoneStage.load().then(() => undefined);
