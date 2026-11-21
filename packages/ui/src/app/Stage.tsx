// The app's frame, as v1 draws it (web/css/split.css, crew.css, layout.css): the crew rail on the
// window's edge, the sidebar as a raised panel beside it (its head: the brand, Search, the avatar,
// the sidebar button and the project chips; its body: New task and the list), the main pane, and the
// grip on the sidebar's edge. Split (≥ 900 px) shows them side by side; stack shows one page.
// Every element carries v1's PARTS name (web/js/spell-schema.js) as its data-part.
import { createEffect, createMemo, createSignal, For, on, Show, type JSX } from 'solid-js';
import { flag, type PartName } from '../core/props.ts';
import { reorderable, type OnReorder } from '../core/reorder.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Spirit } from '../crew/Spirit.tsx';
import { WithAct, type Act } from './act.tsx';
import { useRailCard } from './RailCard.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { ProjectSelect } from './ProjectSelect.tsx';
import { phoneStage } from './phone-stage-load.ts';
import { drag } from './grip-drag.ts';

export type StagePage = 'list' | 'main';

export interface StageProps {
  readonly layout: 'split' | 'stack';
  /** stack only: the page on screen */
  readonly page: StagePage;
  readonly rail?: JSX.Element;
  readonly head: JSX.Element;
  readonly list: JSX.Element;
  readonly main: JSX.Element;
  /** v1 #panel: the pane beside the open task (Changes, Shell, PR), the window's full height at its right edge.
   *  Stacked (a phone), it covers the task as its own page, closed with its own close. */
  readonly side?: JSX.Element;
  readonly gripLabel: string;
  /** split: the task list's width in px (--side-w; v1 wisp.sideW). Unset: the token's 340. Additive (2026-11-17). */
  readonly listWidth?: number | undefined;
  /** split: the task list hidden (the sidebar button, [; v1 body.side-hidden): the open task gets the window, the head's
   *  buttons float over its corner. Additive (2026-11-17). */
  readonly listHidden?: boolean | undefined;
  /** the list's grip dragged: the width it asks for, while dragging (`done` false) and once as it's let go (v1 split.js
   *  #splitGrip). The caller clamps it and passes it back as `listWidth`. Additive (2026-11-17). */
  readonly onListResize?: ((px: number, done: boolean) => void) | undefined;
  /** split: the side panel's width in px (--panel-w; v1 panel.js pref.w). Additive (2026-11-17). */
  readonly panelWidth?: number | undefined;
  /** the side panel's inner edge dragged (v1 .pn-grip), as `onListResize`. Additive (2026-11-17). */
  readonly onPanelResize?: ((px: number, done: boolean) => void) | undefined;
  /** a double press on the panel's edge: back to its default width */
  readonly onPanelReset?: () => void;
  readonly panelGripLabel?: string;
  /** stack, main page: the list stays laid out behind it (Home's prompt box opened from the list) */
  readonly listBehind?: boolean;
  /** the page is the window's title bar, its buttons over the top-left corner (the desktop app on a
   * Mac, v1 body.desktop.mac): room for them, and the top bar, rail and task head drag the window */
  readonly titlebar?: 'mac';
  readonly onGripReset?: () => void;
  /** split with the rail: the picked crew member's colour (a crew colour name); the list and its top bar take a very
   *  light wash of it (v1 #768 .crew-tint) */
  readonly tint?: string | undefined;
  /** added 2026-11-15 (ADR-0065 step 8): the phone apps' destinations (a NavigationBar). Under a phone look the stage
   *  is the platform's Frame (the top app bar, the panes, the navigation bar or rail); the browser never draws it, and
   *  its DOM stays v1's whatever is passed here. */
  readonly nav?: JSX.Element;
  /** stack, phone frame: the page on screen is a pushed detail (a task, its side pane), not a destination's root; the
   *  navigation bar gives way under it. Additive (2026-11-18). */
  readonly pushed?: boolean;
  /** the panes' names for a screen reader, on the phone frame */
  readonly listLabel?: string;
  readonly mainLabel?: string;
  readonly sideLabel?: string;
}

