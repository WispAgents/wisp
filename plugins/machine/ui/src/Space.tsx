// The Space tab: the disk as one bar (build output, wisp's folders, everything else, free), the build output folder by
// folder (ADR-0144: it goes, the folders and their changes stay), Free up space (design/machine-page.md §4) and the
// harnesses' own folders (design/machine.md "Harness data"): the reclaim groups safest first. Review opens Settings → Worktrees, Remove sends
// the group's ids (asking first for the folders no task knows). Harness folders hold sign-ins:
// only the removable files (crash dumps, stale temp files) go, and each harness says how to keep
// less with its own CLI.

import type { PluginClient } from '@wisp/client';
import { Button, Each, Inline, Section, StackedMeter, Text, UsageList, UsageRow, UsageSkeleton } from '@wisp/ui';
import { For, Show, createEffect, createMemo, createSignal, untrack, type JSX } from 'solid-js';
import { machineCommands } from './commands.ts';
import type { MachineData } from './data.ts';
import type { Host } from './host.ts';
import type { BuildFolder, HarnessGroup } from './types.ts';
import { buildGroup, diskParts, reclaimRows, size, type ReclaimRow } from './view.ts';
import './i18n/lazy.ts';

interface Props {
  readonly client: PluginClient;
  readonly host: Host;
  readonly data: MachineData;
  readonly confirm: (o: { title: string; body: string; ok: string; run: () => void }) => void;
  /** Settings → Worktrees (with the Left over filter for folders no task knows) */
  readonly review: (filter: 'all' | 'left') => void;
}

/** The disk as one bar, its free space above it. */
export function DiskSpace(props: Pick<Props, 'client' | 'data'>): JSX.Element {
  const t = props.client.t;
  const parts = createMemo(() => diskParts(props.data.snap(), t));
  const disk = (): { free: number; total: number } | undefined => props.data.snap()?.disk;
  return (
    <Section title={t('machine.space.title')} part="machine.disk">
      <Show when={disk()} fallback={<UsageSkeleton cols={1} />}>
        {(d) => (
          <>
            <Inline gap="2" align="baseline">
              <Text size="xl" weight="semibold" numeric>
                {size(d().free) || '—'}
              </Text>
              <Text tone="muted" size="sm" numeric>
                {t('machine.space.of', { total: size(d().total) })}
              </Text>
            </Inline>
            <StackedMeter
              label={t('machine.gauge.disk')}
              valueText={t('machine.space.freeOf', { free: size(d().free), total: size(d().total) })}
              segments={parts()}
              rest={t('machine.space.part.free', { size: size(d().free) })}
              part="machine.disk.bar"
            />
          </>
        )}
      </Show>
    </Section>
  );
}

/** Build output, folder by folder, the biggest first: Remove on each, and Remove all. A row says Removing… from the
 * click until it leaves the snapshot (the peer samples again the moment the clean-up ends, and the bar's free space
 * grows then); a refusal puts it back and the runner's toast says why. */
