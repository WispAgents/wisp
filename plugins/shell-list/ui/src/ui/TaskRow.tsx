// One task's row, by key: it reads its own row signal, so a token streaming into this task repaints
// this row and nothing else (plan §4.6.9). Opening, selecting or hovering it moves nothing in it
// (fixed columns, gaps for the actions it lacks). On the phone app it swipes (design/mobile.md).

import { createMemo, Show, type JSX } from 'solid-js';
import { FeedItem, ListRow, Swipe, useSignal, type SwipeDir, type SwipeSide } from '@wisp/ui';
import type { EntityKey } from '@wisp/client';
import { agentName, rowView, secondLine } from '../model/row.ts';
import { actionFor, type SwipeKind } from '../model/swipe.ts';
import type { SessionRow } from '../types.ts';
import { useEnv } from './env.ts';
import { runSwipe } from './swipe-run.ts';
import { Actions, HarnessMark, Mark, Pills, RunLine, SubLine, TimeCell, subOf } from './RowParts.tsx';
import { Slot, hasExtensions } from './Slot.tsx';
import '../i18n/lazy.ts';

export interface SwipeConfig {
  readonly left: SwipeKind;
  readonly right: SwipeKind;
}

export interface TaskRowProps {
  readonly k: EntityKey;
  /** a run of a compare task, or a subagent under its lead: no swipe; a subagent has no quick actions */
  readonly nested?: 'run' | 'sub';
  /** the swipe's actions (a plugin may pick archive or delete) */
  readonly swipe?: SwipeConfig;
}

const DEFAULT_SWIPE: SwipeConfig = { left: 'later', right: 'done' };

export function TaskRow(props: TaskRowProps): JSX.Element {
  const env = useEnv();
  const row = useSignal(env.model.row(props.k));
  return <Show when={row()}>{(s) => <Body k={props.k} s={s()} nested={props.nested} swipe={props.swipe ?? DEFAULT_SWIPE} />}</Show>;
}

/** v1's PARTS for the task row's pieces (web/js/spell-schema.js: card, card.dot, card.title, …). */
const V1_ROW = { title: 'card.title', sub: 'card.sub', pills: 'card.side' } as const;

/** How many quick actions the row shows (Save for later, Mark done): its title makes room for them. */
const quickCount = (v: { readonly quickLater: string; readonly quickDone: string }): 0 | 1 | 2 => ((v.quickLater !== 'gap' ? 1 : 0) + (v.quickDone === 'show' ? 1 : 0)) as 0 | 1 | 2;