/** v1 #stage: the rail, the sidebar's head and body, the main pane, the grip. In the phone apps: the kit's Frame. */
export function Stage(props: StageProps): JSX.Element {
  // the look is set on <html> before the first render and never changes: one read picks the frame. The phone frame is
  // its own chunk (the browser's first download never carries it, ADR-0011); loadPlatformStyles brings it with the look.
  if (usePlatform()() === 'web') return WebStage(props);
  void phoneStage.load().catch(() => undefined);
  return <Show when={phoneStage.ready()}>{(m) => m().PhoneStage(props)}</Show>;
}

function WebStage(props: StageProps): JSX.Element {
  // <main>'s attributes are one render effect, rerun as the page flips (each tap and Back on a phone): reading the rail
  // and the side pane there to test them built each again, the crew rail with all its faces (F-phone-8). Built once here,
  // and the rail only where it shows (split): a stack never draws it
  const rail = createMemo(() => (props.layout === 'split' ? props.rail : undefined));
  const side = createMemo(() => props.side);
  // a stack's page flip is said on the panes themselves, never by an attribute of <main> that rules read on their
  // descendants: Chrome invalidates such a rule's targets by attribute name, and every kit element has data-kit, so
  // each tap and Back restyled the whole list (F-phone-8: ~30 ms at 4×). `pane-off`: the pane isn't the page; `pane-under`:
  // Home's page over the list, which stays drawn behind it (the only case that changes the list's own descendants).
  // Names no other rule uses: a descendant rule is invalidated by any element's attribute of that name (<main>'s data-behind, a picker's data-off).
  const over = (): boolean => props.layout === 'stack' && props.page === 'main';
  const behind = (): boolean => over() && props.listBehind === true;
  // a pane going off keeps no focus: content-visibility skips it, but a control focused in it (the row just tapped, the
  // search box with the keyboard up) would stay focused, as display: none and inert didn't let it
  const off: { bar?: HTMLElement; list?: HTMLElement } = {};
  createEffect(
    on(over, (o) => {
      const focused = document.activeElement;
      if (o && focused instanceof HTMLElement && (off.bar?.contains(focused) || off.list?.contains(focused))) focused.blur();
    }),
  );
  // the widths are custom properties on the stage (the only style a feature passes: --side-w, --panel-w)
  const widths = (): Record<string, string> | undefined => {
    const o: Record<string, string> = {};
    if (props.listWidth !== undefined) o['--side-w'] = `${props.listWidth}px`;
    if (props.panelWidth !== undefined) o['--panel-w'] = `${props.panelWidth}px`;
    return Object.keys(o).length ? o : undefined;
  };
  return (
    <main
      data-kit="stage"
      data-part="shell.app"
      data-layout={props.layout}
      data-page={props.page}
      data-behind={flag(props.listBehind)}
      data-titlebar={props.titlebar}
      data-rail={flag(rail() !== undefined)}
      data-side={flag(side() !== undefined)}
      data-list-hidden={flag(props.layout === 'split' && props.listHidden === true)}
      style={widths()}
    >
      {props.layout === 'split' ? rail() : null}
      <header
        ref={(el) => (off.bar = el)}
        data-kit="topbar"
        data-part="topbar"
        data-drag={flag(props.titlebar !== undefined)}
        data-tint={flag(!!props.tint)}
        data-color={props.tint || undefined}
        data-pane-off={flag(over())}
        data-pane-under={flag(behind())}
      >
        {props.head}
      </header>
      {/* under a task's page the list is skipped (content-visibility: hidden), which already keeps it out of the Tab
          order, find in page and the accessibility tree; inert is only for Home's page, where it is drawn behind.
          Toggling inert restyles every element under it (F-phone-8) */}
      <div
        ref={(el) => (off.list = el)}
        data-kit="feed-wrap"
        data-part="sidebar"
        inert={behind()}
        data-tint={flag(!!props.tint)}
        data-color={props.tint || undefined}
        data-pane-off={flag(over())}
        data-pane-under={flag(behind())}
      >
        {props.list}
      </div>
      <section data-kit="split-main" data-part="split.main" data-pane-off={flag(props.layout === 'stack' && props.page === 'list')}>
        {props.main}
      </section>
      <Show when={side()}>
        <aside data-kit="side-pane" data-part="side">
          {side()}
        </aside>
      </Show>
      {/* v1 .pn-grip: the side panel's inner edge (beside it, not in it: the panel scrolls, and would clip it) */}
      <Show when={props.layout === 'split' && side() !== undefined}>
        <div
          data-kit="pn-grip"
          data-part="panel.grip"
          role="separator"
          aria-orientation="vertical"
          aria-label={props.panelGripLabel ?? props.gripLabel}
          onPointerDown={drag(() => props.panelWidth, -1, (px, done) => props.onPanelResize?.(px, done))}
          onDblClick={() => props.onPanelReset?.()}
        />
      </Show>
      <Show when={props.layout === 'split' && !props.listHidden}>
        <div
          data-kit="split-grip"
          data-part="sidebar.grip"
          role="separator"
          aria-orientation="vertical"
          aria-label={props.gripLabel}
          onPointerDown={drag(() => props.listWidth, 1, (px, done) => props.onListResize?.(px, done))}
          onDblClick={() => props.onGripReset?.()}
        />
      </Show>
    </main>
  );
}