export function BuildOutput(props: Props): JSX.Element {
  const t = props.client.t;
  const cmds = machineCommands(props.data.peer);
  const group = createMemo(() => buildGroup(props.data.snap()));
  // the folders being removed, by id: Removing… on their rows, and gone from here once the snapshot drops them
  const [going, setGoing] = createSignal<ReadonlySet<string>>(new Set());
  createEffect(() => {
    const here = new Set((group()?.folders ?? []).map((f) => f.id));
    const left = [...untrack(going)].filter((id) => here.has(id));
    if (left.length !== untrack(going).size) setGoing(new Set(left));
  });
  const isGoing = (ids: readonly string[]): boolean => ids.length > 0 && ids.every((id) => going().has(id));
  // the biggest on their own lines; the small ones (dozens of 5 MB node_modules) as one line with one Remove
  const split = createMemo(() => {
    const all = group()?.folders ?? [];
    const shown = all.filter((f, i) => i < BUILD_ROWS && f.bytes >= BUILD_ROW_MIN);
    return { shown, rest: all.slice(shown.length) };
  });
  const send = (folders: readonly BuildFolder[]): void => {
    const ids = folders.filter((f) => !f.leftover).map((f) => f.id);
    const leftovers = folders.filter((f) => f.leftover).map((f) => f.id);
    const bytes = folders.reduce((n, f) => n + f.bytes, 0);
    const keys = folders.map((f) => f.id);
    setGoing((s) => new Set([...s, ...keys]));
    props.client
      // ids always named, even none: without them the server takes every task's
      .run(cmds.cleanBuild, { build: true, ids, ...(leftovers.length && { leftovers }) })
      .then(() => {
        props.host.toast.success(t('machine.build.going', { size: size(bytes) }));
        // the peer samples every 10 s for a while: the folders' new sizes and the disk's free space follow at once
        void props.client.run(cmds.read, {}).catch(() => {});
      })
      .catch(() => setGoing((s) => new Set([...s].filter((k) => !keys.includes(k))))); // refused: the runner's toast says why
  };
  return (
    <Show when={group()}>
      {(g) => (
        <Section
          title={t('machine.build.title')}
          description={t('machine.build.desc')}
          part="machine.build"
          actions={
            <Button variant="ghost" size="sm" loading={isGoing(g().folders.map((f) => f.id))} onClick={() => send(g().folders)}>
              {t('machine.build.all', { size: size(g().bytes) })}
            </Button>
          }
        >
          <UsageList>
            <Each items={split().shown} key={(f) => f.id}>
              {(f) => (
                <UsageRow
                  title={f().title}
                  mono={f().leftover}
                  busy={going().has(f().id)}
                  sub={
                    going().has(f().id)
                      ? t('machine.build.removing')
                      : f().leftover
                        ? f().only
                          ? t('machine.build.leftoverWork', { count: f().only ?? 0 })
                          : t('machine.build.leftover')
                        : t('machine.build.task')
                  }
                  cells={[{ label: t('machine.col.folder'), value: size(f().bytes) }]}
                  actions={
                    <Button variant="ghost" size="xs" loading={going().has(f().id)} onClick={() => send([f()])}>
                      {t('machine.act.remove')}
                    </Button>
                  }
                />
              )}
            </Each>
            <Show when={split().rest.length > 0}>
              <UsageRow
                title={t('machine.build.more', { count: split().rest.length })}
                busy={isGoing(split().rest.map((f) => f.id))}
                {...(isGoing(split().rest.map((f) => f.id)) && { sub: t('machine.build.removing') })}
                cells={[{ label: t('machine.col.folder'), value: size(split().rest.reduce((n, f) => n + f.bytes, 0)) }]}
                actions={
                  <Button variant="ghost" size="xs" loading={isGoing(split().rest.map((f) => f.id))} onClick={() => send(split().rest)}>
                    {t('machine.act.remove')}
                  </Button>
                }
                part="machine.build.more"
              />
            </Show>
          </UsageList>
        </Section>
      )}
    </Show>
  );
}

/** Build output folders on their own line: the biggest, at most this many, each at least `BUILD_ROW_MIN`. */
const BUILD_ROWS = 8;
const BUILD_ROW_MIN = 50 * 1024 ** 2;

