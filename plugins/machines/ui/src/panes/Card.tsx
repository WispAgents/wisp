// One machine in Machines, the detail beside the list (owner's pick B, 2026-11-27; v1 machines.js card): what it is and
// where, its state and one button (Connect, Try again, Wake, Disconnect, Cancel), ⋯ (Edit, Restart / Stop wisp there,
// Remove); online: an update as a note, then what's running, its wisp version and its coding agents; connecting or
// failed: the steps; failed: what to do, with Let me in (the password once) when the key isn't allowed in yet.
import { createSignal, For, Show, type JSX } from 'solid-js';
import { Button, Callout, CopyCommand, Facts, Inline, Input, Link, Menu, Pill, SettingsRow, Stack, Steps, Text, Wrap, type Fact } from '@wisp/ui';
import type { PeerBody, PeerView, StatusBody } from '../commands.ts';
import { t } from '../i18n.ts';
import { agentsOf, kindOf, menuOf, offersLetIn, primaryOf, removeLabel, stepsOf, troubleLog, troubleTitle, updateLine, versionLine, whereOf } from '../view.ts';
import '../i18n/lazy.ts';

const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
const TAILSCALE_GUIDE = 'https://trywisp.dev/guides/connect-machines-with-tailscale.html';
const TONE: Readonly<Record<string, 'good' | 'warn' | 'bad' | 'neutral'>> = { online: 'good', connecting: 'neutral', reconnecting: 'warn', asleep: 'neutral', off: 'neutral', error: 'bad', incompatible: 'bad' };
const ICON = { ssh: 'server', grid: 'laptop', cloud: 'cloud' } as const;

export type CardAct = 'connect' | 'disconnect' | 'edit' | 'restart' | 'update' | 'stop' | 'remove';

export interface CardProps {
  readonly m: PeerBody;
  readonly s: StatusBody | undefined;
  readonly view: PeerView | undefined;
  readonly onAct: (a: CardAct) => void;
  /** the password: resolves when it went in, rejects with what to do */
  readonly onLetIn: (password: string) => Promise<void>;
  readonly onCopy: (command: string) => void;
  /** the edit form takes the place of the facts while it's open */
  readonly editing: boolean;
  readonly form: () => JSX.Element;
}

function LetIn(props: { readonly host: string; readonly command?: string; readonly onLetIn: (p: string) => Promise<void>; readonly onCopy: (c: string) => void }): JSX.Element {
  const [pw, setPw] = createSignal('');
  const [busy, setBusy] = createSignal(false);
  const [err, setErr] = createSignal('');
  const go = (): void => {
    if (!pw() || busy()) return;
    setBusy(true);
    setErr('');
    props.onLetIn(pw()).then(
      () => setPw(''),
      (e: unknown) => setErr(e instanceof Error ? e.message : String(e)),
    ).finally(() => setBusy(false));
  };
  return (
    <Stack gap="2" part="machines.letin">
      <Inline gap="2">
        <Input
          type="password"
          size="sm"
          aria-label={t('machines.letin.label', { host: props.host })}
          placeholder={t('machines.letin.placeholder', { host: props.host })}
          value={pw()}
          onInput={setPw}
          onKeyDown={(e: KeyboardEvent) => e.key === 'Enter' && go()}
        />
        <Button size="sm" variant="primary" icon="lock" disabled={busy() || !pw()} onClick={go} part="machines.letin.go">
          {busy() ? t('machines.letin.busy') : t('machines.letin.go')}
        </Button>
      </Inline>
      <Text size="sm" tone="muted">
        {t('machines.letin.never')}
      </Text>
      <Show when={err()}>{(e) => <Text tone="bad" part="machines.letin.error">{e()}</Text>}</Show>
      <Show when={props.command}>
        {(cmd) => (
          <Stack gap="1">
            <Text size="sm" tone="muted">
              {t('machines.letin.byHand')}
            </Text>
            <CopyCommand command={cmd()} label={t('machines.copy')} onCopy={props.onCopy} />
          </Stack>
        )}
      </Show>
    </Stack>
  );
}