export interface RailMember {
  readonly id: string;
  readonly name: string;
  readonly avatar: string;
  /** the member's colour (v1 c-<colour>) */
  readonly color: string;
}

export interface CrewRailProps {
  readonly label: string;
  readonly allLabel: string;
  readonly orgLabel: string;
  readonly addLabel: string;
  /** the list beside it shows every task (All picked) */
  readonly allOn: boolean;
  /** Org is open beside it: its button is the picked tile (v1 .cl-org.on) */
  readonly orgOn?: boolean | undefined;
  /** the member whose thread is open: their face is the picked tile (v1 .cl-face.on) */
  readonly pickedId?: string | undefined;
  readonly members: readonly RailMember[];
  readonly onAll?: () => void;
  readonly onMember?: (id: string) => void;
  readonly onOrg?: () => void;
  readonly onAdd?: () => void;
  /** added 2026-11-12: a face's card (v1 .cl-card): what's in it for a member, and its name for a screen reader */
  readonly card?: ((id: string) => JSX.Element) | undefined;
  readonly cardLabel?: ((id: string) => string) | undefined;
}

/** `pressed`: a toggle (Org: aria-pressed, no pip) rather than where you are (All: aria-current and its pip). */
function RailBtn(props: { readonly icon: IconName; readonly label: string; readonly part: string; readonly kind: string; readonly on?: boolean; readonly pressed?: boolean; readonly onClick?: () => void }): JSX.Element {
  return (
    <button
      type="button"
      data-kit="cl-btn"
      data-a={props.kind}
      data-part={props.part}
      data-on={flag(props.on || props.pressed)}
      aria-label={props.label}
      title={props.label}
      aria-current={props.on ? 'page' : undefined}
      aria-pressed={props.pressed}
      onClick={() => props.onClick?.()}
    >
      <Show when={props.on !== undefined}>
        <i data-kit="cl-pip" data-part="crew.rail.pick" />
      </Show>
      <Icon name={props.icon} />
    </button>
  );
}

