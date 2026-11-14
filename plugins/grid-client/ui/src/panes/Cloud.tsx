// Settings → Cloud (v1 settings.js cloud, design/v1-reference/settings.cloud): your wisp on a server. Signed
// out: the grid's address and Sign in, then the code the browser asks for while it approves. Signed in: each
// account (ADR-0061: several grids, or several accounts on one) with Switch and Sign out, this computer on the
// current one's grid, and Add an account; then API keys and My shares (the current account's).
import { createEffect, createSignal, For, Match, Show, Switch as Pick, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { Slot } from '@wisp/shell';
import { Button, Code, Inline, Input, Link, Pill, SettingsBlock, SettingsInputRow, SettingsGroup, SettingsNote, SettingsRow, Stack, Switch, Text } from '@wisp/ui';
import { accountsOf, hostOf, type Account } from '../accounts.ts';
import { cancelSignIn, DEFAULT_GRID, removeAccount, setComputer, switchAccount } from '../commands.ts';
import { t } from '../i18n.ts';
import { ApiKeys } from './ApiKeys.tsx';
import { Confirm } from './Confirm.tsx';
import { MyShares } from './MyShares.tsx';
import { host, useCloud } from './state.ts';

export default function Cloud(props: { readonly client: PluginClient }): JSX.Element {
  const { c, peer, r, grid, setGrid, starting, waiting, failed, start, computerLine } = useCloud(props.client);
  const accounts = () => accountsOf(r());
  // Add an account: the form opens at once; a sign-in that ended (done, cancelled) folds it back (v1 cloudPane)
  const [adding, setAdding] = createSignal(false);
  createEffect(() => {
    const state = r()?.signin?.state;
    if (state === 'done' || state === 'cancelled') setAdding(false);
  });
  const [leaving, setLeaving] = createSignal<Account | undefined>();
  const signOutOf = (yes: boolean): void => {
    const a = leaving();
    setLeaving(undefined);
    if (yes && a) void c.run(removeAccount, { peer, key: a.key }).catch(() => {});
  };
  return (
    <>
      {/* v1 .sx-cloud: the accounts (or the sign-in), Add an account, and the note under them */}
      <Stack part="settings.cloud">
        <Show when={r()?.signedIn}>
          <SettingsGroup part="grid-client.account" title={t('grid-client.accounts.title')} description={t('grid-client.accounts.description')}>
            <For each={accounts()}>
              {(a) => (
                <SettingsRow
                  part="grid-client.signed-in"
                  title={a.handle}
                  description={
                    <Inline gap="2">
                      <Text tone="muted">{t('grid-client.account.on', { host: hostOf(a.grid) })}</Text>
                      <Show when={a.current}>
                        <Pill size="sm" tone="good" part="grid-client.current">
                          {t('grid-client.account.current')}
                        </Pill>
                      </Show>
                    </Inline>
                  }
                  control={
                    <Inline gap="2">
                      <Button size="sm" variant="ghost" part="grid-client.switch" disabled={a.current} onClick={() => void c.run(switchAccount, { peer, key: a.key }).catch(() => {})}>
                        {t('grid-client.account.switch')}
                      </Button>
                      <Button size="sm" variant="ghost" part="grid-client.signout" onClick={() => setLeaving(a)}>
                        {t('grid-client.signOut')}
                      </Button>
                    </Inline>
                  }
                />
              )}
            </For>
            <SettingsRow
              part="grid-client.computer"
              title={t('grid-client.computer.title')}
              description={computerLine()}
              control={<Switch aria-label={t('grid-client.computer.title')} checked={r()?.computer?.on ?? false} onChange={(on) => void c.run(setComputer, { peer, on }).catch(() => {})} />}
            />
          </SettingsGroup>
        </Show>
        <Pick>
          <Match when={waiting()}>
            {(s) => (
              <SettingsGroup part="grid-client.waiting" title={t('grid-client.waiting.title')} description={t('grid-client.waiting.description')}>
                <Stack gap="2">
                  <Text size="lg" mono part="grid-client.code">
                    {s().userCode}
                  </Text>
                  <Inline gap="2">
                    <Link href={s().verifyUrl} external>
                      {host(s().verifyUrl)}
                    </Link>
                    <Button size="sm" variant="ghost" onClick={() => void c.run(cancelSignIn, { peer, id: s().id }).catch(() => {})}>
                      {t('grid-client.cancel')}
                    </Button>
                  </Inline>
                </Stack>
              </SettingsGroup>
            )}
          </Match>
          <Match when={!r()?.signedIn || adding()}>
            <SettingsGroup part="grid-client.signin">
              {/* v1 .cg-signin: the title and its line inside the box, the address and Sign in under them */}
              <SettingsBlock
                part="settings.cloud.signin"
                title={r()?.signedIn ? t('grid-client.add') : t('grid-client.signin.title')}
                description={
                  r()?.signedIn ? (
                    t('grid-client.add.description')
                  ) : (
                    <>
                      {t('grid-client.signin.description.pre')}
                      <Code>{t('grid-client.signin.command')}</Code>
                      {t('grid-client.signin.description.post')}
                    </>
                  )
                }
              >
                <SettingsInputRow>
                  <Input mono aria-label={t('grid-client.signin.grid')} value={grid()} onInput={setGrid} placeholder={DEFAULT_GRID} />
                  <Button size="sm" variant="primary" icon="lock" part="grid-client.signin.go" disabled={starting()} onClick={start}>
                    {t('grid-client.signin.go')}
                  </Button>
                  <Show when={r()?.signedIn}>
                    <Button size="sm" variant="ghost" part="grid-client.add.cancel" onClick={() => setAdding(false)}>
                      {t('grid-client.cancel')}
                    </Button>
                  </Show>
                </SettingsInputRow>
                <Show when={failed()}>{(why) => <Text tone="bad">{why()}</Text>}</Show>
              </SettingsBlock>
            </SettingsGroup>
          </Match>
          <Match when={true}>
            <Inline>
              <Button size="sm" variant="ghost" icon="plus" part="grid-client.add" onClick={() => setAdding(true)}>
                {t('grid-client.add')}
              </Button>
            </Inline>
          </Match>
        </Pick>
        <Confirm
          open={leaving() !== undefined}
          question={t('grid-client.account.signOut.ask', { handle: leaving()?.handle ?? '', host: hostOf(leaving()?.grid ?? '') })}
          detail={t('grid-client.account.signOut.detail')}
          ok={t('grid-client.signOut')}
          danger
          onAnswer={signOutOf}
        />
        {/* the cloud computers (wisp.machines), each with its agents to sign in to (v1 .cg-list): whenever there are any */}
        <Slot input="shell.settings:cloud" layout="contents" />
        <Show when={r()?.signedIn}>
          <ApiKeys client={c} />
          <MyShares client={c} />
        </Show>
        <SettingsNote part="grid-client.note" after={10}>
          {t('grid-client.note')}
        </SettingsNote>
      </Stack>
    </>
  );
}
