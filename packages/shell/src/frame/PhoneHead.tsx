// The Tasks screen's head in the phone apps (ADR-0065 step 13, design/mobile.md §A.1, §A.2): the kit's root TopAppBar
// (M3 small top app bar on Android; the large title that collapses on iOS) in place of v1's desktop head (logo,
// Machines, the All pill, New task row). Its title is the computer filter (*All* or a computer's name, a sheet to pick),
// its Search (the palette over tasks: the M3 search view / the iOS search field) and the me menu (v1 #meMenu: you, Machines, Automations, Settings, the
// theme), and the one project selector (owner, 2026-11-15: no chip row of projects on phones either), which opens the
// same ⌘P list (tap = only, + adds). Under it, in the list, the crew's stories row (`shell.list:crew`, step 13b) and
// New task as the FAB (Android) / the bar's compose button (iOS: the kit hands a Fab's action to the root bar). Its
// own chunk: only the phone apps load it, so the
// browser's first download is unchanged.
import { createMemo, createSignal, onCleanup, onMount, Show, type JSX } from 'solid-js';
import { Button, Dialog, Fab, Inline, Input, Menu, ProjectSelect, RadioGroup, SelectionBar, Sheet, TopAppBar, useSignal, type MenuEntry, type SelectionAction, type TaskSwipe, type ToolbarItem, type TopAppBarSelection } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { Slot } from '../slot.tsx';
import '../i18n/lazy.ts';
import { displayCombo } from '../keys.ts';
import { computerOptions, computerTitle, hasComputerFilter, knownComputer } from './computer-filter.ts';
import { openSwitcher } from './project-switch.ts';
import { useAccount, useHeadChips } from './Sidebar.tsx';
import { computed } from '@wisp/client';
import { SESSION, type SessionRow } from '../rows.ts';
import { allSelected } from './list-select.ts';
import { BULK_ACTIONS, bulkTargets, runBulk, type BulkAction } from './bulk.ts';

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
    /** iOS's pull-down: the same choices, the one shown checked */
    menu: (): MenuEntry[] => {
      const on = knownComputer(pick(), peers());
      return computerOptions(pick(), peers(), name, all()).map((o) => ({ value: o.id, label: o.label, checked: o.id === on }));
    },
    on: (): string => knownComputer(pick(), peers()),
    /** more than one computer: the title is the filter */
    filters: (): boolean => hasComputerFilter(peers()),
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
  const selecting = useSignal(shell.listSelect?.on ?? (() => false));
  const picked = usePicked();
  // one memo: the bar's props read it, not the list (allSelected walks the list's order once per change)
  const selection = createMemo((): TopAppBarSelection | undefined =>
    selecting() ? { count: picked.count(), allPicked: picked.all(), onAll: () => shell.listSelect?.all(), onExit: () => shell.listSelect?.exit(), part: 'phone.tasks.select' } : undefined,
  );
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
    // selection mode's visible way in (the twin of a row's long-press menu's Select): iOS's Select, Android's in ⋮
    ...(shell.listSelect ? [{ id: 'select', label: shell.t('shell.phone.select'), icon: 'check' as const, priority: 1, onSelect: () => shell.listSelect?.start() }] : []),
  ]);
  // selection mode: the bar takes `selection` and the selection bar comes in; the kit picks what shows (iOS's edit mode
  // keeps this bar, its large title and its folded state; Android's contextual bar takes its place at its height)
  return (
    <>
      <TopAppBar
        kind="root"
        part="phone.tasks.bar"
        // one computer: nothing to narrow, the screen's name (QA round 2 #10: "All ▾" beside "All projects ▾")
        title={computers.filters() ? computers.title() : shell.t('shell.dest.tasks')}
        // iOS: the computer filter is a pull-down from the title (UIMenu), its choices checked; Android keeps its sheet
        {...(computers.filters() && {
          onTitle: () => setPicking(true),
          titleMenu: { label: shell.t('shell.phone.pickComputer'), items: computers.menu(), onSelect: (id: string) => computers.set(id) },
        })}
        search={{ label: shell.t('shell.phone.search'), onOpen: () => shell.palette.open() }}
        actions={actions()}
        actionsLabel={shell.t('shell.phone.tasksActions')}
        {...(selection() && { selection: selection() as TopAppBarSelection })}
        t={(k, v) => shell.t(`kit.${k}`, v)}
      />
      <Show when={selecting()}>
        <PhoneSelectBar picked={picked} />
      </Show>
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

/** Delete on several tasks asks first, with how many (as a row's Delete asks). */
function BulkDeleteAsk(props: { readonly count: number; readonly onCancel: () => void; readonly onDelete: () => void }): JSX.Element {
  const t = useShell().t;
  return (
    <Dialog
      open
      role="alertdialog"
      size="sm"
      title={t('shell.select.ask.title', { count: props.count })}
      description={t('shell.select.ask.body')}
      onOpenChange={(open) => !open && props.onCancel()}
      part="phone.tasks.select.ask"
      t={(k) => t(`kit.${k}`)}
      footer={
        <Inline gap="4" justify="end">
          <Button variant="ghost" onClick={() => props.onCancel()}>
            {t('shell.select.ask.cancel')}
          </Button>
          <Button variant="danger" part="phone.tasks.select.ask.ok" onClick={() => props.onDelete()}>
            {t('shell.select.ask.ok')}
          </Button>
        </Inline>
      }
    />
  );
}

/** Esc (a keyboard, Android's Back through the platform) leaves selection, unless something over it took the key. */
function escLeaves(exit: () => void, held: () => boolean): void {
  onMount(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape' && !e.defaultPrevented && !held()) exit();
    };
    document.addEventListener('keydown', onKey);
    onCleanup(() => document.removeEventListener('keydown', onKey));
  });
}