/** v1 #crewRail: All, the members' faces, then Org and + at the foot. */
export function CrewRail(props: CrewRailProps): JSX.Element {
  let rail: HTMLElement | undefined;
  const card = useRailCard(() => rail);
  const on = (): boolean => props.card !== undefined;
  return (
    <nav
      ref={(el) => (rail = el)}
      data-kit="cl-rail"
      data-part="crew.rail"
      aria-label={props.label}
      data-drag=""
      onPointerMove={(e) => on() && card.onRailMove(e)}
      onFocusOut={(e) => on() && !rail?.contains(e.relatedTarget as Node | null) && card.onRailBlur()}
    >
      <RailBtn icon="feed" label={props.allLabel} part="crew.rail.all" kind="all" on={props.allOn} onClick={() => props.onAll?.()} />
      <div data-kit="cl-faces">
        <For each={props.members}>
          {(m) => (
            <button
              type="button"
              data-kit="cl-face"
              data-part="crew.rail.face"
              data-id={m.id}
              data-on={flag(props.pickedId === m.id)}
              aria-current={props.pickedId === m.id ? 'page' : undefined}
              aria-label={m.name}
              title={on() ? undefined : m.name}
              onClick={() => {
                card.hide();
                props.onMember?.(m.id);
              }}
              onPointerEnter={(e) => on() && card.onFaceEnter(m.id, e)}
              onPointerLeave={(e) => on() && card.onFaceLeave(e)}
              onFocus={(e) => on() && card.onFaceFocus(m.id, e)}
              onKeyDown={(e) => on() && card.onFaceKey(m.id, e)}
            >
              <i data-kit="cl-pip" data-part="crew.rail.pick" />
              <span data-kit="cl-av">
                <span data-kit="cr-av" data-part="crew.avatar" data-color={m.color}>
                  <Spirit code={m.avatar} seed={m.name} />
                </span>
              </span>
            </button>
          )}
        </For>
      </div>
      <Show when={props.card}>{(content) => <card.Card label={(id) => props.cardLabel?.(id) ?? id} content={content()} />}</Show>
      <div data-kit="cl-foot">
        <RailBtn icon="org" label={props.orgLabel} part="crew.org" kind="org" pressed={props.orgOn === true} onClick={() => props.onOrg?.()} />
        <RailBtn icon="plus" label={props.addLabel} part="crew.add" kind="add" onClick={() => props.onAdd?.()} />
      </div>
    </nav>
  );
}

export interface ProjectChip {
  readonly id: string;
  readonly name: string;
  readonly title: string;
  /** a project colour (v1's c-<colour>); undefined for All */
  readonly color?: string;
  readonly need: number;
  readonly needLabel: string;
}

export interface SidebarHeadProps {
  readonly brandLabel: string;
  readonly searchLabel: string;
  readonly accountLabel: string;
  readonly viewLabel: string;
  /** the task list is shown (v1 .view-btn[aria-pressed]); unset: not said. Additive (2026-11-17). */
  readonly viewPressed?: boolean | undefined;
  readonly manageLabel: string;
  readonly chips: readonly ProjectChip[];
  /** the chip on (All is '') */
  readonly on: string;
  /** one selector instead of the row of chips (owner, 2026-11-12): the project on, a ▾, and this opens the switcher */
  readonly onSwitch?: () => void;
  /** the selector's tip ("Switch project ⌘P") */
  readonly switchLabel?: string;
  /** added 2026-11-14: several projects picked: the selector names them ("api + web", "4 projects") over their stacked
   *  dots (three at most), the need-you counts summed, the tip listing every one */
  readonly pick?: { readonly colors: readonly string[]; readonly label: string; readonly tip: string; readonly need: number; readonly needLabel: string };
  readonly onBrand?: () => void;
  /** Home is open: the brand (which goes there) is where you are */
  readonly homeOn?: boolean;
  readonly onSearch?: () => void;
  readonly onAccount?: () => void;
  readonly onView?: () => void;
  readonly onChip?: (id: string) => void;
  /** drag a project chip (or Alt+←/→) into your own order (v1 #772); All stays first */
  readonly onReorder?: OnReorder;
  readonly onManage?: () => void;
  readonly accountAct?: Act;
  /** you on the grid, in the avatar (v1 split.js paintMe): your initial (and picture), ringed while the grid is reachable,
   *  grey when this wisp isn't connected; absent: never signed in, the person glyph */
  readonly me?: MeFace;
  readonly manageAct?: Act;
  /** what plugins put in the head (v1's spell SLOTS on #topbar): `lead` sits beside the brand, `chips`
   * after the project chips; brand, search, account and view replace the built-in one when given */
  readonly slots?: {
    readonly lead?: JSX.Element;
    readonly chips?: JSX.Element;
    readonly brand?: JSX.Element;
    readonly search?: JSX.Element;
    readonly account?: JSX.Element;
    readonly view?: JSX.Element;
  };
}

export interface MeFace {
  readonly initial: string;
  readonly avatar?: string;
  /** on: the grid answers; off: signed in, not reachable now; out: signed out (the last account, greyed) */
  readonly state: 'on' | 'off' | 'out';
}

