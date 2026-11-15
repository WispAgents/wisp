// The Computers destination (ADR-0065 steps 8 and 13, design/mobile.md §A.2): the saved computers, each with how it's
// reached and its state as a word, the launched one marked; a tap launches it (the shell runs on it and shows its tasks,
// in the same frame: launch() only switches the home and navigates). *Add computer* in the bar: *Scan from your
// computer* (the desktop's QR through the native scanner, the key pinned) or the address typed by hand. A computer
// whose key changed says so on its row, with *Trust the new key* behind a confirmation. *Stay connected* (the
// background connection) shows while an SSH computer without push is saved, ON unless turned off. Kit components only,
// no CSS: each platform's look comes from the kit. wisp Cloud sign-in is step 14.
import { createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import type { PeerId, PluginClient } from '@wisp/client';
import type { Scanned } from '@wisp/shell/platform';
import { Button, Dialog, IconButton, EmptyState, Field, Input, ListRow, Pill, Scroll, Section, Sheet, Switch, Text, TopAppBar, useSignal, type Toaster } from '@wisp/ui';
import type { Tone } from '@wisp/tokens';
import { alreadySaved, keyChanged, manualComputer, needsStay, scanOutcome, type AddressError, type NewSsh } from '../add.ts';
import type { Computer, NativeBridge } from '../bridge.ts';
import { computerRows, stateOf, type ComputerRow, type ComputerState } from '../computers.ts';
import type { Launcher } from '../launch.ts';
import { peerMap } from '../peers.ts';
import type { StayConnected } from '../stay.ts';

const TONE: Record<ComputerState, Tone> = { online: 'good', connecting: 'info', offline: 'neutral', asleep: 'neutral', update: 'warn' };

export interface ComputersDeps {
  readonly bridge: Pick<NativeBridge, 'list' | 'onComputers' | 'save' | 'trustHostKey'>;
  readonly launcher: Launcher;
  /** the native code scanner (platform.scanCode); none: the address is typed */
  readonly scan?: () => Promise<Scanned>;
  readonly stay?: StayConnected;
  readonly toast?: Pick<Toaster, 'error' | 'success'>;
  /** a new saved id (crypto.randomUUID in the app) */
  readonly newId?: () => string;
}

type T = (k: string, vars?: Record<string, string>) => string;

function Row(props: { readonly client: PluginClient; readonly row: ComputerRow; readonly deps: ComputersDeps; readonly t: T }): JSX.Element {
  const status = useSignal(props.client.peers.status(props.row.peer));
  // the state follows the peer live: a row computed once would keep *Connecting*
  const state = (): ComputerState => stateOf(status());
  const changed = (): ReturnType<typeof keyChanged> => keyChanged(status());
  const [asking, setAsking] = createSignal(false);
  const open = (): void => void props.deps.launcher.launch(props.row);
  const trust = (fp: string): void => {
    setAsking(false); // the dialog goes at once; the row shows Connecting as the peer tries again
    props.deps.bridge.trustHostKey(props.row.id, fp).then(
      () => props.client.peers.reconnect(props.row.peer),
      () => props.deps.toast?.error(props.t('phone.key.failed')),
    );
  };
  return (
    <>
      <ListRow
        part="phone.computer"
        title={props.row.name}
        sub={changed() ? props.t('phone.key.changed') : props.t(`phone.via.${props.row.via}`)}
        {...(changed() && { subTone: 'warn' as const })}
        current={props.row.current}
        strong={props.row.current}
        pills={
          <Pill tone={changed() ? 'warn' : TONE[state()]} dot live={state() === 'connecting'} variant="plain" size="sm">
            {props.t(changed() ? 'phone.state.keyChanged' : `phone.state.${state()}`)}
          </Pill>
        }
        actions={
          <Show
            when={changed()}
            fallback={
              <Button variant={props.row.current ? 'secondary' : 'primary'} size="sm" part="phone.computer.open" onClick={open}>
                {props.t(props.row.current ? 'phone.tasks' : 'phone.open')}
              </Button>
            }
          >
            <Button variant="secondary" size="sm" part="phone.computer.trust" onClick={() => setAsking(true)}>
              {props.t('phone.key.trust')}
            </Button>
          </Show>
        }
        onOpen={() => (changed() ? setAsking(true) : open())}
      />
      <Show when={changed()}>
        {(c) => (
          <Dialog
            role="alertdialog"
            size="sm"
            part="phone.computer.trust-dialog"
            title={props.t('phone.key.confirmTitle', { name: props.row.name })}
            description={props.t(c().pinned ? 'phone.key.confirmPinned' : 'phone.key.confirmChanged')}
            open={asking()}
            onOpenChange={setAsking}
            footer={
              <>
                <Button variant="secondary" onClick={() => setAsking(false)}>
                  {props.t('phone.cancel')}
                </Button>
                <Button variant="danger" part="phone.computer.trust-confirm" onClick={() => trust(c().fingerprint)}>
                  {props.t('phone.key.trust')}
                </Button>
              </>
            }
          >
            <Text mono size="sm" part="phone.computer.fingerprint">
              {c().fingerprint}
            </Text>
          </Dialog>
        )}
      </Show>
    </>
  );
}

const WHY: Record<'unsupported' | 'denied' | 'notWisp', string> = { unsupported: 'phone.add.noScanner', denied: 'phone.add.denied', notWisp: 'phone.add.notWisp' };

function AddSheet(props: {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly deps: ComputersDeps;
  readonly t: T;
  readonly add: (c: NewSsh) => void;
}): JSX.Element {
  const [name, setName] = createSignal('');
  const [address, setAddress] = createSignal('');
  const [error, setError] = createSignal<AddressError | undefined>();
  const [note, setNote] = createSignal<string | undefined>();
  const [scanning, setScanning] = createSignal(false);
  const reset = (): void => {
    setName('');
    setAddress('');
    setError(undefined);
    setNote(undefined);
  };
  const close = (): void => {
    props.onOpenChange(false);
    reset();
  };
  const scan = (): void => {
    const s = props.deps.scan;
    if (!s) return;
    setScanning(true); // the button's pending state while the camera opens
    s().then(
      (r) => {
        setScanning(false);
        if (!props.open) return; // the sheet was closed while the camera was up: nothing is added behind it
        const o = scanOutcome(r);
        if (o.kind === 'save') {
          close();
          props.add(o.computer);
        } else if (o.kind === 'manual') setNote(props.t(WHY[o.why]));
      },
      () => {
        setScanning(false);
        setNote(props.t(WHY.unsupported));
      },
    );
  };
  const save = (): void => {
    const r = manualComputer(name(), address());
    if ('error' in r) return void setError(r.error);
    close();
    props.add(r.computer);
  };
  return (
    <Sheet title={props.t('phone.add.title')} open={props.open} onOpenChange={(o) => (o ? props.onOpenChange(true) : close())} part="phone.add">
      <Show when={props.deps.scan}>
        <Section title={props.t('phone.add.scanTitle')} description={props.t('phone.add.scanHow')}>
          <Button variant="primary" icon="camera" loading={scanning()} part="phone.add.scan" onClick={scan}>
            {props.t('phone.add.scan')}
          </Button>
        </Section>
      </Show>
      <Section title={props.t('phone.add.manualTitle')} description={note() ?? props.t('phone.add.manualHow')}>
        <Field label={props.t('phone.add.address')} hint={props.t('phone.add.addressHint')} {...(error() && { error: props.t(`phone.add.error.${error()}`) })}>
          <Input
            value={address()}
            inputMode="url"
            placeholder={props.t('phone.add.addressPlaceholder')}
            onInput={(v) => {
              setAddress(v);
              setError(undefined);
            }}
            part="phone.add.address"
          />
        </Field>
        <Field label={props.t('phone.add.name')} hint={props.t('phone.add.nameHint')}>
          <Input value={name()} onInput={setName} part="phone.add.name" />
        </Field>
        <Button variant={props.deps.scan ? 'secondary' : 'primary'} part="phone.add.save" onClick={save}>
          {props.t('phone.add.save')}
        </Button>
      </Section>
    </Sheet>
  );
}

export function Computers(props: { readonly client: PluginClient } & ComputersDeps): JSX.Element {
  const t: T = (k, vars) => props.client.t(k, vars);
  const [list, setList] = createSignal<readonly Computer[]>([]);
  const [adding, setAdding] = createSignal(false);
  const load = (): void => void props.bridge.list().then(setList, () => undefined);
  load();
  onCleanup(props.bridge.onComputers(load));
  const home: PeerId = props.client.home; // the shell is run again on a switch, so this is read fresh
  const rows = (): ComputerRow[] => computerRows(list(), peerMap(list()), () => undefined, home);
  const stayOn = useSignal(props.stay?.on ?? (() => false));
  const stayHere = useSignal(props.stay?.offered ?? (() => false));
  // the new computer is in the list at once; the native side's list follows (onComputers), or a refusal takes it back
  const add = (c: NewSsh): void => {
    const had = alreadySaved(list(), c);
    if (had) return void props.toast?.success(t('phone.add.already', { name: had.name }));
    const id = props.newId?.() ?? `ssh-${list().length + 1}`;
    const saved: Computer = { id, ...c };
    const before = list();
    setList([...before, saved]);
    props.bridge.save(saved).then(
      () => props.toast?.success(t('phone.add.added', { name: c.name })),
      () => {
        setList((now) => now.filter((x) => x.id !== id));
        props.toast?.error(t('phone.add.failed', { name: c.name }));
      },
    );
  };
  return (
    <>
      <TopAppBar
        kind="root"
        title={t('phone.computers.title')}
        part="phone.computers.bar"
        // an icon in the bar (M3, HIG: `+`), its name for screen readers
        actions={[
          {
            id: 'add',
            label: t('phone.add.title'),
            icon: 'plus',
            priority: 'pin',
            onSelect: () => setAdding(true),
            render: () => <IconButton icon="plus" label={t('phone.add.title')} onClick={() => setAdding(true)} part="phone.computers.add-bar" />,
          },
        ]}
      />
      <Scroll grow part="phone.computers" aria-label={t('phone.computers.title')}>
        <Show
          when={rows().length > 0}
          fallback={
            <EmptyState
              icon="laptop"
              title={t('phone.computers.empty')}
              action={
                <Button variant="primary" icon="plus" part="phone.computers.add" onClick={() => setAdding(true)}>
                  {t('phone.add.title')}
                </Button>
              }
            >
              {t('phone.computers.emptyHow')}
            </EmptyState>
          }
        >
          <Section title={t('phone.computers.saved')} description={t('phone.computers.help')}>
            <For each={rows()}>{(row) => <Row client={props.client} row={row} deps={props} t={t} />}</For>
          </Section>
        </Show>
        <Show when={props.stay && stayHere() && needsStay(list())}>
          <Section title={t('phone.stay.section')}>
            <ListRow
              part="phone.stay"
              title={t('phone.stay.title')}
              sub={t('phone.stay.help')}
              actions={<Switch aria-label={t('phone.stay.title')} checked={stayOn()} onChange={(on) => props.stay?.set(on)} part="phone.stay.switch" />}
            />
          </Section>
        </Show>
      </Scroll>
      <AddSheet open={adding()} onOpenChange={setAdding} deps={props} t={t} add={add} />
    </>
  );
}

export default Computers;
