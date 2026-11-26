// apps/web: the one bundle. `vite` (npm run dev:web) serves it with the mock peer (npm run dev: with the real server); `vite build`
// writes dist/ with content-hashed assets, which `wisp serve` serves (crates/wisp/src/serve.rs
// web_dir: WISP_WEB_DIR, else web/ by the binary, else this checkout's apps/web/dist) and the
// desktop and Android apps ship. The service worker (@wisp/shell's platform/sw.ts) builds to dist/sw.js (a fixed name: its scope
// is the site, and the page registers it by that name), in a build of its own (serviceWorker below).

import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { build as viteBuild, defineConfig, type Plugin, type ProxyOptions } from 'vite';
import solid from 'vite-plugin-solid';
import { mockPeer } from './dev/vite-mock.ts';
// the kit's class mirror of data-kit, so its rules are bucketed by class (ADR-0104, perf audit 2026-11-18 #5)
import { kitClasses } from '@wisp/ui/vite';
import { proxiedOrigin } from './dev/dev-proxy.ts';

function build(): string {
  try {
    return execFileSync('git', ['rev-parse', '--short=10', 'HEAD'], { encoding: 'utf8' }).trim();
  } catch {
    return `dev-${Date.now().toString(36)}`;
  }
}

const REPO = fileURLToPath(new URL('../../', import.meta.url)).replace(/\\/g, '/');

/** ADR-0011 budgets: our own TS modules (packages/*, plugins/*) have no import-time side effects, so a module whose
 * exports nothing uses stays out of the bundle, and one only a lazy chunk uses goes there instead of the entry.
 * (Solid's compiled components end in a top-level `delegateEvents([...])`, which rollup would otherwise keep, and
 * with it the whole module.) Stylesheets and the kit's `*styles.ts` modules, which load CSS by being imported, keep
 * theirs; so do the `i18n/lazy.ts` modules, which add a package's lazily used strings to its catalogue by being imported
 * (only from lazily loaded modules, so the strings load with them); so does everything in node_modules. */
export function moduleSideEffects(id: string): boolean {
  const path = id.replace(/\\/g, '/').replace(/\?.*$/, '');
  if (!path.startsWith(REPO)) return true;
  const rel = path.slice(REPO.length);
  if (rel.includes('node_modules/') || !/^(packages|plugins)\//.test(rel) || !/\.tsx?$/.test(rel)) return true;
  return /^packages\/ui\/src\/(shell|platform)\/[\w-]*styles\.ts$/.test(rel) || /^(packages\/shell\/src|plugins\/[\w-]+\/ui\/src)\/i18n\/lazy\.ts$/.test(rel) || rel === 'packages/shell/src/i18n/home.ts';
}

/** `npm run dev` (tools/dev.ts): WISP_DEV_API is the checkout's own `wisp serve`; /api goes there instead of the mock.
 * The edge only takes its own Host and Origin (crates/wisp-api/src/guard.rs): Host is rewritten, and Origin only for
 * this page's own (proxiedOrigin), so another local page posting through the proxy is refused as before. */
const DEV_API = process.env['WISP_DEV_API'];
export function devProxy(target: string, vitePort: number): Record<string, ProxyOptions> {
  return {
    '/api': {
      target,
      changeOrigin: true,
      configure: (proxy) =>
        void proxy.on('proxyReq', (req, from) => {
          const origin = proxiedOrigin(from.headers.origin, vitePort, target);
          if (origin !== undefined) req.setHeader('origin', origin);
        }),
    },
  };
}

const SW = fileURLToPath(new URL('../../packages/shell/src/platform/sw.ts', import.meta.url));

/** dist/sw.js, built on its own once the page's bundle is written: a classic worker script (no `import`), so nothing
 *  it shares with the page (share-form.ts: the share target's caps, which the page reads too) may become a chunk it
 *  would import. One file, every module inlined. */
export function serviceWorker(): Plugin {
  let outDir = 'dist';
  return {
    name: 'wisp-sw',
    apply: 'build',
    configResolved(c) {
      outDir = c.build.outDir;
    },
    async closeBundle() {
      await viteBuild({
        configFile: false,
        logLevel: 'warn',
        build: {
          outDir,
          emptyOutDir: false,
          target: 'es2022',
          copyPublicDir: false,
          rollupOptions: { treeshake: { moduleSideEffects }, input: { sw: SW }, output: { format: 'iife', entryFileNames: 'sw.js', inlineDynamicImports: true } },
        },
      });
    },
  };
}

export default defineConfig(({ command }) => ({
  root: fileURLToPath(new URL('.', import.meta.url)),
  plugins: [kitClasses(), solid(), serviceWorker(), ...(command === 'serve' && !DEV_API && process.env['WISP_MOCK'] !== '0' ? [mockPeer()] : [])],
  define: { __WISP_BUILD__: JSON.stringify(build()) },
  server: { port: Number(process.env['PORT'] ?? 5180), strictPort: false, host: '127.0.0.1', ...(DEV_API ? { proxy: devProxy(DEV_API, Number(process.env['PORT'] ?? 5180)) } : {}) },
  preview: { port: Number(process.env['PORT'] ?? 5181), host: '127.0.0.1' },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    modulePreload: { polyfill: false },
    // a harness's mark (plugins/*/ui/src/logo.svg) is a file beside the bundle, never a data: URL inside it: seven of
    // them were ~4 KB br of the first page's JS (budget, ADR-0011); the rest keep Vite's own rule
    assetsInlineLimit: (file: string) => (/[\\/]plugins[\\/][^\\/]+[\\/]ui[\\/]src[\\/]logo\.svg$/.test(file) ? false : undefined),
    rollupOptions: {
      treeshake: { moduleSideEffects },
      // the service worker is built apart (serviceWorker above)
      input: { index: fileURLToPath(new URL('index.html', import.meta.url)) },
      output: {
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
      },
    },
  },
}));
