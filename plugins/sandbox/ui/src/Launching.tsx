// A `/sandbox` launch's page (ADR-0047; the owner's 2026-11-25 report): the task's first line with the Sandbox pill
// from the first frame, and its launch row as it goes: a glowing grid while the sandbox is made, started and gets the
// project, step by step; Claude's sign-in for sandboxes right here when it's missing (Sign in → the browser → this
// task goes on by itself, no trip to Settings); or why it didn't start, with Try again. Started: the task opens.
import { Show, createEffect, createMemo, createSignal, onCleanup, type JSX } from 'solid-js';
import { entityKey, type PeerId, type PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Callout, GlowGrid, Icon, Inline, Input, Pill, Stack, Steps, Text, useSignal, type Step } from '@wisp/ui';
import { claudeBegin, claudeCode, dismiss, retry, type SigninRow } from './commands.ts';
import { t } from './i18n.ts';
import { LAUNCH, LAUNCHES_TOPIC, SIGNIN, type LaunchRow, type LaunchStep } from './launches.ts';
import type { LaunchingParams } from './views.ts';
import './i18n/lazy.ts';

const STEPS: readonly LaunchStep[] = ['make', 'wake', 'copy', 'start'];

/** Each step done, being done or still to come, from where the launch is. */
export function stepsOf(row: LaunchRow | undefined): Step[] {
  const at = row ? STEPS.indexOf(row.step as LaunchStep) : 0;
  const failed = row?.state === 'failed';
  return STEPS.map((s, i) => ({
    title: t(`sandbox.step.${s}`),
    status: i < at ? 'done' : i === at ? (failed ? 'failed' : 'current') : 'pending',
  }));
}

