// Settings → Plugins (v1's Spells, ADR-0031: a change to wisp is a plugin now): every plugin this wisp runs,
// built-in or yours, with its state (on, off, refused and why) and its switch. A switch shows at once and
// the list is read again after it; switching off frees everything the plugin made (its scope).
import { createResource, createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { Button, Callout, Code, Dialog, IconButton, Inline, PathField, Pill, SettingsCast, SettingsEmpty, SettingsFieldRow, SettingsGroup, SettingsNote, SettingsRow, Stack, Switch, Text } from '@wisp/ui';
import { useShell } from '@wisp/shell';
import { cast, castPrompt, folder, folders, install, list, permissionWords, rowAbout, setEnabled, trust, uninstall, type Installed, type PluginRow } from '../commands.ts';
import { confirmOf, type Confirm } from '../confirm.ts';
import { t } from '../i18n.ts';
import '../i18n/lazy.ts';

/** the plugins that can't be switched off (wisp-plugins host's STAYS_ON) */
const STAYS_ON: readonly string[] = ['wisp.plugins', 'wisp.settings'];

const IS_URL = /^(https?:\/\/|git@|ssh:\/\/)/;
const TONE: Record<string, 'good' | 'neutral' | 'bad'> = { active: 'good', resolved: 'good', off: 'neutral', refused: 'bad' };

export default function Plugins(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const peer = c.home;
  const [rows, { refetch, mutate }] = createResource(() => c.run(list, { peer }).catch((): PluginRow[] => []));
  const [busy, setBusy] = createSignal<string | undefined>();
  // the foot names this wisp's own plugins folder (audit C9: it said ~/.wisp/plugins whatever WISP_HOME was)
  const [where] = createResource(() => c.run(folder, { peer }).then((f) => f.path).catch(() => undefined));
  // a change made elsewhere (`wisp plugin enable`, an agent, another window) shows here too: the server's
  // plugins.changed notice reads the list again (audit A F21: the pane kept its first list)
  const shell = useShell();
  onCleanup(shell.client.peers.onNotice((from, n) => from === peer && n.kind === 'plugins.changed' && void refetch()));
  // Trust (ADR-0048): a plugin of yours trusted or back; it reloads, so the list is read again after
  const setTrust = (p: PluginRow, next: boolean): void => {
    mutate((rs) => (rs ?? []).map((x) => (x.id === p.id ? { ...x, trusted: next } : x)));
    setBusy(p.id);
    c.run(trust, { peer, id: p.id, trusted: next })
      .catch(() => {})
      .finally(() => {
        setBusy(undefined);
        void refetch();
      });
  };
  const flip = (p: PluginRow, on: boolean): void => {
    mutate((rs) => (rs ?? []).map((x) => (x.id === p.id ? { ...x, enabled: on, state: on ? 'active' : 'off' } : x)));
    setBusy(p.id);
    c.run(setEnabled, { peer, id: p.id, enabled: on })
      .catch(() => {})
      .finally(() => {
        setBusy(undefined);
        void refetch();
      });
  };
  // Install…: a folder or a git URL; the answer says what it asks for before anything of it runs
  const say = t as (k: string, v?: Readonly<Record<string, string>>) => string;
  const [source, setSource] = createSignal('');
  // Folder… (v1 File…): the system's folder dialog in the desktop app; in a browser the box, which suggests the
  // folders under what you type (Tab goes into one)
  const platform = shell.platform;
  let field: HTMLInputElement | undefined;
  const browse = async (): Promise<void> => {
    if (platform.pickFolder) {
      const dir = await platform.pickFolder();
      if (dir) setSource(dir);
      return;
    }
    if (!source().trim()) setSource('~/');
    field?.focus();
  };
  const [installing, setInstalling] = createSignal(false);
  const [got, setGot] = createSignal<Installed | undefined>(undefined);
  const go = (): void => {
    const v = source().trim();
    if (!v) return;
    setInstalling(true);
    c.run(install, { peer, source: v })
      .then(
        (r) => {
          setSource('');
          setGot(r);
          void refetch();
        },
        () => undefined,
      )
      .finally(() => setInstalling(false));
  };
  const switchOn = (r: Installed): void => {
    setGot(undefined);
    const p = (rows() ?? []).find((x) => x.id === r.id);
    flip(p ?? { id: r.id, version: r.version, state: 'off', enabled: false, builtin: false }, true);
  };
  // Remove: gone from the list at once, back with the shell's toast if refused
  const remove = (p: PluginRow): void => {
    mutate((rs) => (rs ?? []).filter((x) => x.id !== p.id));
    if (got()?.id === p.id) setGot(undefined);
    c.run(uninstall, { peer, id: p.id })
      .catch(() => {})
      .finally(() => void refetch());
  };
  // Trust and Remove ask first (setup audit B13); Untrust only takes away, so it goes at once
  const [asking, setAsking] = createSignal<Confirm | undefined>();
  const askFirst = (kind: 'trust' | 'untrust' | 'remove', p: PluginRow): void => {
    const c = confirmOf(kind, p, say);
    if (c) return void setAsking(c);
    if (kind === 'untrust') setTrust(p, false);
  };
  const agreed = (c: Confirm): void => {
    setAsking(undefined);
    const p = (rows() ?? []).find((x) => x.id === c.id);
    if (!p) return;
    if (c.kind === 'trust') setTrust(p, true);
    else remove(p);
  };
  const yours = (): PluginRow[] => (rows() ?? []).filter((p) => !p.builtin);
  // v1's Turn every spell off: each of yours that's on goes off (each switch shows at once, as one flip does)
  const allOff = (): void => {
    for (const p of yours()) if (p.enabled) flip(p, false);
  };
  const builtins = (): PluginRow[] => (rows() ?? []).filter((p) => p.builtin).sort((a, b) => a.id.localeCompare(b.id));
  const row = (p: PluginRow): JSX.Element => {
    const about = (): ReturnType<typeof rowAbout> => rowAbout(p, say);
    return (
    <SettingsRow
      part="plugins.row"
      title={about().title}
      description={
        <For each={about().lines}>
          {(line) => (
            <Text as="div" size="sm" tone="muted">
              {line}
            </Text>
          )}
        </For>
      }
      control={
        <>
          <Show when={about().trust}>
            {(tr) => (
              <Button size="sm" variant="ghost" part="plugins.trust" disabled={busy() === p.id} title={say(tr().next ? 'plugins.trustOf' : 'plugins.untrustOf', { name: about().title })} onClick={() => askFirst(tr().next ? 'trust' : 'untrust', p)}>
                {tr().label}
              </Button>
            )}
          </Show>
          <Pill tone={TONE[p.state] ?? 'neutral'} dot variant="outline">
            {t(`plugins.state.${(TONE[p.state] ? p.state : 'off') as 'active'}`)}
          </Pill>
          {/* wisp.plugins and wisp.settings stay on (the host refuses them off: audit F setup F7) */}
          <Switch aria-label={t('plugins.on', { name: p.id })} checked={p.enabled} disabled={busy() === p.id || (p.enabled && STAYS_ON.includes(p.id))} onChange={(on) => flip(p, on)} />
          <Show when={p.id.startsWith('user.')}>
            <IconButton icon="trash" size="sm" variant="ghost" label={t('plugins.remove', { name: p.id })} onClick={() => askFirst('remove', p)} />
          </Show>
        </>
      }
    />
    );
  };
  // Cast (v1 Settings → Spells' box): describe the change, an agent makes it as a plugin and installs it off
  const [ask, setAsk] = createSignal('');
  const [casting, setCasting] = createSignal(false);
  const castIt = (): void => {
    const v = ask().trim();
    if (!v || casting()) return;
    setCasting(true);
    c.run(cast, { peer, prompt: castPrompt(v), title: v, ext: { 'wisp.plugins': { kind: 'cast' } } })
      .then(
        (r) => {
          setAsk('');
          c.navigate({ t: 'openTask', id: r.id as never });
        },
        () => undefined,
      )
      .finally(() => setCasting(false));
  };
  const EXAMPLES = [1, 2, 3, 4, 5, 6, 7, 8].map((n) => t(`plugins.example.${n}` as 'plugins.example.1'));
  return (
    <>
      <SettingsGroup part="plugins.cast">
        <SettingsRow part="plugins.cast.row" title={t('plugins.cast')} description={t('plugins.cast.hint')} />
        <SettingsCast
          part="plugins.cast.box"
          value={ask()}
          placeholder={t('plugins.cast.placeholder')}
          label={t('plugins.cast')}
          pin={t('plugins.cast.pin')}
          send={{ label: t('plugins.cast.send'), busy: casting() }}
          onInput={setAsk}
          onSend={castIt}
          examples={EXAMPLES}
          onExample={setAsk}
        />
      </SettingsGroup>
      <SettingsGroup part="plugins.yours">
        <Show when={yours().length > 0} fallback={<SettingsEmpty part="plugins.none" icon="wizard" title={t('plugins.none')} description={t('plugins.none.description')} />}>
          <For each={yours()}>{row}</For>
        </Show>
      </SettingsGroup>
      <SettingsGroup part="plugins.install">
        <SettingsRow
          part="plugins.install.row"
          title={t('plugins.install')}
          description={t('plugins.install.hint')}
          control={
            <Button size="sm" variant="ghost" icon="folder" part="plugins.install.folder" onClick={() => void browse()}>
              {t('plugins.install.folder')}
            </Button>
          }
        />
        <SettingsFieldRow part="plugins.install.box">
          <PathField
            ref={(el) => (field = el)}
            value={source()}
            onInput={setSource}
            suggest={(q) => c.run(folders, { peer, q })}
            skip={(q) => IS_URL.test(q)}
            onPick={setSource}
            onEnter={() => go()}
            placeholder={t('plugins.install.placeholder')}
            aria-label={t('plugins.install')}
          />
          <Button size="sm" variant="ghost" loading={installing()} disabled={!source().trim()} onClick={go}>
            {t('plugins.install.go')}
          </Button>
        </SettingsFieldRow>
        <Show when={got()}>
          {(r) => (
            <Callout tone="warn" title={say('plugins.installed', { name: r().name, version: r().version })} part="plugins.installed">
              <Stack gap="2">
                <Text size="sm">{r().permissions.length > 0 ? t('plugins.asks') : t('plugins.asksNothing')}</Text>
                <For each={r().permissions}>
                  {(p) => (
                    <Text size="sm" part="plugins.installed.permission">
                      {`• ${permissionWords(p, say)}`}
                    </Text>
                  )}
                </For>
                <Inline gap="2">
                  <Button size="sm" variant="primary" onClick={() => switchOn(r())}>
                    {t('plugins.switchOn')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setGot(undefined)}>
                    {t('plugins.notNow')}
                  </Button>
                </Inline>
              </Stack>
            </Callout>
          )}
        </Show>
      </SettingsGroup>
      <SettingsNote part="plugins.foot">
        {t('plugins.foot.pre')}
        <Code>{where() ?? t('plugins.foot.path')}</Code>
        {t('plugins.foot.post')}{' '}
        <Button variant="link" size="xs" part="plugins.allOff" onClick={allOff}>
          {t('plugins.allOff')}
        </Button>
      </SettingsNote>
      <SettingsGroup part="plugins.builtin" title={t('plugins.builtin')} description={t('plugins.builtin.description')}>
        <For each={builtins()}>{row}</For>
      </SettingsGroup>
      <Show when={asking()}>
        {(a) => (
          <Dialog
            open
            size="sm"
            role="alertdialog"
            onOpenChange={(o) => !o && setAsking(undefined)}
            title={a().title}
            part="plugins.confirm"
            footer={
              <Inline gap="3" justify="end">
                <Button variant="ghost" size="sm" onClick={() => setAsking(undefined)}>
                  {t('plugins.cancel')}
                </Button>
                <Button variant={a().kind === 'remove' ? 'danger' : 'primary'} size="sm" onClick={() => agreed(a())}>
                  {a().ok}
                </Button>
              </Inline>
            }
          >
            <Text as="p" tone="muted">
              {a().detail}
            </Text>
          </Dialog>
        )}
      </Show>
    </>
  );
}
