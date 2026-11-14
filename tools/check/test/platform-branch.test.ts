import assert from 'node:assert/strict';
import test from 'node:test';
import { platformBranch } from '../rules/platform-branch.ts';
import { fixture, messages, run } from './helpers.ts';

// ADR-0065 edge case: a feature or plugin UI reading `data-platform` or `platform.kind === 'ios'` fails `npm run check`.
test('a_feature_reading_the_platform_fails', async () => {
  const root = fixture('platform-branch');
  assert.deepEqual(await run(platformBranch, root), [
    'apps/web/src/phone.css:1 platform-branch', // a stylesheet outside the kit picks a platform
    'packages/shell/src/frame/answer.ts:1 platform-branch', // the shell outside its platform module
    'plugins/list/ui/src/index.ts:1 platform-branch', // 'ios' == platform.kind, either side, loose equality
    'plugins/list/ui/src/index.ts:13 platform-branch', // the attribute in a template string
    'plugins/list/ui/src/index.ts:2 platform-branch', // dataset.platform
    'plugins/list/ui/src/index.ts:7 platform-branch', // case 'android' in a switch on platform.kind
    'plugins/list/ui/src/more.tsx:1 platform-branch', // the kit's usePlatform, even renamed
    'plugins/list/ui/src/more.tsx:2 platform-branch', // ['android', 'ios'].includes(platform.kind)
    'plugins/list/ui/src/more.tsx:5 platform-branch', // a destructured kind
    'plugins/list/ui/src/more.tsx:7 platform-branch', // a JSX attribute
  ]);
  // the kit, the tokens, the shell's platform module, tests, platform.kind === 'desktop' and a machine's own OS pass
  const all = (await messages(platformBranch, root)).join('\n');
  assert.match(all, /usePlatform\(\)/);
});
