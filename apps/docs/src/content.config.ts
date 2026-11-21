// The pages live next to the code they explain (ADR-0039: docs/site, reviewed in the feature's PR); the generated
// reference lands in docs/site/reference (git-ignored, made by scripts/reference.mjs before every dev and build).
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

export const collections = {
  docs: defineCollection({ loader: glob({ pattern: '**/*.{md,mdx}', base: '../../docs/site' }), schema: docsSchema() }),
};