/** v1 .sf-av: the picture over the initial; a picture that fails to load leaves the initial (no :has(), design/layout.md) */
function MeAvatar(props: { readonly me: MeFace | undefined }): JSX.Element {
  const [broken, setBroken] = createSignal<string | undefined>(undefined);
  return (
    <Show when={props.me} fallback={<Icon name="user" />}>
      {(m) => (
        <Show when={m().avatar && broken() !== m().avatar ? m().avatar : undefined} fallback={<b>{m().initial}</b>}>
          {(src) => <img src={src()} alt="" referrerpolicy="no-referrer" onError={() => setBroken(src())} />}
        </Show>
      )}
    </Show>
  );
}

/** v1's row of project chips (draggable into your order): what the head shows without a switcher. */
function Chips(props: SidebarHeadProps & { readonly projectIds: () => string[] }): JSX.Element {
  return (
    <div data-kit="p-scroll">
      <For each={props.chips}>
        {(c) => (
          <button
            type="button"
            {...reorderable('topbar.projects', c.id, props.projectIds, c.color === undefined ? undefined : props.onReorder)}
            data-kit="p-chip"
            data-on={flag(props.on === c.id)}
            aria-current={props.on === c.id ? 'true' : undefined}
            data-color={c.color ?? 'slate'}
            title={c.title}
            onClick={() => props.onChip?.(c.id)}
          >
            <Show when={c.color}>
              <i data-kit="p-dot" />
            </Show>
            <span data-text="control">{c.name}</span>
            <Show when={c.need > 0}>
              <span data-kit="p-need" title={c.needLabel}>
                {c.need}
              </span>
            </Show>
          </button>
        )}
      </For>
    </div>
  );
}

/** v1 #topbar in split: the brand, Search, the avatar, the sidebar button, then the project chips. */
export function SidebarHead(props: SidebarHeadProps): JSX.Element {
  const projectIds = (): string[] => props.chips.filter((c) => c.color !== undefined).map((c) => c.id);
  return (
    <>
      <div data-kit="tb-lead">
        {props.slots?.brand ?? (
          <a
            data-kit="brand"
            data-part="topbar.brand"
            href="#"
            aria-current={props.homeOn ? 'page' : undefined}
            title={props.brandLabel}
            aria-label={props.brandLabel}
            onClick={(e) => (e.preventDefault(), props.onBrand?.())}
          >
            <span data-kit="brand-orb" />
          </a>
        )}
        {props.slots?.lead}
      </div>
      <div data-kit="projects" data-part="topbar.projects" data-no-drag="">
        <Show when={props.onSwitch} fallback={<Chips {...props} projectIds={projectIds} />}>
          {(open) => {
            const cur = (): ProjectChip | undefined => props.chips.find((c) => c.id === props.on) ?? props.chips[0];
            const name = (): string | undefined => props.pick?.label ?? cur()?.name;
            const need = (): number => props.pick?.need ?? cur()?.need ?? 0;
            const tip = (): string | undefined => (props.pick ? `${props.pick.tip}\n${props.switchLabel ?? ''}`.trim() : props.switchLabel);
            return (
              <ProjectSelect
                part="topbar.project-select"
                where="head"
                label={name() ?? ''}
                colors={props.pick ? props.pick.colors : cur()?.color ? [cur()?.color ?? 'slate'] : []}
                need={need()}
                needLabel={props.pick?.needLabel ?? cur()?.needLabel}
                title={tip()}
                ariaLabel={props.pick ? `${props.pick.label} · ${props.switchLabel ?? ''}` : props.switchLabel}
                onOpen={() => open()()}
              />
            );
          }}
        </Show>
        {props.slots?.chips}
        {/* v1's + (Manage projects) only when given something to do: the app's is in the switcher now (docs/ux/2026-11-18 #11) */}
        <Show when={props.manageAct || props.onManage}>
          <WithAct
            act={props.manageAct ?? { onClick: () => props.onManage?.() }}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="p-chip" data-add="" title={props.manageLabel} aria-label={props.manageLabel}>
                <Icon name="plus" />
              </button>
            )}
          />
        </Show>
      </div>
      <div data-kit="tb-view">
        {props.slots?.view ?? (
          <button
            type="button"
            data-kit="view-btn"
            data-part="topbar.view"
            title={props.viewLabel}
            aria-label={props.viewLabel}
            aria-pressed={props.viewPressed === undefined ? undefined : props.viewPressed}
            onClick={() => props.onView?.()}
          >
            <Icon name="split" />
          </button>
        )}
      </div>
      <div data-kit="tb-search">
        {props.slots?.search ?? (
          <button type="button" data-kit="search-btn" data-part="topbar.search" title={props.searchLabel} aria-label={props.searchLabel} onClick={() => props.onSearch?.()}>
            <Icon name="search" />
          </button>
        )}
      </div>
      <div data-kit="tb-account">
        {props.slots?.account ?? (
          <WithAct
            act={props.accountAct ?? { onClick: () => props.onAccount?.() }}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="me-btn" data-part="sidebar.foot" title={props.accountLabel} aria-label={props.accountLabel}>
                <span data-kit="sf-av" data-part="topbar.account" data-state={props.me?.state}>
                  <MeAvatar me={props.me} />
                </span>
              </button>
            )}
          />
        )}
      </div>
    </>
  );
}

