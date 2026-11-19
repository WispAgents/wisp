// Remove its worktrees? (design/worktrees-on-done.md, v1 done-folders.js folderDialog): one row a folder the
// task marked done and what ran under it hold, clean ones ticked, one holding work unticked and saying what
// it would lose. Remove closes at once and the removal runs behind (a toast says what went, or what was kept
// and why); Keep pins every row. Esc, × or a click outside answers nothing: the hourly cleanup still looks.
import { createEffect, createSignal, For, onCleanup, onMount, Show, type JSX } from 'solid-js';
import { setSetting, type EntityKey, type ExtensionId, type PluginClient, type SettingRow } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Checkbox, Divider, Inline, Skeleton, Stack, Text, useSignal } from '@wisp/ui';
import { bare, forget, peerOf, rowsFor, settingKey, VIEW } from '../ask.ts';
import { doneApply, doneList } from '../commands.ts';
import { heading, initiallyTicked, outcome, removal, removeButton, rowStatus, SESSION, undone, withSizes, type Said } from '../done.ts';
import { fmtSize } from '../format.ts';
import { t } from '../i18n.ts';
import { SETTING, type DoneFolder } from '../types.ts';
import type { DoneParams } from '../view.ts';
import '../i18n/lazy.ts';

const say = (s: Said): string => t(s.key, s.vars);

// One row a folder (v1 .wd-row): its tick, the task's name over its branch taking all the width there is (the branch
// cut only when it must), one short status at the right (warn once Remove would lose what it holds) and its size.
function Row(props: { readonly f: DoneFolder; readonly ticked: boolean; readonly onTick: (on: boolean) => void }): JSX.Element {
  const who = (): string => (props.f.own ? t('worktrees.folders.this') : props.f.title || props.f.branch);
  const st = () => rowStatus(props.f, props.ticked);
  // what Remove would lose, in full, on hover
  const tip = (): { title?: string } => {
    const x = st().tip;
    return x ? { title: say(x) } : {};
  };
  return (
    <Inline gap="5" align="center" padX="6" padY="4" part="worktrees.folders.row">
      <Checkbox checked={props.ticked} onChange={(on) => props.onTick(on)} aria-label={t('worktrees.folders.tick', { name: who() })} part="worktrees.folders.tick" />
      <Stack gap="0" grow part="worktrees.folders.who">
        <Text weight="medium" truncate title={props.f.title}>
          {who()}
        </Text>
        <Text size="sm" tone="muted" mono truncate title={props.f.branch || props.f.path}>
          {props.f.branch || props.f.path}
        </Text>
      </Stack>
      <Text size="sm" tone={st().warn ? 'warn' : 'muted'} nowrap part="worktrees.folders.state" {...tip()}>
        {say(st())}
      </Text>
      <Show when={props.f.size !== null} fallback={<Skeleton shape="line" width="time-col" part="worktrees.folders.size" />}>
        <Text size="sm" tone="muted" numeric nowrap align="end" part="worktrees.folders.size">
          {fmtSize(props.f.size ?? 0)}
        </Text>
      </Show>
    </Inline>
  );
}

/** The rows in their shape while git looks (v1 .wd-wait). */
function Placeholders(props: { readonly n: number }): JSX.Element {
  return (
    <For each={Array.from({ length: props.n })}>
      {(_, i) => (
        <>
          <Show when={i() > 0}>
            <Divider />
          </Show>
          <Inline gap="5" align="center" padX="6" padY="4" aria-hidden="true">
            <Skeleton shape="block" width="check" height="check" />
            <Stack gap="1" grow>
              <Skeleton shape="line" width="1/2" />
              <Skeleton shape="line" width="1/3" />
            </Stack>
            <Skeleton shape="line" width="time-col" />
          </Inline>
        </>
      )}
    </For>
  );
}