export default function Launching(props: { readonly client: PluginClient; readonly params: LaunchingParams }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const peer = c.home as PeerId;
  const scope = c.scope.child('sandbox-launching');
  onCleanup(() => scope.dispose());
  c.watch(peer, LAUNCHES_TOPIC, scope);
  const row = useSignal(c.entities.row<LaunchRow>(LAUNCH, entityKey(peer, props.params.id)));
  const launch = (): LaunchRow | undefined => row() as LaunchRow | undefined;
  const signinRow = useSignal(c.entities.row<SigninRow>(SIGNIN, entityKey(peer, 'claude')));
  const signin = (): SigninRow | undefined => signinRow() as SigninRow | undefined;
  const title = (): string => launch()?.title || props.params.prompt?.split('\n').find((l) => l.trim())?.trim().slice(0, 80) || '';
  const state = createMemo(() => launch()?.state ?? (props.params.prompt ? 'preparing' : 'gone'));

  // started: the task itself, once
  let opened = false;
  createEffect(() => {
    const l = launch();
    if (!opened && l?.state === 'started' && l.peer && l.task) {
      opened = true;
      c.navigate({ t: 'openTask', id: entityKey(l.peer as PeerId, l.task) as never });
    }
  });

  // the sign-in: Sign in opens Claude's page in the browser; the browser finishing it moves the launch on (server side)
  const [starting, setStarting] = createSignal(false);
  const [code, setCode] = createSignal('');
  const [sending, setSending] = createSignal(false);
  const signIn = (): void => {
    setStarting(true);
    void c.run(claudeBegin, { peer }).then(
      // `claude setup-token` opens the browser itself; Open the page again is there when it didn't
      () => setStarting(false),
      (e: unknown) => {
        setStarting(false);
        shell.toast.show(e instanceof Error ? e.message : String(e));
      },
    );
  };
  const sendCode = (): void => {
    if (!code().trim()) return;
    setSending(true);
    void c.run(claudeCode, { peer, code: code() }).finally(() => setSending(false));
  };
  const inBrowser = (): boolean => signin()?.state === 'link' || signin()?.state === 'waiting' || signin()?.state === 'saving';
  const again = (): void => void c.run(retry, { peer, id: props.params.id });
  const drop = (): void => {
    void c.run(dismiss, { peer, id: props.params.id });
    c.navigate({ t: 'home' });
  };

  const glow = (): 'working' | 'waiting' | 'failed' => (state() === 'failed' ? 'failed' : state() === 'signin' ? 'waiting' : 'working');
  return (
    <Stack gap="5" pad="6" part="sandbox.launching">
      <Inline gap="2">
        <Text weight="semibold" size="lg" truncate>
          {title()}
        </Text>
        <Pill tone="neutral" variant="outline" part="sandbox.pill">
          <Inline gap="1">
            <Icon name="box" size="sm" />
            {t('sandbox.pill')}
          </Inline>
        </Pill>
      </Inline>
      <Show
        when={state() !== 'gone'}
        fallback={
          <Stack gap="3">
            <Text tone="muted">{t('sandbox.gone')}</Text>
            <span>
              <Button size="sm" variant="secondary" icon="back" onClick={() => c.navigate({ t: 'home' })}>
                {t('sandbox.back')}
              </Button>
            </span>
          </Stack>
        }
      >
        <GlowGrid state={glow()} part="sandbox.launching.grid">
          <Show when={state() === 'preparing' || state() === 'started'}>
            <Icon name="box" size="lg" />
            <Text weight="semibold" size="md">
              {t('sandbox.preparing')}
            </Text>
            <Text size="sm" tone="muted" align="center">
              {t('sandbox.preparing.hint')}
            </Text>
          </Show>
          <Show when={state() === 'signin'}>
            <Icon name="lock" size="lg" />
            <Text weight="semibold" size="md">
              {inBrowser() ? t('sandbox.signin.browser') : t('sandbox.signin.title')}
            </Text>
            <Text size="sm" tone="muted" align="center">
              {inBrowser() ? t('sandbox.signin.browser.hint') : t('sandbox.signin.lede')}
            </Text>
            <Show
              when={inBrowser()}
              fallback={
                <Button variant="primary" icon="lock" loading={starting()} onClick={signIn} part="sandbox.launching.signin">
                  {t('sandbox.signin.go')}
                </Button>
              }
            >
              <Show when={signin()?.state === 'saving'} fallback={
                <Show when={signin()?.link}>
                  {(link) => (
                    <Button size="sm" variant="secondary" icon="web" onClick={() => void shell.platform.openExternal(link())}>
                      {t('sandbox.signin.reopen')}
                    </Button>
                  )}
                </Show>
              }>
                <Button size="sm" variant="secondary" loading>
                  {t('sandbox.claude.saving')}
                </Button>
              </Show>
            </Show>
          </Show>
          <Show when={state() === 'failed'}>
            <Icon name="warn" size="lg" />
            <Text weight="semibold" size="md">
              {t('sandbox.failed')}
            </Text>
          </Show>
        </GlowGrid>
        <Show when={state() === 'signin' && signin()?.state === 'link'}>
          <Inline gap="2" part="sandbox.launching.code">
            <Input size="sm" value={code()} onInput={setCode} placeholder={t('sandbox.signin.code')} aria-label={t('sandbox.signin.code')} onKeyDown={(e) => e.key === 'Enter' && sendCode()} />
            <Button size="sm" variant="secondary" disabled={!code().trim()} loading={sending()} onClick={sendCode}>
              {t('sandbox.claude.send')}
            </Button>
          </Inline>
        </Show>
        <Show when={signin()?.state === 'failed' && state() === 'signin'}>
          <Callout tone="bad" title={t('sandbox.claude.failed')} part="sandbox.launching.signin-error">
            {signin()?.error ?? ''}
          </Callout>
        </Show>
        <Show when={state() === 'preparing' || state() === 'started' || state() === 'failed'}>
          <Steps items={stepsOf(launch())} part="sandbox.launching.steps" />
        </Show>
        <Show when={state() === 'failed'}>
          <Callout tone="bad" title={launch()?.error ?? t('sandbox.failed')} part="sandbox.launching.error" />
          <Inline gap="2">
            <Button size="sm" variant="primary" icon="rotate" onClick={again}>
              {t('sandbox.retry')}
            </Button>
            <Button size="sm" variant="ghost" onClick={drop}>
              {t('sandbox.dismiss')}
            </Button>
          </Inline>
        </Show>
      </Show>
    </Stack>
  );
}
