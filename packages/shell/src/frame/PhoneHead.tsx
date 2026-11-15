// The Tasks screen's head in the phone apps (ADR-0065 step 13, design/mobile.md §A.1, §A.2): the kit's root TopAppBar
// (M3 small top app bar on Android; the large title that collapses on iOS) in place of v1's desktop head (logo,
// Machines, the All pill, New task row). Its title is the computer filter (*All* or a computer's name, a sheet to pick),
// its actions Search (the palette over tasks) and the me menu (v1 #meMenu: you, Machines, Automations, Settings, the
// theme). Under it, in the list, v1's project chips as one row and New task as the FAB (Android) / the bar's compose
// button (iOS: the kit hands a Fab's action to the root bar). Its own chunk: only the phone apps load it, so the
// browser's first download is unchanged.
import { createMemo, createSignal, type JSX } from 'solid-js';
import { Button, Fab, IconButton, Menu, RadioGroup, Sheet, Toolbar, TopAppBar, useSignal, type ToolbarItem } from '@wisp/ui';
import { useShell } from '../context.tsx';
import '../i18n/lazy.ts';
import { computerOptions, computerTitle, knownComputer } from './computer-filter.ts';
import { useAccount, useHeadChips } from './Sidebar.tsx';

/** The computer filter, from the client's peers (every saved computer) and the device setting. */
function useComputers() {
  const shell = useShell();
  const peers = useSignal(shell.client.peers.list());
  const pick = useSignal(shell.computerFilter?.value ?? (() => 'all'));
  const name = (p: string): string => shell.client.peers.name(p as never);
  const all = (): string => shell.t('shell.phone.allComputers');
  return {
    title: (): string => computerTitle(pick(), peers(), name, all()),
    options: () => computerOptions(pick(), peers(), name, all()),
    on: (): string => knownComputer(pick(), peers()),
    set: (id: string): void => shell.computerFilter?.set(id),
  };
}

export function PhoneHead(): JSX.Element {
  const shell = useShell();
  const account = useAccount();
  const computers = useComputers();
  const [picking, setPicking] = createSignal(false);
  const actions = createMemo((): ToolbarItem[] => [
    // icons only in an app bar (M3, HIG): the title keeps its room
    {
      id: 'search',
      label: shell.t('shell.phone.search'),
      icon: 'search',
      priority: 'pin',
      onSelect: () => shell.palette.open(),
      render: () => <IconButton icon="search" label={shell.t('shell.phone.search')} onClick={() => shell.palette.open()} part="phone.tasks.search" />,
    },
    {
      id: 'me',
      label: account.label(),
      icon: 'user',
      priority: 'pin',
      render: () => <Menu trigger={{ icon: 'user', 'aria-label': account.label() }} items={account.items()} onSelect={account.onSelect} placement="bottom-end" part="phone.tasks.me" />,
    },
  ]);
  return (
    <>
      <TopAppBar
        kind="root"
        part="phone.tasks.bar"
        title={computers.title()}
        onTitle={() => setPicking(true)}
        actions={actions()}
        actionsLabel={shell.t('shell.phone.tasksActions')}
        t={(k) => shell.t(`kit.${k}`)}
      />
      <Sheet title={shell.t('shell.phone.pickComputer')} open={picking()} onOpenChange={setPicking} part="phone.tasks.computers" t={(k) => shell.t(`kit.${k}`)}>
        <RadioGroup
          label={shell.t('shell.phone.pickComputer')}
          hideLabel
          name="computer-filter"
          options={computers.options().map((o) => ({ value: o.id, label: o.label }))}
          value={computers.on()}
          onChange={(id) => {
            computers.set(id);
            setPicking(false);
          }}
        />
      </Sheet>
    </>
  );
}

/** Over the list: v1's project chips as one row (it never wraps: the last fold into More), and New task. */
export function PhoneListTop(props: { readonly onNew: () => void; readonly filter: () => string; readonly setFilter: (id: string) => void }): JSX.Element {
  const shell = useShell();
  const head = useHeadChips(props.filter);
  // in your order from the start (a pinned item would push the row to the end); the last ones fold into More first, All
  // last. Each chip is its name in both forms: as icons every project would be the same folder, so they go to More
  const items = createMemo((): ToolbarItem[] => {
    const all = head.chips();
    return all.map((c, i) => {
      const label = c.id === 'all' || c.need === 0 ? c.name : `${c.name} · ${c.need}`;
      const on = c.id === 'all' ? head.on() === 'all' : head.pickIds().includes(c.id);
      const pick = (): void => props.setFilter(c.id);
      return {
        id: c.id,
        label,
        icon: c.id === 'all' ? 'grid' : 'folder',
        priority: c.id === 'all' ? all.length + 1 : all.length - i,
        pressed: on,
        variant: 'ghost',
        onSelect: pick,
        render: () => (
          <Button variant="ghost" size="sm" pressed={on} onClick={pick} part="phone.tasks.project">
            {label}
          </Button>
        ),
      };
    });
  });
  return (
    <>
      {/* one project (All only): nothing to pick, no row */}
      {items().length > 1 ? <Toolbar aria-label={shell.t('shell.phone.projects')} items={items()} justify="start" compact gap="1" part="phone.tasks.projects" t={(k) => shell.t(`kit.${k}`)} /> : null}
      <Fab label={shell.t('shell.side.new')} icon="plus" onClick={props.onNew} part="phone.tasks.new" />
    </>
  );
}

export default PhoneHead;