function Trouble(props: { readonly m: PeerBody; readonly s: StatusBody; readonly view: PeerView | undefined; readonly onAct: (a: CardAct) => void; readonly onLetIn: (p: string) => Promise<void>; readonly onCopy: (c: string) => void }): JSX.Element {
  const h = () => props.s.hint ?? undefined;
  const log = (): string => troubleLog(props.s, props.view?.log ?? []);
  return (
    <Callout tone={props.s.state === 'reconnecting' ? 'warn' : 'bad'} title={troubleTitle(props.s, say)} part="machines.trouble">
      <Stack gap="2">
        <Show when={h()?.text}>{(x) => <Text size="sm">{x()}</Text>}</Show>
        <Show
          when={offersLetIn(props.s)}
          fallback={<Show when={h()?.command}>{(cmd) => <CopyCommand command={cmd()} label={t('machines.copy')} onCopy={props.onCopy} />}</Show>}
        >
          <LetIn host={props.m.address.replace(/^.*@/, '')} {...(h()?.command && { command: h()?.command as string })} onLetIn={props.onLetIn} onCopy={props.onCopy} />
        </Show>
        <Show when={h()?.url}>
          {(u) => (
            <Link href={u()} external>
              {t('machines.getNode')}
            </Link>
          )}
        </Show>
        <Show when={h()?.tailscale}>
          <Link href={TAILSCALE_GUIDE} external>
            {t('machines.tailscale.anywhere')}
          </Link>
        </Show>
        <Show when={props.s.stale}>
          <Button size="sm" variant="ghost" icon="replay" onClick={() => props.onAct('restart')}>
            {t('machines.restart')}
          </Button>
        </Show>
        <Show when={log()}>
          <Text size="sm" mono tone="muted" part="machines.log">
            {log()}
          </Text>
        </Show>
      </Stack>
    </Callout>
  );
}

/** What the list beside the detail says of a machine: its state, or how many tasks run there while online. */
export function listLine(m: PeerBody, s: StatusBody | undefined, view: PeerView | undefined): { readonly sub: string; readonly dot: 'good' | 'warn' | 'bad' | 'muted' } {
  const st = s?.state ?? 'off';
  const tone = TONE[st] ?? 'neutral';
  const dot = tone === 'neutral' ? 'muted' : tone;
  const working = view?.working ?? 0;
  if (st === 'online' && working > 0) return { sub: t('machines.working', { count: working }), dot };
  return { sub: kindOf(m) === 'grid' && st === 'asleep' ? t('machines.state.offline') : say(`machines.state.${st}`), dot };
}

export const iconOf = (m: PeerBody): 'server' | 'laptop' | 'cloud' => ICON[kindOf(m)];

/** The picked machine, beside the list (Machines, option B): its name, state and where with its one button and ⋯; an
 *  update as a note with its button; connecting or failed: the steps and what to do; then what it is, a line each. */
