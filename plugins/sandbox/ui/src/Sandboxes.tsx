// size-ok: one pane whose sign-in, env vars and list share the one sandbox.info answer and its optimistic writes
// Settings → Computers → Sandboxes (ADR-0047, v1 Settings → Cloud → Sandboxes): sign in to Claude once for every
// sandbox (`claude setup-token` runs here; its link opens, its code goes back; the token goes to the grid sealed,
// never kept here), the env vars every sandbox gets (names only come back), and every sandbox the grid holds of the
// user's, how many of the most it keeps, with whose it is, Open its task, Sign in to Codex there (its own device
// login, followed on its row), and Delete. The list is live: a sandbox's machine row landing or going, and its link's
// state, show as they happen; a refusal shows where it happened, never only in a toast.
import { For, Match, Show, Switch, createEffect, createMemo, createResource, createSignal, on, onCleanup, type JSX } from 'solid-js';
import { entityKey, type PeerId, type PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Callout, ForKeyed, IconButton, Inline, Input, Meter, SettingsGroup, SettingsRow, Skeleton, Stack, Text, useSignal } from '@wisp/ui';
import { Confirm, type Ask } from './Confirm.tsx';
import {
  claudeBegin,
  claudeCode,
  codexLogin,
  codexState,
  drop,
  env,
  info,
  logout,
  type ClaudeSignIn,
  type Info,
  type LoginAnswer,
  type SigninRow,
} from './commands.ts';
import { LAUNCH, LAUNCHES_TOPIC, type LaunchRow } from './launches.ts';
import { RowNotes } from './RowNotes.tsx';
import { SIGNIN } from './signin-kind.ts';
import { t } from './i18n.ts';
import {
  codexEnded,
  codexFlow,
  envKeyProblem,
  errorText,
  liveList,
  ownerTask,
  ownerWords,
  type CodexFlow,
  type Listed,
  type MachineEntity,
  type StatusEntity,
} from './model.ts';
import './i18n/lazy.ts';

/** wisp.machines' rows (its `machines` topic): a sandbox's machine and its link's state. */
const MACHINES_TOPIC = 'machines';
const PEER_ROW = 'machines_peer';
const STATUS_ROW = 'machines_status';
/** How soon a change in the list asks `sandbox.info` again (several at once ask once). */
const REFETCH_MS = 300;
/** How often a Codex sign-in there is looked at, and for how long at most (its device code lasts ~15 minutes). */
const CODEX_POLL_MS = 1000;
const CODEX_POLLS = 20 * 60;