function Body(props: { readonly k: EntityKey; readonly s: SessionRow; readonly nested: 'run' | 'sub' | undefined; readonly swipe: SwipeConfig }): JSX.Element {
  const env = useEnv();
  const t = env.shell.t;
  const open = (): boolean => env.open() === props.k;
  // the phone list's selection mode (ADR-0065 step 13): a tap picks the row, the row shows its check, no swipe
  const picking = useSignal(env.shell.listSelect?.on ?? (() => false));
  const pickedKeys = useSignal(env.shell.listSelect?.keys ?? (() => [] as readonly string[]));
  const picked = (): boolean | undefined => (picking() ? pickedKeys().includes(props.k) : undefined);
  const view = createMemo(() => rowView(props.s, env.rowCtx(props.k)));
  // extension slots only where something fills them: v1's row has no such elements (qa/parity.mjs compares the tree)
  const bottom = hasExtensions('shell.row:bottom');
  const head = hasExtensions('shell.row:head');
  const hover = hasExtensions('shell.row:hover');
  const subLook = createMemo(() => subOf(view().sub, env.shell.t, env.now()));
  // two lines and the slot's actions: a desktop's list (the phone apps swipe)
  // two lines on every look (desktop #2; phones too, docs/ux/2026-11-18-phone.md #10): a run's own nested row keeps three
  const two = (): boolean => props.nested !== 'run';
  const line = createMemo(() => secondLine(view()));
  /** the run line is in the second line already: with the answer's words (two lines), or alone on a phone */
  const runInSub = (): boolean => (two() && line().line === 'sub') || (env.swipes === true && line().line === 'run');
  const name = (): string => agentName(props.s.agent, env.agents());
  const session = (): { id: string; agent: string } => ({ ...props.s, id: props.k, agent: props.s.agent });
  const side = (kind: SwipeKind): SwipeSide | undefined => {
    const a = actionFor(kind, props.s);
    return a ? { label: t(a.label), icon: a.icon, tone: a.tone } : undefined;
  };
  // the side's action, as the menu runs it; Delete task asks first (swipe-run.ts)
  const swiped = (dir: SwipeDir): void => runSwipe(dir === 'left' ? props.swipe.left : props.swipe.right, env, props.k);
  const list = (): JSX.Element => (
    <ListRow
      layout="feed"
      // v1's own name for a task row's head (v1-parts.json: card.head ↔ #feed .it-head): restyles and qa/parity.mjs find it
      part={props.nested ? `list.row.${props.nested}` : 'card.head'}
      // v1's names too, so qa/parity.mjs measures the row against #feed .item (a nested run is v1's own row)
      also={props.nested ? undefined : V1_ROW}
      title={props.s.title || t('shell-list.row.untitled')}
      lead={<Mark s={props.s} view={view()} name={name()} />}
      // a row is two lines (docs/ux/2026-11-18-desktop.md #2, phone #10): the harness's logo and the answer's last line
      // (the machine first when it isn't this computer); with no answer, harness · model as that line.
      sub={
        two() && line().line === 'sub' ? (
          <>
            <HarnessMark s={props.s} />
            <Show when={line().where && view().run}>{(r) => `${r().where} · `}</Show>
            <SubLine sub={view().sub} />
          </>
        ) : env.swipes && line().line === 'run' && view().run ? (
          // a phone, nothing said yet: harness · model is the second line itself, not a third under an empty one
          <RunLine run={view().run as NonNullable<ReturnType<typeof view>['run']>} s={props.s} noWhere />
        ) : (
          <SubLine sub={view().sub} />
        )
      }
      subTone={subLook().tone}
      subTitle={subLook().title}
      // the third line only when it says something (v1: no run line on wisp's own Welcome task)
      foot={
        (view().run && !runInSub()) || bottom() ? (
          <>
            <Show when={!runInSub() && view().run}>{(r) => <RunLine run={r()} s={props.s} noWhere={two()} />}</Show>
            <Show when={bottom()}>
              <Slot input="shell.row:bottom" session={session} />
            </Show>
          </>
        ) : undefined
      }
      pills={
        <>
          <Pills view={view()} />
          <Show when={head()}>
            <Slot input="shell.row:head" session={session} layout="contents" />
          </Show>
        </>
      }
      time={<TimeCell view={view()} />}
      actions={props.nested === 'sub' ? undefined : <Actions k={props.k} view={view()} hover={hover() ? <Slot input="shell.row:hover" session={session} layout="contents" /> : undefined} />}
      quick={props.nested === 'sub' ? 0 : quickCount(view())}
      slots={!env.swipes && props.nested !== 'sub'}
      current={open()}
      selected={env.selected() === props.k && !open()}
      quiet={view().quiet}
      strong={view().unread || props.s.state === 'waiting'}
      {...((p) => (p === undefined ? {} : { picked: p }))(picked())}
      onOpen={() => (picking() ? env.shell.listSelect?.toggle(props.k) : env.openTask(props.k))}
      onContextMenu={() => {
        if (props.nested !== 'sub' && !picking()) env.setMenuFor(props.k);
      }}
    />
  );
  return (
    <Show
      when={env.swipes && !props.nested}
      fallback={
        props.nested ? (
          list()
        ) : (
          <FeedItem part={'card' as `${string}.${string}`} id={props.k} current={open()} selected={env.selected() === props.k && !open()}>
            {list()}
          </FeedItem>
        )
      }
    >
      <Swipe
        left={side(props.swipe.left)}
        right={side(props.swipe.right)}
        onSwipe={swiped}
        disabled={picking()}
        onArm={(on) => env.shell.platform.haptic(on ? 'select' : 'tap')}
        // the phone apps: a long-press opens this row's menu (the list's ContextMenu: Android's sheet with Select
        // first, iOS's context menu with the row as its preview); its twins are the bar's Select and the task's menu
        onLongPress={(row) => (picking() ? env.shell.listSelect?.toggle(props.k) : env.menuAt(props.k, row))}
        part="list.swipe"
      >
        <FeedItem part={'card' as `${string}.${string}`} id={props.k} current={open()} selected={env.selected() === props.k && !open()}>
          {list()}
        </FeedItem>
      </Swipe>
    </Show>
  );
}
