// @wisp/mobile's bundle: the phone apps' own entry (index.html → src/main.tsx), built apart from the browser's
// (apps/web) so the browser's first download never changes for it (ADR-0011). `npm run dev` here serves it with
// apps/web's mock peers (two computers: MOCK_COMPUTERS=1) and a stand-in native side (dev/preview-bridge.ts):
// http://127.0.0.1:5190/?platform=android | ios. `npm run build` writes dist/, which the Capacitor apps ship.
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import { mockPeer } from '../../apps/web/dev/vite-mock.ts';
import { moduleSideEffects } from '../../apps/web/vite.config.ts';

process.env['MOCK_COMPUTERS'] ??= '1';

export default defineConfig(({ command }) => ({
  root: fileURLToPath(new URL('.', import.meta.url)),
  plugins: [solid(), ...(command === 'serve' ? [mockPeer()] : [])],
  define: { __WISP_BUILD__: JSON.stringify(process.env['WISP_BUILD'] ?? `phone-${Date.now().toString(36)}`) },
  server: { port: Number(process.env['PORT'] ?? 5190), strictPort: false, host: '127.0.0.1' },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    modulePreload: { polyfill: false },
    rollupOptions: { treeshake: { moduleSideEffects } },
  },
}));
