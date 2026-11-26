// Ship it (v1 ship.js shipDialog), an overlay view: it opens on the click in its final shape (the
// route, the fields, the button), guessed from the row; what would ship (git and gh, a second or
// so) fills it in place. Confirm closes it at once and the row follows the stages. A failed ship's
// words come back here on Retry.

import { createSignal, For, Match, onCleanup, Show, Switch, type JSX } from 'solid-js';
import { entityKey, type EntityKey, type PluginClient } from '@wisp/client';
import { Button, Checkbox, Field, Icon, Inline, Input, Skeleton, Stack, Text, TextArea, useSignal } from '@wisp/ui';
import { prepare, write } from './commands.ts';
import { flowsFor } from './flows.ts';
import { t } from './i18n.ts';
import { unsentLine } from './model/merge.ts';
import { footLine, goLabel, infoGuess, routeFacts, shipBlock, stepsFor, type ShipGuess } from './model/ship.ts';
import { stewardArg, stewardStart } from './model/steward.ts';
import { useShell } from './shell.ts';
import { useUnsent, withUnsent } from './unsent.ts';
import type { ShipInfo, ShipRepo } from './types.ts';
import { SHIP_VIEW, useSession } from './use.ts';
import './i18n/lazy.ts';

export { parseShipParams, type ShipParams } from './params.ts';
import type { ShipParams } from './params.ts';

function Route(props: { readonly r?: ShipRepo; readonly branch?: string; readonly several: boolean }): JSX.Element {
  return (
    <Stack gap="1" part="ship.route">
      <Inline gap="3">
        <Icon name="branch" size="sm" tone="muted" />
        <Show when={props.several && props.r?.repo}>
          {(repo) => (
            <Text weight="semibold" nowrap>
              {repo()}
            </Text>
          )}
        </Show>
        <Text mono truncate title={props.r?.branch ?? props.branch ?? ''}>
          {props.r?.branch ?? props.branch ?? ''}
        </Text>
        <Icon name="arrow" size="sm" tone="faint" />
        <Show when={props.r} fallback={<Skeleton shape="line" width="1/4" />}>
          {(r) => <Text mono truncate>{`${r().remote || t('wisp.ship.dialog.noRemoteName')}/${r().base}`}</Text>}
        </Show>
      </Inline>
      <Show when={props.r} fallback={<Skeleton shape="line" width="1/2" />}>
        {(r) => (
          <Text size="sm" tone="muted" truncate>
            {routeFacts(t, r(), props.several)}
          </Text>
        )}
      </Show>
    </Stack>
  );
}