export interface SplitTopProps {
  readonly newLabel: string;
  readonly newTitle: string;
  readonly stuffLabel: string;
  /** Home is open beside it */
  readonly on: boolean;
  /** My Stuff is open beside it (v1 .split-stuff.on) */
  readonly stuffOn?: boolean;
  readonly onNew?: () => void;
  readonly onStuff?: () => void;
}

/** v1 .split-top: New task (a small prompt box) and My Stuff. */
export function SplitTop(props: SplitTopProps): JSX.Element {
  return (
    <div data-kit="split-top">
      <div data-kit="st-row">
        <button type="button" data-kit="split-new" data-part="sidebar.new" data-on={flag(props.on)} aria-current={props.on ? 'page' : undefined} title={props.newTitle} onClick={() => props.onNew?.()}>
          <Icon name="plus" />
          <span data-text="control">{props.newLabel}</span>
          <kbd>N</kbd>
        </button>
        <button
          type="button"
          data-kit="split-stuff"
          data-part="stuff.button"
          data-on={flag(props.stuffOn)}
          aria-current={props.stuffOn ? 'page' : undefined}
          title={props.stuffLabel}
          aria-label={props.stuffLabel}
          onClick={() => props.onStuff?.()}
        >
          <Icon name="stuff" />
        </button>
      </div>
    </div>
  );
}

export interface FeedSectionProps {
  /** v1's section id: needs, working, done … (its part is section.<id>) */
  readonly id: string;
  readonly name: string;
  readonly count: number;
  readonly collapsed?: boolean;
  readonly onToggle?: () => void;
  readonly children?: JSX.Element;
}

/** v1 #feed .item: one task's row in a feed, its 1px border (transparent at rest) around the row's head. */
/** v1 #feed .item round a row: open (current) it's filled, picked by the keyboard (selected) it's outlined. */
export function FeedItem(props: { readonly part?: PartName; readonly id?: string; readonly current?: boolean; readonly selected?: boolean; readonly children?: JSX.Element }): JSX.Element {
  return (
    <div data-kit="feed-item" data-part={props.part ?? 'ui.feed-item'} data-id={props.id} data-current={flag(props.current)} data-selected={flag(props.selected)}>
      {props.children}
    </div>
  );
}

/** v1 #feed: the sections, each a heading that folds it and its rows. */
export function Feed(props: { readonly children?: JSX.Element }): JSX.Element {
  return (
    <div data-kit="feed" data-part="feed">
      {props.children}
    </div>
  );
}

export function FeedSection(props: FeedSectionProps): JSX.Element {
  return (
    <section data-kit="f-group" data-part={`section.${props.id}`} data-collapsed={flag(props.collapsed)}>
      <h4 data-kit="sec-toggle" data-part="section.heading" role="button" tabindex="0" aria-expanded={!props.collapsed} onClick={() => props.onToggle?.()}>
        <Icon name="chevron" />
        <span data-kit="sec-name" data-text="control">
          {props.name}
        </span>
        <span data-kit="sec-n">{props.count}</span>
      </h4>
      <Show when={!props.collapsed}>
        <div data-kit="f-list">{props.children}</div>
      </Show>
    </section>
  );
}
