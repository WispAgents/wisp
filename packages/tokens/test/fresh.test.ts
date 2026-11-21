// The generated files are committed: this fails when someone edits tokens/*.json and forgets
// `npm run build` in packages/tokens.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { render } from '../src/render.ts';
import { CSS_OUT, TS_OUT, platformOut, readTokenFiles, renderPlatforms } from '../src/build.ts';

test('dist/tokens.css and src/generated.ts match tokens/*.json (run `npm run build` in packages/tokens)', () => {
  const { css, ts } = render(readTokenFiles());
  assert.equal(readFileSync(CSS_OUT, 'utf8'), css, 'dist/tokens.css is stale');
  assert.equal(readFileSync(TS_OUT, 'utf8'), ts, 'src/generated.ts is stale');
});

test('dist/platform-<look>.css match tokens/platform/<look>.json (run `npm run build` in packages/tokens)', () => {
  for (const [look, css] of renderPlatforms()) assert.equal(readFileSync(platformOut(look), 'utf8'), css, `dist/platform-${look}.css is stale`);
});
