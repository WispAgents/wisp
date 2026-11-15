// Settings' phone pages (SettingsPhone.tsx), loaded apart from the first page (ADR-0011): the browser never asks for them
// (not even on idle: `preload: false`). The phone apps load them with the phone frame (app/PhoneStage.tsx, loaded with the
// look), so opening Settings from the me menu draws the platform's page in its first frame.
import { lazyPart } from '../controls/lazy-part.ts';

export const settingsPhone = lazyPart(() => import('./SettingsPhone.tsx'), { preload: false });
