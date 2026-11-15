// The Tasks screen's head in the phone apps (ADR-0065 step 13, design/mobile.md §A.1, §A.2): the kit's root TopAppBar
// (M3 small top app bar on Android; the large title that collapses on iOS) in place of v1's desktop head (logo,
// Machines, the All pill, New task row). Its title is the computer filter (*All* or a computer's name, a sheet to pick),
// its Search (the palette over tasks: the M3 search view / the iOS search field) and the me menu (v1 #meMenu: you, Machines, Automations, Settings, the
// theme), and the one project selector (owner, 2026-11-15: no chip row of projects on phones either), which opens the
// same ⌘P list (tap = only, + adds). Under it, in the list, the crew's stories row (`shell.list:crew`, step 13b) and
// New task as the FAB (Android) / the bar's compose button (iOS: the kit hands a Fab's action to the root bar). Its
// own chunk: only the phone apps load it, so the
// browser's first download is unchanged.
import { createMemo, createSignal, Show, type JSX } from 'solid-js';
import { Button, Dialog, Fab, Inline, Input, Menu, ProjectSelect, RadioGroup, Sheet, TopAppBar, useSignal, type MenuEntry, type ToolbarItem } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { Slot } from '../slot.tsx';
import '../i18n/lazy.ts';
import { displayCombo } from '../keys.ts';
import { computerOptions, computerTitle, knownComputer } from './computer-filter.ts';
import { openSwitcher } from './project-switch.ts';
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

/** The sidebar's selector, in the bar: the project (or the pick, or All), its need-you count; a tap opens the ⌘P list. */
function useBarProject(filter: () => string) {
  const shell = useShell();
  const head = useHeadChips(filter);
  const switchLabel = (): string => `${shell.t('shell.projects.switch')} (${displayCombo('mod+p', shell.platform.apple).join('')})`;
  return () => {
    const pk = head.pick();
    const cur = head.chips().find((c) => c.id === head.on()) ?? head.chips()[0];
    // beside the computer filter's All: All projects in full, so the two never read the same
    const label = pk?.label ?? (cur?.id === 'all' ? shell.t('shell.side.all') : (cur?.name ?? ''));
    return { label, colors: pk ? pk.colors : cur?.color ? [cur.color] : [], need: pk?.need ?? cur?.need ?? 0, needLabel: pk?.needLabel ?? cur?.needLabel, title: pk?.tip ?? cur?.title, ariaLabel: `${label} · ${switchLabel()}` };
  };
}

