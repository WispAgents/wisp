// Settings → Cloud → API keys (F-mach-15, ADR-0058; v1 web/js/features/api-keys.js): keys for scripting your cloud
// wisp, `Authorization: Bearer wgk_…` at its address. They live on the grid, hashed. Create shows the row at once
// (Making…) and clears the box; the key comes back once, in `fresh` below and nowhere else (no store row, no draft,
// no storage), and goes on Done or when the pane is left. Revoke asks, then the row goes at once; a refusal brings
// it back in place and the client's toast says why.
import { createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import { defineCommand, type PeerId, type PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Code, englishT, Inline, Input, SettingsBlock, SettingsGroup, SettingsInputRow, SettingsRow, Stack, Text, timeLabel } from '@wisp/ui';
import { t } from '../i18n.ts';
import { adding, keyLine, keysOf, madeOf, nameProblem, NAME_MAX, restored, settled, tryLine, without, type ApiKey, type Fresh, type KeysView } from '../keys.ts';
import { Confirm } from './Confirm.tsx';

const list = defineCommand<{ peer: PeerId }, unknown>({ id: 'grid-client.keys', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the keys list' });
const create = defineCommand<{ peer: PeerId; name: string }, unknown>({
  id: 'grid-client.keysCreate',
  owner: (a) => a.peer,
  wire: (a) => ({ name: a.name.trim() }),
  optimistic: 'none',
  pending: 'the new key’s row (Making…)',
});
const revoke = defineCommand<{ peer: PeerId; id: string }>({ id: 'grid-client.keysRevoke', owner: (a) => a.peer, wire: (a) => ({ id: a.id }), optimistic: 'none', pending: 'the row, gone at once' });

export function ApiKeys(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
  const [view, setView] = createSignal<KeysView | undefined>();
  const [failed, setFailed] = createSignal('');
  const [name, setName] = createSignal('');
  const [problem, setProblem] = createSignal('');
  const [fresh, setFresh] = createSignal<Fresh | undefined>();
  const [asking, setAsking] = createSignal<ApiKey | undefined>();
  // leaving the pane, however it's left: the key goes with it
  onCleanup(() => setFresh(undefined));
  const keys = (): readonly ApiKey[] => view()?.keys ?? [];
  const edit = (fn: (l: readonly ApiKey[]) => ApiKey[]): void => {
    setView((v) => (v ? { ...v, keys: fn(v.keys) } : v));
  };
  const when = (at: number): string => timeLabel(at, c.scope.timers.now(), englishT);

  const load = (): void => {
    setFailed('');
    c.run(list, { peer: c.home }).then(
      (v) => setView(keysOf(v)),
      (e: unknown) => setFailed(e instanceof Error ? e.message : String(e)),
    );
  };
  load();

  let n = 0;
  const make = (): void => {
    const why = nameProblem(name());
    if (why) return void setProblem(t(why));
    const typed = name();
    const tmp = `new-${(n += 1)}`;
    // at once: the box empty, the row in the list; the key itself comes back from the grid
    setProblem('');
    setName('');
    edit((l) => adding(l, tmp, typed, c.scope.timers.now()));
    c.run(create, { peer: c.home, name: typed }).then(
      (v) => {
        const made = madeOf(v);
        edit((l) => settled(l, tmp, made?.row));
        if (made) setFresh({ name: made.row.name, key: made.key });
      },
      () => {
        edit((l) => settled(l, tmp, undefined));
        if (!name()) setName(typed);
      },
    );
  };

  const drop = (k: ApiKey): void => {
    edit((l) => without(l, k.id));
    c.run(revoke, { peer: c.home, id: k.id }).then(
      () => shell.toast.show(t('grid-client.keys.revoked', { name: k.name })),
      () => {
        edit((l) => restored(l, k));
        load(); // gone from another computer already: the list says so
      },
    );
  };

  const copy = (f: Fresh): void => void shell.platform.copy(f.key).then((ok) => ok && shell.toast.show(t('grid-client.keys.copied')));

  return (
    <SettingsGroup part="grid-client.keys">
      {/* v1 .key-card: the title and its line, the form above the list (a new key's panel opens under it) */}
      <SettingsBlock part="grid-client.keys.add" title={t('grid-client.keys.title')} description={t('grid-client.keys.description')}>
        <SettingsInputRow part="grid-client.keys.form">
          <Input
            aria-label={t('grid-client.keys.nameLabel')}
            placeholder={t('grid-client.keys.namePlaceholder')}
            value={name()}
            maxLength={NAME_MAX * 2}
            autocomplete="off"
            spellcheck={false}
            invalid={problem() !== ''}
            onInput={(v) => {
              setName(v);
              if (problem()) setProblem('');
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.isComposing) {
                e.preventDefault();
                make();
              }
            }}
          />
          <Button size="sm" variant="ghost" icon="plus" part="grid-client.keys.create" disabled={view() === undefined} onClick={make}>
            {t('grid-client.keys.create')}
          </Button>
        </SettingsInputRow>
        <Show when={problem()}>{(p) => <Text tone="bad">{p()}</Text>}</Show>
      </SettingsBlock>
      <Show when={fresh()}>
        {(f) => (
          <SettingsBlock part="grid-client.keys.new" title={t('grid-client.keys.ready', { name: f().name })} description={t('grid-client.keys.once')}>
            <Stack gap="2">
              <Code block>{f().key}</Code>
              <Text tone="muted">{t('grid-client.keys.try')}</Text>
              <Code block lang="sh">
                {tryLine(view()?.address ?? '')}
              </Code>
              <Inline gap="2">
                <Button size="sm" variant="primary" icon="copy" part="grid-client.keys.copy" onClick={() => copy(f())}>
                  {t('grid-client.keys.copy')}
                </Button>
                <Button size="sm" variant="ghost" part="grid-client.keys.done" onClick={() => setFresh(undefined)}>
                  {t('grid-client.keys.done')}
                </Button>
              </Inline>
            </Stack>
          </SettingsBlock>
        )}
      </Show>
      <Show
        when={view()}
        fallback={
          <Show
            when={failed()}
            fallback={<SettingsRow part="grid-client.keys.loading" title="" description={t('grid-client.keys.loading')} control={<span />} />}
          >
            {(why) => (
              <SettingsRow
                part="grid-client.keys.failed"
                title={t('grid-client.keys.failed')}
                description={why()}
                control={
                  <Button size="sm" variant="ghost" onClick={load}>
                    {t('grid-client.keys.retry')}
                  </Button>
                }
              />
            )}
          </Show>
        }
      >
        <Show when={keys().length > 0} fallback={<SettingsRow part="grid-client.keys.none" title="" description={t('grid-client.keys.none')} control={<span />} />}>
          <For each={keys()}>
            {(k) => (
              <SettingsRow
                part="grid-client.key"
                title={k.name}
                description={keyLine(k, when, say)}
                control={
                  <Button size="sm" variant="ghost" tone="bad" part="grid-client.key.revoke" disabled={k.pending === true} onClick={() => setAsking(k)}>
                    {t('grid-client.keys.revoke')}
                  </Button>
                }
              />
            )}
          </For>
        </Show>
      </Show>
      <Confirm
        open={asking() !== undefined}
        question={t('grid-client.keys.revoke.ask', { name: asking()?.name ?? '' })}
        detail={t('grid-client.keys.revoke.detail')}
        ok={t('grid-client.keys.revoke.ok')}
        danger
        onAnswer={(yes) => {
          const k = asking();
          setAsking(undefined);
          if (yes && k) drop(k);
        }}
      />
    </SettingsGroup>
  );
}
