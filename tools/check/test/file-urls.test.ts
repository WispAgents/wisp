import assert from 'node:assert/strict';
import test from 'node:test';
import { fileUrls } from '../rules/file-urls.ts';
import { fixture, run } from './helpers.ts';

test('file-urls: a_url_pathname_or_a_glued_file_url_used_for_a_path_fails_on_every_os', async () => {
  assert.deepEqual(await run(fileUrls, fixture('file-urls')), [
    'packages/a/test/bad.test.ts:2 file-urls', // a URL's .pathname read as a file
    'packages/a/test/bad.test.ts:3 file-urls',
    'qa/run.mjs:1 file-urls', // a template-glued file URL: never the entry on Windows
    'qa/run.mjs:2 file-urls', // a concatenated file URL from a D:\ path
    'qa/run.mjs:3 file-urls', // three slashes is still D:\ backslashes inside a URL
  ]);
});