export default function Sandboxes(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const peer = c.home as PeerId;
  const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
  // why sandbox.info failed, kept: a pending or failed answer isn't "not signed in, none yet" (UI QA r1)
  const [failed, setFailed] = createSignal<string | undefined>(undefined);
  const [data, { mutate, refetch }] = createResource(() =>
    c.run(info, { peer }).then(
      (d) => (setFailed(undefined), d),
      (e: unknown): Info | undefined => (setFailed(e instanceof Error ? e.message : String(e)), undefined),
    ),
  );
  const retryInfo = (): void => {
    setFailed(undefined);
    void refetch();
  };
  const claudeOn = (): boolean => !!data()?.logins.includes('claude');
  // the Claude sign-in: its link, then the code
  const [flow, setFlow] = createSignal<ClaudeSignIn | undefined>(undefined);
  const [code, setCode] = createSignal('');
  const [signOutError, setSignOutError] = createSignal<string | undefined>(undefined);
  // the sign-in's row: one finished in the browser (no code pasted) shows here as it happens
  const scope = c.scope.child('sandboxes-pane');
  onCleanup(() => scope.dispose());
  c.watch(peer, LAUNCHES_TOPIC, scope);
  c.watch(peer, MACHINES_TOPIC, scope);
  const live = useSignal(c.entities.row<SigninRow>(SIGNIN, entityKey(peer, 'claude')));
  createEffect(() => {
    const s = live() as SigninRow | undefined;
    if (!s || !flow()) return;
    if (s.state === 'done') {
      setFlow(undefined);
      void refetch();
    } else if (s.state === 'link' || s.state === 'failed') setFlow(s);
  });

  // the list, live: this computer's sandbox machines as their rows say now, the grid's others from `sandbox.info`;
  // `sandbox.info` is asked again (once for a burst) when one lands or goes, a launch ends, or the window comes back
  const mine = (r: { readonly id: string }): boolean => r.id.startsWith(`${peer}:`);
  const machines = useSignal(c.entities.list<MachineEntity>(PEER_ROW));
  const status = useSignal(c.entities.list<StatusEntity>(STATUS_ROW));
  const launches = useSignal(c.entities.list<LaunchRow>(LAUNCH));
  const [gone, setGone] = createSignal<ReadonlySet<string>>(new Set());
  const [rowError, setRowError] = createSignal<Readonly<Record<string, string>>>({});
  const rows = createMemo(() =>
    liveList(data()?.sandboxes ?? [], (machines() as readonly MachineEntity[]).filter(mine), (status() as readonly StatusEntity[]).filter(mine), gone()),
  );
  let again = (): void => {};
  const soon = (): void => {
    again();
    again = scope.timeout(REFETCH_MS, () => void refetch());
  };
  const boxes = createMemo(() =>
    (machines() as readonly MachineEntity[])
      .filter((m) => mine(m) && typeof m.body?.data?.['sandbox'] === 'string')
      .map((m) => m.id)
      .sort()
      .join(' '),
  );
  const ended = createMemo(() =>
    (launches() as readonly LaunchRow[])
      .filter((l) => l.state === 'started' || l.state === 'failed')
      .map((l) => `${l.id}:${l.state}`)
      .sort()
      .join(' '),
  );
  createEffect(on([boxes, ended], soon, { defer: true }));
  onCleanup(shell.platform.onResume(soon));

  const begin = (): void => {
    setFlow({ state: 'waiting' });
    void c.run(claudeBegin, { peer }).then(setFlow, (e: unknown) => setFlow({ state: 'failed', error: errorText(e) }));
  };
  const finish = (): void => {
    const v = code().trim();
    if (!v) return;
    setCode('');
    setFlow({ state: 'saving' });
    void c.run(claudeCode, { peer, code: v }).then(
      (r) => {
        setFlow(r.state === 'done' ? undefined : r);
        void refetch();
      },
      // a code the grid or Claude refused says why where it was pasted (the failed callout)
      (e: unknown) => setFlow({ state: 'failed', error: errorText(e) }),
    );
  };
  const signOut = (): void => {
    setSignOutError(undefined);
    void c.run(logout, { peer, agent: 'claude' }).then(mutate, (e: unknown) => setSignOutError(errorText(e)));
  };

  // env vars: the name shows at once, the value goes and never comes back. A name the grid would refuse says why
  // before it goes; one it refused anyway comes back into the inputs, with its words under them
  const [key, setKey] = createSignal('');
  const [value, setValue] = createSignal('');
  const [envError, setEnvError] = createSignal<string | undefined>(undefined);
  const problem = (): string | undefined => {
    const k = key().trim();
    const p = k ? envKeyProblem(k) : undefined;
    return p ? say(`sandbox.env.bad.${p}`, { name: k }) : undefined;
  };
  const withVar = (k: string, on: boolean): void => {
    const d = data();
    if (d)
      mutate({
        ...d,
        env: on ? [...new Set([...d.env, k])] : d.env.filter((x) => x !== k),
      });
  };
  const addVar = (): void => {
    const k = key().trim();
    const v = value();
    if (!k || !v || envKeyProblem(k)) return;
    const had = data()?.env.includes(k) ?? false;
    setKey('');
    setValue('');
    setEnvError(undefined);
    withVar(k, true);
    void c.run(env, { peer, key: k, value: v }).then(mutate, (e: unknown) => {
      if (!had) withVar(k, false);
      setKey(k);
      setValue(v);
      setEnvError(errorText(e));
    });
  };
  const removeVar = (k: string): void => {
    setEnvError(undefined);
    withVar(k, false);
    void c.run(env, { peer, key: k, value: '' }).then(mutate, (e: unknown) => {
      withVar(k, true);
      setEnvError(errorText(e));
    });
  };

  // Delete: the row goes at once and stays gone whatever an older answer says; refused, it comes back with why
  const remove = (s: Listed): void => {
    setGone((g) => new Set([...g, s.sandbox]));
    setRowError(({ [s.sandbox]: _, ...rest }) => rest);
    const args = s.elsewhere || !s.machine ? { peer, sandbox: s.sandbox } : { peer, machine: s.machine };
    void c.run(drop, args).then(soon, (e: unknown) => {
      setGone((g) => new Set([...g].filter((x) => x !== s.sandbox)));
      setRowError((r) => ({ ...r, [s.sandbox]: errorText(e) }));
    });
  };
  const crewName = (id: string): string =>
    (
      c.entities.list<{ id: string; name?: string; body?: { name?: string } }>('crew_member')() as readonly {
        id: string;
        name?: string;
        body?: { name?: string };
      }[]
    ).find((m) => m.id === id || m.id.endsWith(`:${id}`))?.body?.name ?? id;

  // Codex signs in on the sandbox's own wisp: the row shows it starting at once, then follows the login there until
  // its page and code are up, and on until it's done or failed (it stops with the pane)
  const [codex, setCodex] = createSignal<Readonly<Record<string, CodexFlow>>>({});
  const codexAt = (sbPeer: string, f: CodexFlow): void => {
    setCodex((all) => ({ ...all, [sbPeer]: f }));
  };
  const signInCodex = (sbPeer: string): void => {
    codexAt(sbPeer, { state: 'starting' });
    let polls = 0;
    const fail = (e: unknown): void => codexAt(sbPeer, { state: 'failed', error: errorText(e) });
    const follow = (r: LoginAnswer): void => {
      const f = codexFlow(r);
      codexAt(sbPeer, f);
      if (codexEnded(f) || ++polls > CODEX_POLLS) return;
      scope.timeout(CODEX_POLL_MS, () => void c.run(codexState, { peer: sbPeer as PeerId }).then(follow, fail));
    };
    void c.run(codexLogin, { peer: sbPeer as PeerId }).then(follow, fail);
  };

  // Delete asks first (Confirm): only its Delete drops the sandbox
  const [asking, setAsking] = createSignal<Ask | undefined>(undefined);
  const askRemove = (s: Listed): void => void setAsking({ name: s.name, owner: ownerWords(s.owner, say, crewName), run: () => remove(s) });
  const used = (): number => data()?.used ?? rows().length;
  const pane = (): JSX.Element => (
    <>
      <Text as="p" tone="muted" part="sandbox.lede">
        {t('sandbox.lede')}
      </Text>
      <Show when={data() && !data()?.enabled}>
        <Callout tone="warn" title={t('sandbox.off')} part="sandbox.off">
          {data()?.why ?? ''}
        </Callout>
      </Show>
      <SettingsGroup part="sandbox.signin" title={t('sandbox.signin')} description={t('sandbox.signin.hint')}>
        <SettingsRow
          part="sandbox.claude"
          title={t('sandbox.claude')}
          description={t(claudeOn() ? 'sandbox.claude.on' : flow()?.state === 'link' ? 'sandbox.signin.browser.hint' : 'sandbox.claude.off')}
          control={
            <Show
              when={claudeOn()}
              fallback={
                <Button
                  size="sm"
                  variant="primary"
                  loading={flow()?.state === 'waiting' || flow()?.state === 'saving'}
                  disabled={!data()?.enabled}
                  onClick={begin}
                >
                  {t('sandbox.claude.go')}
                </Button>
              }
            >
              <Button size="sm" variant="ghost" onClick={signOut}>
                {t('sandbox.claude.out')}
              </Button>
            </Show>
          }
        />
        <Show when={flow()?.state === 'link' && flow()?.link}>
          {(link) => (
            <Stack gap="2" part="sandbox.claude.flow">
              <span>
                <Button size="sm" variant="secondary" icon="external" onClick={() => void shell.platform.openExternal(link())}>
                  {t('sandbox.claude.open')}
                </Button>
              </span>
              <Inline gap="2">
                <Input
                  size="sm"
                  value={code()}
                  onInput={setCode}
                  placeholder={t('sandbox.claude.code')}
                  aria-label={t('sandbox.claude.code')}
                  onKeyDown={(e) => e.key === 'Enter' && finish()}
                />
                <Button size="sm" variant="primary" disabled={!code().trim()} onClick={finish}>
                  {t('sandbox.claude.send')}
                </Button>
              </Inline>
            </Stack>
          )}
        </Show>
        <Show when={flow()?.state === 'failed'}>
          <Callout tone="bad" title={t('sandbox.claude.failed')} part="sandbox.claude.error">
            {flow()?.error ?? ''}
          </Callout>
        </Show>
        <Show when={signOutError()}>
          {(why) => (
            <Callout tone="bad" title={t('sandbox.claude.outFailed')} part="sandbox.claude.outError">
              {why()}
            </Callout>
          )}
        </Show>
      </SettingsGroup>
      <SettingsGroup part="sandbox.env" title={t('sandbox.env')} description={t('sandbox.env.hint')}>
        <For each={data()?.env ?? []} fallback={<SettingsRow part="sandbox.env.none" title={t('sandbox.env.none')} control={<span />} />}>
          {(k) => (
            <SettingsRow
              part="sandbox.env.row"
              title={k}
              control={<IconButton icon="trash" size="sm" variant="ghost" label={say('sandbox.env.remove', { name: k })} onClick={() => removeVar(k)} />}
            />
          )}
        </For>
        <Inline gap="2" part="sandbox.env.add">
          <Input
            size="sm"
            value={key()}
            onInput={(v) => setKey(v.toUpperCase().replace(/[^A-Z0-9_]/g, '_'))}
            placeholder={t('sandbox.env.key')}
            aria-label={t('sandbox.env.key')}
          />
          <Input
            size="sm"
            type="password"
            value={value()}
            onInput={setValue}
            placeholder={t('sandbox.env.value')}
            aria-label={t('sandbox.env.value')}
            onKeyDown={(e) => e.key === 'Enter' && addVar()}
          />
          <Button size="sm" variant="secondary" disabled={!key().trim() || !value() || !!problem()} onClick={addVar}>
            {t('sandbox.env.add')}
          </Button>
        </Inline>
        <Show when={problem()}>
          {(why) => (
            <Text as="p" size="sm" tone="bad" part="sandbox.env.problem">
              {why()}
            </Text>
          )}
        </Show>
        <Show when={envError()}>
          {(why) => (
            <Callout tone="bad" title={t('sandbox.env.failed')} part="sandbox.env.error">
              {why()}
            </Callout>
          )}
        </Show>
      </SettingsGroup>
      <SettingsGroup part="sandbox.list" title={t('sandbox.list')}>
        <Show when={data()?.limit}>
          {(limit) => (
            <SettingsRow
              part="sandbox.quota"
              title={say('sandbox.quota', { used: used(), limit: limit() })}
              description={used() >= limit() ? t('sandbox.quota.full') : undefined}
              control={
                <Meter
                  part="sandbox.quota.meter"
                  value={used() / limit()}
                  label={t('sandbox.quota.label')}
                  valueText={say('sandbox.quota', {
                    used: used(),
                    limit: limit(),
                  })}
                  tone={used() >= limit() ? 'warn' : 'accent'}
                />
              }
            />
          )}
        </Show>
        <Show when={rows().length > 0} fallback={<SettingsRow part="sandbox.list.none" title={t('sandbox.list.none')} control={<span />} />}>
          <ForKeyed each={rows()} by={(x) => x.sandbox}>
            {(s) => {
              const cx = (): CodexFlow | undefined => (s().peer ? codex()[s().peer] : undefined);
              const theirs = (): boolean => !s().elsewhere && s().owner !== null && s().owner !== undefined;
              return (
                <SettingsRow
                  part="sandbox.row"
                  title={s().name}
                  description={
                    s().elsewhere
                      ? t('sandbox.list.elsewhere')
                      : s().state
                        ? `${ownerWords(s().owner, say, crewName)} · ${s().state}`
                        : ownerWords(s().owner, say, crewName)
                  }
                  control={
                    <Inline gap="1">
                      <Show when={ownerTask(s().owner)}>
                        {(task) => (
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() =>
                              c.navigate({
                                t: 'openTask',
                                id: (task().includes(':') ? task() : entityKey(s().peer as PeerId, task())) as never,
                              })
                            }
                          >
                            {t('sandbox.list.open')}
                          </Button>
                        )}
                      </Show>
                      <Show when={s().peer && !s().elsewhere}>
                        <Button size="sm" variant="ghost" loading={cx()?.state === 'starting'} onClick={() => signInCodex(s().peer)}>
                          {t('sandbox.list.codex')}
                        </Button>
                      </Show>
                      <Show
                        when={theirs()}
                        fallback={
                          // the user's own (no owner) or one only the grid holds: nothing deletes it but this
                          <Button
                            size="sm"
                            variant="ghost"
                            part="sandbox.row.delete"
                            aria-label={say('sandbox.list.remove', {
                              name: s().name,
                            })}
                            onClick={() => askRemove(s())}
                          >
                            {t('sandbox.list.delete')}
                          </Button>
                        }
                      >
                        <IconButton icon="trash" size="sm" variant="ghost" label={say('sandbox.list.remove', { name: s().name })} onClick={() => askRemove(s())} />
                      </Show>
                    </Inline>
                  }
                  below={
                    <RowNotes
                      codex={cx()}
                      error={rowError()[s().sandbox]}
                      onOpen={(url) => void shell.platform.openExternal(url)}
                      onCopy={(v) => void shell.platform.copy(v)}
                    />
                  }
                />
              );
            }}
          </ForKeyed>
        </Show>
      </SettingsGroup>
      <Confirm ask={asking()} onClose={() => setAsking(undefined)} />
    </>
  );
  // the pane once sandbox.info has answered; until then a skeleton, and why when it couldn't, with Try again
  return (
    <Switch
      fallback={
        <Stack gap="3" part="sandbox.loading">
          <Skeleton width="3/4" />
          <Skeleton shape="block" width="full" />
          <Skeleton shape="block" width="full" />
        </Stack>
      }
    >
      <Match when={failed()}>
        {(why) => (
          <Callout tone="bad" title={t('sandbox.info.failed')} part="sandbox.info.error">
            <Stack gap="2">
              <Text size="sm">{why()}</Text>
              <span>
                <Button size="sm" variant="secondary" icon="rotate" onClick={retryInfo} part="sandbox.info.retry">
                  {t('sandbox.retry')}
                </Button>
              </span>
            </Stack>
          </Callout>
        )}
      </Match>
      <Match when={data()}>{pane()}</Match>
    </Switch>
  );
}