export function FreeUp(props: Props): JSX.Element {
  const t = props.client.t;
  const cmds = machineCommands(props.data.peer);
  const groups = createMemo(() => reclaimRows(props.data.snap(), t));
  const sum = createMemo(() => groups().reduce((n, g) => n + g.bytes, 0));
  const remove = (g: ReclaimRow): void => {
    const run = (): void =>
      void props.client
        .run(cmds.clean, g.kind === 'leftover' ? { leftovers: g.ids } : { ids: g.ids })
        .then(() => {
          props.host.toast.success(t('machine.free.going', { label: g.label, size: size(g.bytes) }));
          void props.client.run(cmds.read, {}).catch(() => {});
        })
        .catch(() => {}); // refused: the row comes back and the runner's toast says why
    if (g.safe) return run();
    props.confirm({ title: t('machine.free.confirmTitle', { label: g.label }), body: t('machine.free.confirmBody'), ok: t('machine.act.remove'), run });
  };
  const next = (): string => {
    const days = props.data.snap()?.cleanup ?? '7';
    return days === 'off' ? t('machine.free.off') : t('machine.free.next', { days });
  };
  return (
    <Section title={t('machine.free.title')} description={props.data.snap() ? (groups().length ? t('machine.free.sum', { size: size(sum()) }) : t('machine.free.nothing')) : ''} part="machine.freeUp">
      <UsageList>
        <Show when={props.data.snap()} fallback={<For each={[0, 1]}>{() => <UsageSkeleton cols={1} />}</For>}>
          <Each items={groups()} key={(g) => g.key}>
            {(g) => (
              <UsageRow
                title={g().label}
                sub={g().safe ? t('machine.free.safe') : t('machine.free.unsafe')}
                cells={[{ label: t('machine.col.folder'), value: size(g().bytes) }]}
                actions={
                  <>
                    <Button variant="ghost" size="xs" onClick={() => props.review(g().kind === 'leftover' ? 'left' : 'all')}>
                      {t('machine.act.review')}
                    </Button>
                    <Button variant="ghost" size="xs" {...(!g().safe && { tone: 'bad' as const })} onClick={() => remove(g())}>
                      {t('machine.act.remove')}
                    </Button>
                  </>
                }
              />
            )}
          </Each>
        </Show>
      </UsageList>
      <Text as="p" tone="muted" size="xs">
        {next()}
      </Text>
    </Section>
  );
}

export function HarnessData(props: Props): JSX.Element {
  const t = props.client.t;
  const cmds = machineCommands(props.data.peer);
  // what's being removed, each on its own: one harness's Remove puts only its row and button in waiting ('*': all)
  const [busy, setBusy] = createSignal<ReadonlySet<string>>(new Set());
  const isBusy = (id: string): boolean => busy().has(id) || busy().has('*');
  const group = createMemo(() => props.data.snap()?.reclaim.find((g): g is HarnessGroup => g.kind === 'harness'));
  const clean = async (ids?: readonly string[]): Promise<void> => {
    const keys = ids ?? ['*'];
    setBusy((s) => new Set([...s, ...keys]));
    try {
      const r = await props.client.run(cmds.harnessClean, ids ? { ids } : {});
      props.host.toast.success(t('machine.harness.freed', { size: size(r.freed) }));
    } catch {
      // refused: the runner's toast says why
    } finally {
      setBusy((s) => new Set([...s].filter((k) => !keys.includes(k))));
    }
  };
  return (
    <Show when={group()}>
      {(g) => (
        <Section
          title={t('machine.harness.title')}
          description={t('machine.harness.desc', { size: size(g().total) })}
          part="machine.harness"
          actions={
            <Show when={g().bytes > 0}>
              <Button variant="ghost" size="sm" loading={busy().has('*')} onClick={() => void clean()}>
                {t('machine.act.remove')}
              </Button>
            </Show>
          }
        >
          <UsageList>
            <Each items={g().harnesses} key={(h) => h.id}>
              {(h) => (
                <UsageRow
                  busy={isBusy(h().id)}
                  title={h().name}
                  sub={h().removable.bytes > 0 ? t('machine.harness.removable', { size: size(h().removable.bytes) }) : h().how || t('machine.harness.nothing')}
                  cells={[{ label: t('machine.col.folder'), value: size(h().bytes) }]}
                  actions={
                    <Show when={h().removable.bytes > 0}>
                      <Button variant="ghost" size="xs" loading={isBusy(h().id)} onClick={() => void clean([h().id])}>
                        {t('machine.act.remove')}
                      </Button>
                    </Show>
                  }
                />
              )}
            </Each>
          </UsageList>
        </Section>
      )}
    </Show>
  );
}
