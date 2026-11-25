// Settings → Agents' small parts, shared by the list's All harnesses and each harness's detail (v1 settings.js
// paintAgents, agent-login.js): a sign-in's control (Sign in, Cancel, Signed in · Sign out), the keys a key-based
// harness signs in with, how to get one that's missing, and its install guide.
import { createSignal, For, Match, Show, Switch as Pick, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, CopyCommand, Dialog, Inline, Input, Pill, SettingsRow, Stack } from '@wisp/ui';
import { keyRemove, keySet, loginCancel, loginStart, logout } from '../commands.ts';
import { t } from '../i18n.ts';
import type { HarnessRow, KeyRow, LoginRow } from '../types.ts';
import { loginControl, masked, site } from '../words.ts';
import { quiet, type AgentsData } from './agents-data.ts';
import '../i18n/lazy.ts';

/** `row`: the harness's (or GitHub's) row as it is now: its id, and whether it's known signed out (it updates in place
 *  as its detect settles) */
export function SignControl(props: { readonly d: AgentsData; readonly row: () => { readonly id: string; readonly signedIn?: boolean | null | undefined }; readonly account: string; readonly isAccount: boolean }): JSX.Element {
  const d = props.d;
  // no sign-in yet: what the row knows (GitHub's row says gh is signed in before any sign-in from here)
  const known = (): LoginRow | undefined => d.login(props.row().id, props.account) ?? (props.row().signedIn === true ? { agent: props.row().id, account: props.account, state: 'idle', signedIn: true } : undefined);
  const ctl = (): ReturnType<typeof loginControl> => loginControl(known(), { account: props.isAccount, out: props.row().signedIn === false });
  const args = (): { peer: typeof d.peer; agent: string; account: string } => ({ peer: d.peer, agent: props.row().id, account: props.account });
  return (
    <Pick>
      <Match when={ctl() === 'cancel'}>
        <Button size="sm" variant="ghost" onClick={() => quiet(d.c.run(loginCancel, args()))}>
          {t('accounts.login.cancel')}
        </Button>
      </Match>
      <Match when={ctl() === 'signIn'}>
        <Button size="sm" onClick={() => quiet(d.c.run(loginStart, args()))}>
          {t('accounts.login.signIn')}
        </Button>
      </Match>
      <Match when={ctl() === 'signedIn'}>
        <Inline gap="3">
          <Pill tone="good" icon="check" variant="plain">
            {t('accounts.login.signedIn')}
          </Pill>
          <Button size="xs" variant="ghost" onClick={() => quiet(d.c.run(logout, args()))}>
            {t('accounts.login.signOut')}
          </Button>
        </Inline>
      </Match>
    </Pick>
  );
}

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
