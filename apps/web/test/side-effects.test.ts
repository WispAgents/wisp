// ADR-0011: the build treats our own TS modules as free of import-time side effects (vite.config.ts
// moduleSideEffects), so code only a lazy chunk uses leaves the entry. What does load something by being imported
// (stylesheets, the kit's *styles.ts) and everything outside packages/ and plugins/ keeps its side effects.
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { moduleSideEffects } from '../vite.config.ts';

const repo = fileURLToPath(new URL('../../../', import.meta.url));

describe('moduleSideEffects', () => {
  it('drops unused kit, shell and plugin TS modules', () => {
    expect(moduleSideEffects(`${repo}packages/ui/src/app/Task.tsx`)).toBe(false);
    expect(moduleSideEffects(`${repo}packages/shell/src/frame/Panes.tsx`)).toBe(false);
    expect(moduleSideEffects(`${repo}plugins/crew/ui/src/index.ts`)).toBe(false);
    expect(moduleSideEffects(`${repo}packages/ui/src/blocks/Answer.tsx?v=1`)).toBe(false);
  });

  it('keeps the modules that load CSS by being imported, and every stylesheet', () => {
    expect(moduleSideEffects(`${repo}packages/ui/src/shell/styles.ts`)).toBe(true);
    expect(moduleSideEffects(`${repo}packages/ui/src/platform/android-styles.ts`)).toBe(true);
    expect(moduleSideEffects(`${repo}packages/ui/src/platform/ios-styles.ts`)).toBe(true);
    expect(moduleSideEffects(`${repo}packages/ui/src/css/index.css`)).toBe(true);
    expect(moduleSideEffects(`${repo}packages/ui/src/css/answer.css?direct`)).toBe(true);
  });

  it('keeps the i18n/lazy.ts modules, which add a package\'s lazily used strings to its catalogue by being imported', () => {
    expect(moduleSideEffects(`${repo}packages/shell/src/i18n/lazy.ts`)).toBe(true);
    expect(moduleSideEffects(`${repo}plugins/crew/ui/src/i18n/lazy.ts`)).toBe(true);
    expect(moduleSideEffects(`${repo}plugins/shell-task/ui/src/i18n/lazy.ts?v=2`)).toBe(true);
    // only those: a plugin's other i18n modules stay droppable
    expect(moduleSideEffects(`${repo}plugins/crew/ui/src/i18n.ts`)).toBe(false);
    expect(moduleSideEffects(`${repo}plugins/shell-task/ui/src/i18n/index.ts`)).toBe(false);
  });

  it('leaves dependencies, the app itself and anything outside the checkout alone', () => {
    expect(moduleSideEffects(`${repo}node_modules/@zag-js/menu/dist/index.mjs`)).toBe(true);
    expect(moduleSideEffects(`${repo}packages/ui/node_modules/x/index.ts`)).toBe(true);
    expect(moduleSideEffects(`${repo}apps/web/src/worker.ts`)).toBe(true);
    expect(moduleSideEffects('/elsewhere/packages/ui/src/app/Task.tsx')).toBe(true);
    expect(moduleSideEffects('\0vite/preload-helper.js')).toBe(true);
  });
});