export function MachineDetail(props: CardProps): JSX.Element {
  const st = (): string => props.s?.state ?? 'off';
  const k = () => kindOf(props.m);
  const p = () => primaryOf(props.m, st());
  const stateText = (): string => (k() === 'grid' && st() === 'asleep' ? t('machines.state.offline') : say(`machines.state.${st()}`));
  const ready = () => (props.s?.remote?.agents ?? []).filter((a) => a.available);
  const busy = (): boolean => st() === 'connecting' || st() === 'reconnecting';
  const version = (): string => props.s?.remote?.version ?? '';
  const facts = (): Fact[] => [
    ...(st() === 'online'
      ? [
          {
            label: t('machines.fact.working'),
            value: <Text>{(props.view?.working ?? 0) > 0 ? t('machines.tasks', { count: props.view?.working ?? 0 }) : t('machines.working.none')}</Text>,
            part: 'machines.fact.working' as const,
          },
        ]
      : []),
    { label: t('machines.fact.where'), value: <Text>{whereOf(props.m, say)}</Text>, part: 'machines.fact.where' as const },
    ...(st() === 'online' && version() ? [{ label: t('machines.fact.version'), value: <Text>{versionLine(version(), say)}</Text>, part: 'machines.fact.version' as const }] : []),
    ...(st() === 'online' && agentsOf(props.s).kind !== 'unknown'
      ? [
          {
            label: t('machines.fact.agents'),
            value: (
              <Show when={ready().length > 0} fallback={<Text tone="bad">{t('machines.noAgents')}</Text>}>
                <Wrap gap="1">
                  <For each={ready()}>{(a) => <Pill size="sm">{a.name || a.id}</Pill>}</For>
                </Wrap>
              </Show>
            ),
            part: 'machines.agents' as const,
          },
        ]
      : []),
    ...(k() === 'ssh'
      ? [{ label: t('machines.fact.connects'), value: <Text>{props.m.autoConnect === false ? t('machines.connects.manual') : t('machines.connects.auto')}</Text>, part: 'machines.fact.connects' as const }]
      : []),
  ];
  return (
    <Stack gap="4" part="machines.detail">
      <SettingsRow
        part="machines.row"
        big
        tile={ICON[k()]}
        title={props.m.name}
        description={
          <Inline gap="2">
            <Pill tone={TONE[st()] ?? 'neutral'} dot live={busy()} variant="plain">
              {stateText()}
            </Pill>
            <Text size="sm" tone="muted">
              {`· ${say(`machines.kind.${k()}`)}`}
            </Text>
          </Inline>
        }
        control={
          <Inline gap="2">
            <Button size="sm" variant={p().primary ? 'primary' : 'secondary'} onClick={() => props.onAct(p().act)} part="machines.row.go">
              {say(p().label)}
            </Button>
            <Menu
              trigger={{ icon: 'dots', 'aria-label': t('machines.more', { name: props.m.name }), variant: 'ghost', size: 'sm' }}
              items={menuOf(props.m, st()).flatMap((a) => [
                ...(a === 'remove' ? [{ kind: 'separator' as const }] : []),
                {
                  value: a,
                  label: a === 'remove' ? say(removeLabel(props.m)) : say(`machines.menu.${a}`),
                  icon: ({ edit: 'pen', restart: 'replay', stop: 'stop', remove: 'trash' } as const)[a],
                  ...((a === 'remove' || a === 'stop') && { danger: true }),
                },
              ])}
              onSelect={(v) => props.onAct(v as CardAct)}
            />
          </Inline>
        }
      />
      <Show when={st() === 'online' && props.view?.outdated}>
        {(v) => (
          // the button under the words, not beside them: beside, a phone squeezed the words one a line (QA, 390 px)
          <Callout tone="warn" icon="replay" part="machines.update">
            <Stack gap="2">
              <Text size="sm">{t('machines.update.note', { version: v(), current: version() || '?' })}</Text>
              <Inline>
                <Button size="sm" variant="secondary" title={t('machines.update.title', { name: props.m.name, version: v() })} onClick={() => props.onAct('update')} part="machines.update.go">
                  {updateLine(version(), v(), say)}
                </Button>
              </Inline>
            </Stack>
          </Callout>
        )}
      </Show>
      <Show when={(busy() || st() === 'error') && (props.s?.steps?.length ?? 0) > 0}>
        <Steps items={stepsOf(props.s?.steps)} part="machines.steps" />
      </Show>
      <Show when={(st() === 'error' || st() === 'reconnecting') && props.s?.error ? props.s : undefined}>
        {(s) => <Trouble m={props.m} s={s()} view={props.view} onAct={props.onAct} onLetIn={props.onLetIn} onCopy={props.onCopy} />}
      </Show>
      <Show when={props.editing} fallback={<Facts items={facts()} part="machines.facts" />}>
        {props.form()}
      </Show>
    </Stack>
  );
}
