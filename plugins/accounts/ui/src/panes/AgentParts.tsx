// Settings → Agents' small parts (Agents.tsx): a harness's keys box, how to get one that isn't found, and its install
// guide's button.
import { createSignal, For, Show, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, CopyCommand, Dialog, Inline, Input, SettingsRow, Stack } from '@wisp/ui';
import { keyRemove, keySet } from '../commands.ts';
import { t } from '../i18n.ts';
import type { HarnessRow, KeyRow } from '../types.ts';
import { masked, site } from '../words.ts';
import '../i18n/lazy.ts';

const quiet = (p: Promise<unknown>): void => {
  p.catch(() => {}); // the shell's onError shows the rollback toast
};

/** The keys a harness signs in with (opencode Zen, Go…): set, test, remove; never the key itself. */
export function Keys(props: { readonly c: PluginClient; readonly h: HarnessRow; readonly keys: readonly KeyRow[]; readonly onClose: () => void }): JSX.Element {
  const [typed, setTyped] = createSignal<Record<string, string>>({});
  const peer = props.c.home;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && props.onClose()}
      title={t('accounts.keys.title', { name: props.h.name })}
      description={t('accounts.keys.where', { site: site(props.keys[0]?.url ?? '') })}
      part="accounts.keys"
    >
      <Stack gap="1">
        <For each={props.keys}>
          {(k) => (
            <SettingsRow
              part="accounts.key"
              title={k.name}
              description={k.set ? t('accounts.keys.saved', { hint: masked(k) }) : t('accounts.keys.none')}
              wideControl
              control={
                <Inline gap="2">
                  <Input
                    size="sm"
                    mono
                    type="password"
                    aria-label={t('accounts.keys.input', { name: k.name })}
                    placeholder={k.set ? t('accounts.keys.replace') : t('accounts.keys.paste')}
                    value={typed()[k.provider] ?? ''}
                    onInput={(v) => setTyped({ ...typed(), [k.provider]: v })}
                  />
                  <Button
                    size="sm"
                    disabled={(typed()[k.provider] ?? '').trim() === ''}
                    onClick={() => {
                      quiet(props.c.run(keySet, { peer, agent: k.agent, provider: k.provider, key: (typed()[k.provider] ?? '').trim() }));
                      setTyped({ ...typed(), [k.provider]: '' });
                    }}
                  >
                    {t('accounts.keys.save')}
                  </Button>
                  <Show when={k.set}>
                    <Button size="sm" variant="ghost" onClick={() => quiet(props.c.run(keyRemove, { peer, agent: k.agent, provider: k.provider }))}>
                      {t('accounts.keys.remove')}
                    </Button>
                  </Show>
                </Inline>
              }
            />
          )}
        </For>
      </Stack>
    </Dialog>
  );
}

/** Not found here: this OS's tip, the command to copy (v1 setup.js setupCommand), and its install guide. */
export function HowTo(props: { readonly tip: string; readonly command?: string | undefined; readonly onCopy: (cmd: string) => void }): JSX.Element {
  return (
    <>
      {props.tip}
      <Show when={props.command}>{(cmd) => <CopyCommand part="accounts.setup.command" command={cmd()} label={t('accounts.setup.copy')} onCopy={props.onCopy} />}</Show>
    </>
  );
}

/** v1 setupDocs: the install guide, a ghost button that opens it outside wisp. */
export function Docs(props: { readonly name: string; readonly url?: string | undefined }): JSX.Element {
  const shell = useShell();
  return (
    <Show when={props.url}>
      {(url) => (
        <Button size="sm" variant="ghost" icon="external" part="accounts.setup.docs" title={t('accounts.setup.guide', { name: props.name })} onClick={() => shell.platform.openExternal(url())}>
          {t('accounts.setup.docs')}
        </Button>
      )}
    </Show>
  );
}