export default function DoneDialog(props: { readonly client: PluginClient; readonly params: DoneParams }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const key = props.params.key;
  const [folders, setFolders] = createSignal<readonly DoneFolder[] | undefined>(undefined);
  const [ticked, setTicked] = createSignal<readonly string[]>([]);
  const [always, setAlways] = createSignal(false);
  const [only, setOnly] = createSignal(props.params.only);
  // followed (useSignal, round 324): a row that lands or locks after the dialog opened hides Always do this
  const setting = useSignal(c.entities.row<SettingRow>('setting', settingKey(key, SETTING.onDone)));
  // Remove already says what to do, and a formula or plugin's lock isn't the dialog's to change
  const lock = (): boolean => only() || !!setting()?.locked_by;
  const close = (): void => c.navigate({ t: 'closeOverlay', id: VIEW as ExtensionId });
  // Undo (or anything that reopens the task) takes the question with it: nothing of a live task's folder is removed
  const task = useSignal(c.entities.row<{ state?: string }>(SESSION, key as EntityKey));
  createEffect(() => {
    if (undone(task())) close();
  });
  let open = true;
  onCleanup(() => {
    open = false;
    forget(key);
  });

  onMount(() => {
    rowsFor(c, key, props.params.only).then(
      (a) => {
        if (!open) return;
        if (a.folders.length === 0) return close();
        setOnly(a.only);
        setFolders(a.folders);
        setTicked(initiallyTicked(a.folders));
        // sizes after: du over a big node_modules takes seconds
        c.run(doneList, { peer: peerOf(key), sessionId: bare(key), sizes: true }).then(
          (full) => {
            const now = folders();
            if (open && now) setFolders(withSizes(now, full.folders));
          },
          () => {
            const now = folders();
            if (open && now) setFolders(now.map((f) => ({ ...f, size: f.size ?? 0 })));
          },
        );
      },
      () => close(),
    );
  });

  const remember = (value: 'remove' | 'keep'): void => {
    if (always() && !lock()) void c.run(setSetting, { peer: peerOf(key), key: SETTING.onDone, value }).catch(() => undefined);
  };
  const answer = (body: { remove?: string[]; force?: string[]; keep?: string[] }, list: readonly DoneFolder[], loud: boolean): void => {
    const keepTasks = body.keep ? list.filter((f) => body.keep?.includes(f.path)).map((f) => f.task) : [];
    c.run(doneApply, { peer: peerOf(key), sessionId: bare(key), ...body, keepTasks }).then(
      (out) => {
        const o = outcome(out, list);
        if (!o || (!loud && o.tone === 'success')) return;
        const text = t(o.said.key, { ...o.said.vars, size: fmtSize(o.bytes) });
        if (o.tone === 'error') shell.toast.error(text);
        else shell.toast.success(text);
      },
      () => undefined, // the client's onError says why
    );
  };
  const onRemove = (): void => {
    const list = folders();
    if (!list) return;
    const r = removal(list, ticked());
    if (r.remove.length === 0) return;
    remember('remove');
    close();
    answer(r, list, true);
  };
  const onKeep = (): void => {
    const list = folders();
    if (!list) return;
    remember('keep');
    close();
    answer({ keep: list.map((f) => f.path) }, list, false);
  };
  const btn = () => removeButton(folders() ?? [], ticked());
  const head = () => heading(folders() ?? [], only());

  return (
    <Stack gap="4" part="worktrees.folders">
      <Text as="p" tone="muted">
        {folders() ? say(head().hint) : t('worktrees.folders.hint')}
      </Text>
      {/* v1 .wd-rows: the rows in one bordered list, a hairline between two */}
      <Stack gap="0" border="line" radius="lg" part="worktrees.folders.rows">
        <Show
          when={folders()}
          fallback={<Placeholders n={props.params.n} />}
        >
          {(list) => (
            <For each={list()}>
              {(f, i) => (
                <>
                  <Show when={i() > 0}>
                    <Divider />
                  </Show>
                  <Row f={f} ticked={ticked().includes(f.path)} onTick={(on) => setTicked(on ? [...ticked(), f.path] : ticked().filter((p) => p !== f.path))} />
                </>
              )}
            </For>
          )}
        </Show>
      </Stack>
      <Inline gap="3" align="center" justify="between" part="worktrees.folders.foot">
        <Show when={!lock()} fallback={<span />}>
          <Checkbox checked={always()} onChange={setAlways} label={t('worktrees.folders.always')} part="worktrees.folders.always" />
        </Show>
        <Inline gap="2">
          <Button size="sm" variant="ghost" part="worktrees.folders.keep" disabled={!folders()} onClick={onKeep}>
            {t('worktrees.folders.keep')}
          </Button>
          <Button size="sm" variant={btn().danger ? 'danger' : 'primary'} part="worktrees.folders.remove" disabled={!folders() || btn().disabled} onClick={onRemove}>
            {say(btn().label)}
          </Button>
        </Inline>
      </Inline>
    </Stack>
  );
}
