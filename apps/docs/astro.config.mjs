// trywisp.dev/docs (ADR-0039): Starlight over docs/site, the reference generated from the code (scripts/reference.mjs),
// every page also as markdown with llms.txt (scripts/llms.mjs). Served from GitHub Pages under the site (pages.yml).
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://trywisp.dev',
  base: '/docs',
  trailingSlash: 'always',
  // the CSS goes in each page: no stylesheet request holds the first paint (about 200 ms on a slow phone)
  build: { inlineStylesheets: 'always' },
  integrations: [
    starlight({
      title: 'wisp docs',
      description: 'Manage the code your agents write: run Claude Code, Codex and every coding agent from the app, the TUI or the CLI; declare them as code; track every line they write.',
      // every page: the site's card for links shared on X, Slack, LinkedIn…; the page's own title and description
      // come from Starlight (og:title, og:description, canonical)
      head: [
        { tag: 'meta', attrs: { property: 'og:image', content: 'https://trywisp.dev/assets/og.png' } },
        { tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
        { tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
        { tag: 'meta', attrs: { property: 'og:image:alt', content: 'wisp: manage the code your agents write' } },
        { tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
        { tag: 'meta', attrs: { name: 'twitter:site', content: '@trywispagents' } },
        { tag: 'meta', attrs: { name: 'twitter:image', content: 'https://trywisp.dev/assets/og.png' } },
        { tag: 'link', attrs: { rel: 'alternate', type: 'text/plain', href: '/docs/llms.txt', title: 'llms.txt' } },
      ],
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
            { label: 'Overview', link: '/guides/' },
            { label: 'Start with your agent', collapsed: true, items: [{ autogenerate: { directory: 'guides/agents' } }] },
            { label: 'Apps', items: [{ autogenerate: { directory: 'guides/apps' } }] },
            { label: 'Declare', items: [{ autogenerate: { directory: 'guides/declare' } }] },
            { label: 'Run', items: [{ autogenerate: { directory: 'guides/run' } }] },
            { label: 'Track', items: [{ autogenerate: { directory: 'guides/track' } }] },
          ],
        },
        { label: 'Integrations', items: [{ autogenerate: { directory: 'integrations' } }] },
        { label: 'Build plugins', items: [{ autogenerate: { directory: 'plugins' } }] },
        { label: 'Reference', items: [{ autogenerate: { directory: 'reference' } }] },
        { label: 'Import from an earlier wisp', items: [{ autogenerate: { directory: 'migrate' } }] },
      ],
    }),
  ],
});