// size-ok: one form; each field is a line, and the loading shape mirrors it
export default function ShipDialog(props: { readonly client: PluginClient; readonly params: ShipParams }): JSX.Element {
  const shell = useShell();
  const key = props.params.key;
  const flows = flowsFor(props.client, () => shell, key);
  const s = useSession(props.client, () => key);
  const unsent = useUnsent();
  const typed = flows.run()?.error !== undefined ? flows.run()?.values : undefined; // a failed ship's words, back for Retry
  const [info, setInfo] = createSignal<ShipInfo>();
  const [title, setTitle] = createSignal('');
  const [body, setBody] = createSignal('');
  const [draft, setDraft] = createSignal(typed?.draft ?? false);
  const [saveTask, setSaveTask] = createSignal(typed?.saveTask ?? false);
  // Keep it green (ADR-0132): the task's own Steward, else Settings → Ship's default; a failed ship's choice on Retry
  const stewardSetting = useSignal(props.client.entities.row<{ value?: unknown }>('setting', entityKey(props.client.home, 'ship.steward')));
  const [picked, setPicked] = createSignal<{ on: boolean; review: boolean } | undefined>(typed?.steward ? { on: typed.steward !== 'off', review: typed.steward === 'review' } : undefined);
  // until a box is clicked they follow the task and the setting (either may arrive after the dialog opens)
  const keepGreen = (): { on: boolean; review: boolean } => picked() ?? stewardStart(s()?.steward, stewardSetting()?.value);
  const steward = (): boolean => keepGreen().on;
  const stewardReview = (): boolean => keepGreen().review;
  const setSteward = (on: boolean): void => void setPicked({ on, review: stewardReview() });
  const setStewardReview = (review: boolean): void => void setPicked({ on: steward(), review });
  const scope = props.client.scope.child('ship-dialog');
  onCleanup(() => scope.dispose());
  const close = (): void => props.client.navigate({ t: 'closeOverlay', id: SHIP_VIEW as never });
  // A flow of its own: leaving (closing the dialog) drops a late answer.
  props.client.machine<'loading' | 'ready' | 'failed', 'loaded', void>(
    {
      id: `wisp.ship.prepare:${key}`,
      initial: 'loading',
      states: {
        loading: {
          async invoke() {
            const i = await props.client.run(prepare, { key });
            setTitle(typed?.title || i.title);
            setBody(typed?.body ?? i.body);
            setInfo(i);
            return 'loaded' as const;
          },
          on: { loaded: 'ready' },
          onError: 'failed',
        },
        ready: {},
        // the shell's toast says why; nothing to ship from here
        failed: { invoke: () => close() },
      },
    },
    undefined,
    scope,
  );
  const guess = (): ShipGuess => {
    const i = info();
    return i ? infoGuess(i) : props.params.guess;
  };
  const repos = (): readonly ShipRepo[] => {
    const i = info();
    return i ? (i.repos ?? [i]) : [];
  };
  const several = (): boolean => guess().repos > 1;
  const block = () => {
    const i = info();
    return i ? shipBlock(i) : undefined;
  };
  // TRY-13: the task's own agent writes the title and the description (the repo's PR template filled in, when it has one)
  const harnesses = useSignal(shell.harnesses());
  const agentName = (): string => harnesses().find((h) => h.id === s()?.agent)?.name ?? s()?.agent ?? '';
  const [writing, setWriting] = createSignal(false);
  const writeIt = (): void => {
    setWriting(true);
    props.client
      .run(write, { key })
      .then((d) => {
        if (d.title) setTitle(d.title);
        if (d.body) setBody(d.body);
      })
      .catch(() => {}) // the shell's toast says why
      .finally(() => setWriting(false));
  };
  let titleBox: HTMLInputElement | undefined;
  const go = (): void => {
    const i = info();
    if (!i) return;
    const dirty = i.dirty > 0 || saveTask();
    if (dirty && !title().trim()) {
      titleBox?.focus();
      shell.toast.error(t('wisp.ship.dialog.needTitle'));
      return;
    }
    const g = infoGuess(i);
    const keep = stewardArg(steward(), stewardReview(), s()?.steward);
    const values = { title: title() || i.title, body: g.update ? i.body : body(), draft: !g.update && draft(), ...(saveTask() && { saveTask: true }), ...(keep && { steward: keep }) };
    close();
    flows.start({ steps: stepsFor(dirty), values, update: g.update });
  };
  return (
    <Stack gap="6" part="ship.dialog">
      <Stack gap="4">
        <Show when={info()} fallback={<For each={props.params.guess.branches.length ? props.params.guess.branches : ['']}>{(b) => <Route branch={b} several={several()} />}</For>}>
          <For each={repos()}>{(r) => <Route r={r} several={several()} />}</For>
        </Show>
      </Stack>
      <Show when={!info() || info()?.dirty || saveTask()}>
        <Field label={guess().update ? t('wisp.ship.dialog.message') : t('wisp.ship.dialog.messageTitle')}>
          <Show when={info()} fallback={<Skeleton shape="block" height="control-md" width="full" />}>
            <Input value={title()} onInput={setTitle} maxLength={100} spellcheck={false} autofocus ref={(el) => (titleBox = el)} onKeyDown={(e) => e.key === 'Enter' && go()} />
          </Show>
        </Field>
      </Show>
      <Show when={!guess().update}>
        <Field label={t('wisp.ship.dialog.body')}>
          <Show when={info()} fallback={<Skeleton shape="block" height="avatar-lg" width="full" />}>
            <TextArea value={body()} onInput={setBody} mono spellcheck={false} minRows={6} maxRows={14} />
          </Show>
        </Field>
        {/* the draft box's line, Write at its end (v1's dialog has nothing there: nothing below moves) */}
        <Inline justify="between">
          <Checkbox checked={draft()} onChange={setDraft} label={t('wisp.ship.dialog.draft')} disabled={!info()} />
          <Show when={agentName()}>
            <Button size="xs" variant="link" part="ship.write" loading={writing()} disabled={!info() || writing()} onClick={writeIt}>
              {writing() ? t('wisp.ship.dialog.writing', { name: agentName() }) : t('wisp.ship.dialog.write', { name: agentName() })}
            </Button>
          </Show>
        </Inline>
      </Show>
      {/* Keep it green (the Steward, ADR-0132): the task fixes its PR's failing checks and new review comments, wisp
          pushes; the second box adds a review of each push by another vendor's model. Never merges */}
      <Stack gap="2" part="ship.steward">
        <Checkbox checked={steward()} onChange={setSteward} label={t('wisp.ship.dialog.steward')} disabled={!info()} part="ship.steward.on" />
        <Show when={steward()}>
          <Checkbox checked={stewardReview()} onChange={setStewardReview} label={t('wisp.ship.dialog.stewardReview')} disabled={!info()} part="ship.steward.review" />
          <Text as="p" size="xs" tone="muted" part="ship.steward.hint">
            {t(stewardReview() ? 'wisp.ship.dialog.stewardHintReview' : 'wisp.ship.dialog.stewardHint')}
          </Text>
        </Show>
      </Stack>
      <Show when={info()?.hasWisp}>
        <Checkbox checked={saveTask()} onChange={setSaveTask} label={t('wisp.ship.dialog.saveTask')} disabled={!info()} part="ship.saveTask" />
      </Show>
      <Show when={s()?.overlap}>
        {(o) => (
          <Text as="p" size="sm" tone="warn" breakAnywhere>
            {o()}
          </Text>
        )}
      </Show>
      <Show when={unsentLine(t, withUnsent(s(), unsent)?.unsentComments)}>
        {(line) => (
          <Text as="p" size="sm" tone="warn">
            {line()}
          </Text>
        )}
      </Show>
      <Switch>
        <Match when={block() === 'noRemote'}>
          <Text as="p" size="sm" tone="bad">
            {several() ? t('wisp.ship.dialog.noRemoteMany') : t('wisp.ship.dialog.noRemote')}
          </Text>
        </Match>
        <Match when={block() === 'nothing'}>
          <Text as="p" size="sm" tone="muted">
            {t('wisp.ship.dialog.nothing')}
          </Text>
        </Match>
      </Switch>
      <Stack gap="3">
        <Text as="p" size="xs" tone="muted">
          {footLine(t, guess())}
        </Text>
        <Inline gap="3" justify="end">
          <Button variant="secondary" size="sm" onClick={close}>
            {t('wisp.ship.cancel')}
          </Button>
          <Button variant="primary" size="sm" icon="send" disabled={!info() || block() !== undefined} onClick={go} part="ship.go">
            {goLabel(t, guess())}
          </Button>
        </Inline>
      </Stack>
    </Stack>
  );
}
