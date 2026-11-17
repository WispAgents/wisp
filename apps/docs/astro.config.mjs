// trywisp.dev/docs (ADR-0039): Starlight over docs/site, the reference generated from the code (scripts/reference.mjs),
// every page also as markdown with llms.txt (scripts/llms.mjs). Served from GitHub Pages under the site (pages.yml).
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://trywisp.dev',
  base: '/docs',
  trailingSlash: 'always',
  integrations: [
    starlight({
      title: 'wisp docs',
      description: 'Your coding agents, as code. Declare them, run them anywhere, track every line they write.',
      logo: { src: './src/assets/wisp.svg', replacesTitle: false },
      favicon: '/favicon.svg',
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/WispAgents/wisp' }],
      editLink: { baseUrl: 'https://github.com/WispAgents/wisp/edit/main/docs/site/' },
      lastUpdated: true,
      customCss: ['./src/styles/wisp.css'],
      sidebar: [
        { label: 'Get started', items: [{ autogenerate: { directory: 'start' } }] },
        { label: 'Concepts', items: [{ autogenerate: { directory: 'concepts' } }] },
        {
          label: 'Guides',
          items: [
            { label: 'Declare', items: [{ autogenerate: { directory: 'guides/declare' } }] },
            { label: 'Run', items: [{ autogenerate: { directory: 'guides/run' } }] },
            { label: 'Track', items: [{ autogenerate: { directory: 'guides/track' } }] },
          ],
        },
        { label: 'Integrations', items: [{ autogenerate: { directory: 'integrations' } }] },
        { label: 'Build plugins', items: [{ autogenerate: { directory: 'plugins' } }] },
        { label: 'Reference', items: [{ autogenerate: { directory: 'reference' } }] },
        { label: 'Migrate from v1', items: [{ autogenerate: { directory: 'migrate' } }] },
      ],
    }),
  ],
});