/** What the list has picked: how many, and whether it is every row the list shows. */
function usePicked() {
  const shell = useShell();
  const keys = useSignal(shell.listSelect?.keys ?? (() => [] as readonly string[]));
  const order = useSignal(shell.listOrder?.keys ?? (() => [] as readonly string[]));
  const all = createMemo(() => allSelected({ on: true, keys: keys() }, order()));
  return { keys, count: (): number => keys().length, all };
}
type Picked = ReturnType<typeof usePicked>;

const BULK_ICON = { done: 'done', later: 'bookmark', archive: 'archive', delete: 'trash' } as const;
const BULK_LABEL = { done: 'shell.select.markDone', later: 'shell.select.later', archive: 'shell.select.archive', delete: 'shell.select.delete' } as const;

// The Tasks list's selection bar (ADR-0065 step 13, design/mobile.md §A.4): the kit's SelectionBar, Android's contextual
// top app bar in place of the Tasks bar while the list picks rows; on iOS only its bottom toolbar, the Tasks bar itself
// being in edit mode (TopAppBar `selection`), so the list under it never moves. Its bulk
// actions are the rows' own (bulk.ts): each picked row changes at once, a refusal rolls back with a toast; Delete asks
// first, as a row's Delete does. Back (Esc, Android's ×, iOS's Done) leaves.
function PhoneSelectBar(props: { readonly picked: Picked }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const sel = shell.listSelect;
  const picked = props.picked;
  const keys = picked.keys;
  const [asking, setAsking] = createSignal<readonly string[]>();
  const rowOf = (k: string): SessionRow | undefined => shell.client.entities.peek<SessionRow>(SESSION, k as never) as SessionRow | undefined;
  // which bulk actions some picked task can take: it watches the picked rows only, never the whole list (5k rows)
  const enabled = useSignal(
    computed(() => {
      const ks = sel?.keys() ?? [];
      const rows = new Map(ks.map((k) => [k, shell.client.entities.row<SessionRow>(SESSION, k as never)()]));
      return new Set(BULK_ACTIONS.filter((a) => bulkTargets(a, ks, (k) => rows.get(k)).length > 0));
    }),
  );
  const exit = (): void => sel?.exit();
  const act = (a: BulkAction): void => {
    const targets = bulkTargets(a, keys(), rowOf);
    if (!targets.length) return;
    if (a === 'delete') return void setAsking(targets);
    runBulk({ client: shell.client, toast: shell.toast, t }, a, targets);
    exit();
  };
  escLeaves(exit, () => asking() !== undefined);
  const actions = createMemo((): SelectionAction[] =>
    BULK_ACTIONS.map((a) => ({ id: a, label: t(BULK_LABEL[a]), icon: BULK_ICON[a], onSelect: () => act(a), disabled: !enabled().has(a), ...(a === 'delete' && { danger: true }) })),
  );
  return (
    <>
      <SelectionBar
        part="phone.tasks.select"
        count={keys().length}
        allPicked={picked.all()}
        onAll={() => sel?.all()}
        onExit={exit}
        actions={actions()}
        actionsLabel={t('shell.select.actions')}
        t={(k, v) => t(`kit.${k}`, v)}
      />
      <Show when={asking()}>
        {(targets) => (
          <BulkDeleteAsk
            count={targets().length}
            onCancel={() => setAsking(undefined)}
            onDelete={() => {
              const ks = targets();
              setAsking(undefined);
              runBulk({ client: shell.client, toast: shell.toast, t }, 'delete', ks);
              exit();
            }}
          />
        )}
      </Show>
    </>
  );
}

/** Over the list: the crew's stories and New task (the projects are the bar's selector, owner 2026-11-15). */
export function PhoneListTop(props: { readonly onNew: () => void }): JSX.Element {
  const shell = useShell();
  const selecting = useSignal(shell.listSelect?.on ?? (() => false));
  return (
    <>
      {/* the crew as stories (step 13b): the crew's section, which the phone apps draw here and not in the list */}
      <Slot input="shell.list:crew" layout="stack" />
      {/* selection mode: the bar's actions take over, so New task steps aside (M3's contextual bar; iOS's edit mode) */}
      <Show when={!selecting()}>
        <Fab label={shell.t('shell.side.new')} icon="plus" onClick={props.onNew} part="phone.tasks.new" />
      </Show>
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
  /** the head's swipe to the next / previous task in the list's order (its twins: Next task / Previous task in the menu) */
  readonly swipe: TaskSwipe;
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
        swipe={props.swipe}
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