export function PhoneHead(props: { readonly filter: () => string }): JSX.Element {
  const shell = useShell();
  const account = useAccount();
  const computers = useComputers();
  const project = useBarProject(props.filter);
  const [picking, setPicking] = createSignal(false);
  const actions = createMemo((): ToolbarItem[] => [
    {
      id: 'project',
      label: project().label,
      icon: 'folder',
      priority: 'pin',
      render: () => <ProjectSelect {...project()} part="phone.tasks.project" where="bar" onOpen={() => openSwitcher(shell)} />,
    },
    // icons only in an app bar (M3, HIG): the title keeps its room. Search is the bar's own (`search`): Android's icon
    // opens the M3 search view, iOS's field sits under the large title; both are the palette's task search
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
        search={{ label: shell.t('shell.phone.search'), onOpen: () => shell.palette.open() }}
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

/** Over the list: the crew's stories and New task (the projects are the bar's selector, owner 2026-11-15). */
export function PhoneListTop(props: { readonly onNew: () => void }): JSX.Element {
  const shell = useShell();
  return (
    <>
      {/* the crew as stories (step 13b): the crew's section, which the phone apps draw here and not in the list */}
      <Slot input="shell.list:crew" layout="stack" />
      <Fab label={shell.t('shell.side.new')} icon="plus" onClick={props.onNew} part="phone.tasks.new" />
    </>
  );
}

// New task's bar (ADR-0065 step 13, design/mobile.md §A.2 New task): the kit's compose TopAppBar. Android draws it as the
// M3 full-screen dialog's bar (Close ×, New task, Send as a text button at its end), iOS as a sheet's (Cancel, New task,
// Send in bold); the kit's platform sheet shapes the page under it. Send is the prompt box's own send (the same checks
// and the same optimistic launch); Close / Cancel keep the draft, as Back did (v1). In this chunk with the Tasks head, so
// the browser's first download only names one phone chunk.
export function PhoneCompose(props: { readonly onClose: () => void; readonly onSend: () => void; readonly canSend: boolean }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  return (
    <TopAppBar
      kind="compose"
      part="phone.new.bar"
      title={t('shell.phone.newTask')}
      back={{ label: t('shell.phone.close'), onBack: props.onClose, close: t('shell.phone.cancel') }}
      confirm={{ label: t('shell.phone.send'), onSelect: props.onSend, disabled: !props.canSend }}
      t={(k) => t(`kit.${k}`)}
    />
  );
}

// The task page's head (ADR-0065 step 13, design/mobile.md §A.1, §A.2 Task page): the kit's page TopAppBar (M3 medium
// top app bar; the iOS navigation bar with Back titled Tasks) in place of v1's split head. Its actions are what plugins
// put in the head (`shell.task:head`) and the task's menu (⋮ / ⋯): the one menu model (TaskView's `more()`,
// task-menu.ts) with what the browser's details bar holds (Transcript, Rerun, Open in, Changes, Shell, Ship it, Mark
// done) and Details, which brings the details column up as a sheet rather than a column. Rename is a dialog here (the
// bar's title isn't edited in place). In this chunk, which the list's head has loaded already: the page's first frame
// has its bar.
export interface PhoneTaskBarProps {
  readonly title: string;
  readonly onBack: () => void;
  readonly menu: () => readonly MenuEntry[];
  readonly onSelect: (value: string) => void;
  /** the details column (TaskDetails without its actions: they are in the menu) */
  readonly details: () => JSX.Element;
  readonly detailsOpen: boolean;
  readonly onDetails: (open: boolean) => void;
  readonly renaming: boolean;
  readonly onRenaming: (open: boolean) => void;
  readonly onRename: (title: string) => void;
  /** what plugins put in the task's head (`shell.task:head`), before the menu */
  readonly head: () => JSX.Element;
}

export function PhoneTaskBar(props: PhoneTaskBarProps): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const kit = (k: string): string => t(`kit.${k}`);
  return (
    <>
      <TopAppBar
        kind="page"
        part="phone.task.bar"
        title={props.title}
        back={{ label: t('shell.dest.tasks'), onBack: props.onBack }}
        actions={[{ id: 'head', label: t('shell.phone.taskHead'), icon: 'dots', priority: 'pin', render: () => props.head() }]}
        menu={{ label: t('shell.task.more'), items: props.menu(), onSelect: props.onSelect }}
        t={kit}
      />
      <Sheet title={t('shell.phone.details')} open={props.detailsOpen} onOpenChange={props.onDetails} part="phone.task.details" t={kit}>
        {props.details()}
      </Sheet>
      <Show when={props.renaming}>
        <RenameDialog title={props.title} onClose={() => props.onRenaming(false)} onRename={props.onRename} />
      </Show>
    </>
  );
}

function RenameDialog(props: { readonly title: string; readonly onClose: () => void; readonly onRename: (title: string) => void }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const [value, setValue] = createSignal(props.title);
  const save = (): void => {
    const next = value().replace(/\s+/g, ' ').trim();
    props.onClose();
    if (next && next !== props.title) props.onRename(next);
  };
  return (
    <Dialog
      open
      size="sm"
      title={t('shell.more.rename')}
      onOpenChange={(open) => !open && props.onClose()}
      part="phone.task.rename"
      t={(k) => t(`kit.${k}`)}
      footer={
        <Inline gap="4" justify="end">
          <Button variant="ghost" onClick={() => props.onClose()}>
            {t('shell.stop.cancel')}
          </Button>
          <Button variant="primary" part="phone.task.rename.save" onClick={save}>
            {t('shell.phone.save')}
          </Button>
        </Inline>
      }
    >
      <Input
        value={value()}
        aria-label={t('shell.more.rename')}
        onInput={setValue}
        onKeyDown={(e: KeyboardEvent) => {
          if (e.key === 'Enter' && !e.isComposing) {
            e.preventDefault();
            save();
          }
        }}
      />
    </Dialog>
  );
}

export default PhoneHead;
