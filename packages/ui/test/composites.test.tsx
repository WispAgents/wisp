import { describe, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount, part } from './helpers.tsx';
import {
  Banner, Callout, Card, Checklist, Composer, EmptyState, ListRow, Section, SettingsRow, Stats, Steps, Toolbar, nextInToolbar, sendsOn,
} from '../src/composites/index.ts';
import { RowRun, RowButton } from '../src/composites/ListRow.tsx';
import { Time } from '../src/content/Time.tsx';
import { Dot } from '../src/shell-list/Dot.tsx';

const key = (k: string, extra: Partial<KeyboardEventInit> = {}): KeyboardEvent => new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...extra });

describe('every composite renders its data-part (default and given)', () => {
  test('defaults', () => {
    mount(() => (
      <>
        <ListRow title="Fix the login" />
        <Card>Body</Card>
        <Toolbar aria-label="Task actions" items={[{ id: 'a', label: 'A', icon: 'check', priority: 1, onSelect: () => {} }]} />
        <Composer value="" onInput={() => {}} onSend={() => {}} placeholder="Reply…" />
        <EmptyState title="No machines yet" />
        <Banner>Reconnecting to wisp…</Banner>
        <Callout title="Heads up">Text</Callout>
        <Stats items={[{ label: 'tests passing', value: 42 }]} />
        <Steps items={[{ title: 'Build', status: 'done' }]} />
        <Checklist items={[{ text: 'Merged', done: true }]} />
        <Section title="Theme">Rows</Section>
        <SettingsRow title="Theme" control={<span />} />
      </>
    ));
    for (const name of [
      'ui.list-row', 'ui.card', 'ui.toolbar', 'ui.composer', 'ui.empty-state', 'ui.banner', 'ui.callout',
      'ui.stats', 'ui.steps', 'ui.checklist', 'ui.section', 'ui.settings-row',
    ]) {
      expect(part(name), name).not.toBeNull();
    }
    expect(part('ui.composer.send')).not.toBeNull();
    expect(part('ui.list-row.time')).not.toBeNull();
  });

  test('a given part replaces the default, and inner parts follow it', () => {
    mount(() => <ListRow part="list.row" title="A task" />);
    expect(part('list.row')).not.toBeNull();
    expect(part('list.row.title')?.textContent).toBe('A task');
    expect(part('ui.list-row')).toBeNull();
  });
});

describe('ListRow', () => {
  const view = (selected: () => boolean) => (
    <ListRow title="Refactor auth" sub="wisp · main" lead={<span>•</span>} pills={<span>Working</span>} time={<span>28m ago</span>} actions={<button type="button">⋯</button>} selected={selected()} onOpen={() => {}} />
  );

  test('selecting adds and removes no elements and keeps the column order', () => {
    const [sel, setSel] = createSignal(false);
    const host = mount(() => view(sel));
    const shape = (): string[] => [...host.querySelectorAll('[data-kit]')].map((n) => n.getAttribute('data-kit') ?? '');
    const before = shape();
    const timeNode = part('ui.list-row.time');
    setSel(true);
    expect(shape()).toEqual(before);
    expect(part('ui.list-row')?.getAttribute('aria-selected')).toBe('true');
    // Same nodes, not re-created ones: nothing replays an entry animation.
    expect(part('ui.list-row.time')).toBe(timeNode);
    expect(before.slice(0, 6)).toEqual(['list-row', 'list-row-lead', 'list-row-main', 'list-row-hit', 'list-row-title', 'list-row-sub']);
  });

  test('every cell is rendered even when empty, so columns never shift', () => {
    mount(() => <ListRow title="Bare" />);
    for (const cell of ['lead', 'main', 'pills', 'time', 'actions']) expect(part(`ui.list-row.${cell}`), cell).not.toBeNull();
  });

  test('the title carries its full text for hover, and opens the row', () => {
    const onOpen = vi.fn();
    mount(() => <ListRow title="A very long task title that will be cut" onOpen={onOpen} current />);
    const title = part('ui.list-row.title');
    expect(title?.getAttribute('title')).toBe('A very long task title that will be cut');
    const hit = title?.closest('button');
    expect(hit?.getAttribute('aria-current')).toBe('page');
    hit?.click();
    expect(onOpen).toHaveBeenCalledOnce();
  });

  test('href renders a real link', () => {
    mount(() => <ListRow title="Docs" href="/docs" />);
    expect(part('ui.list-row.title')?.closest('a')?.getAttribute('href')).toBe('/docs');
  });
});

describe('Composer', () => {
  function setup(submit?: 'enter' | 'mod-enter') {
    const onSend = vi.fn();
    const [value, setValue] = createSignal('Ship it');
    mount(() => <Composer value={value()} onInput={setValue} onSend={onSend} {...(submit ? { submit } : {})} />);
    const box = part('ui.composer.input');
    if (!(box instanceof HTMLTextAreaElement)) throw new Error('no textarea');
    return { onSend, box, setValue };
  }

  test('Enter sends the text; Shift+Enter does not', () => {
    const { onSend, box } = setup();
    box.dispatchEvent(key('Enter', { shiftKey: true }));
    expect(onSend).not.toHaveBeenCalled();
    const enter = key('Enter');
    box.dispatchEvent(enter);
    expect(onSend).toHaveBeenCalledWith('Ship it');
    expect(enter.defaultPrevented).toBe(true);
  });

  test('mod-enter mode: Enter is a new line, ⌘/Ctrl+Enter sends', () => {
    const { onSend, box } = setup('mod-enter');
    box.dispatchEvent(key('Enter'));
    expect(onSend).not.toHaveBeenCalled();
    box.dispatchEvent(key('Enter', { metaKey: true }));
    expect(onSend).toHaveBeenCalledOnce();
  });

  test('blank text and busy do not send; Send is disabled while blank', () => {
    const { onSend, box, setValue } = setup();
    setValue('   ');
    box.dispatchEvent(key('Enter'));
    expect(onSend).not.toHaveBeenCalled();
    expect(part('ui.composer.send')?.hasAttribute('disabled')).toBe(true);
  });

  test('an IME confirming a word with Enter is not a send', () => {
    expect(sendsOn({ key: 'Enter', shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, isComposing: true }, 'enter')).toBe(false);
    expect(sendsOn({ key: 'Enter', shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, isComposing: false }, 'enter')).toBe(true);
  });

  test("Send's name comes from the kit's strings unless given", () => {
    mount(() => <Composer value="x" onInput={() => {}} onSend={() => {}} sendLabel="Reply" part="task.reply" />);
    expect(part('task.reply.send')?.getAttribute('aria-label')).toBe('Reply');
  });
});

describe('Toolbar', () => {
  test('arrow keys, Home and End move focus between its controls, wrapping', () => {
    const noop = (): void => {};
    mount(() => (
      <Toolbar
        aria-label="Task actions"
        items={[
          { id: 'a', label: 'A', icon: 'check', priority: 1, onSelect: noop },
          { id: 'b', label: 'B', icon: 'rerun', priority: 1, onSelect: noop },
          { id: 'c', label: 'C', icon: 'archive', priority: 1, onSelect: noop, disabled: true },
          { id: 'd', label: 'D', icon: 'send', priority: 'pin', onSelect: noop },
        ]}
      />
    ));
    // The visible copy of each item, not the hidden measuring copy.
    const btn = (n: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-kit="toolbar-item"][data-item="${n}"] button`);
    btn('a')?.focus();
    btn('a')?.dispatchEvent(key('ArrowRight'));
    expect(document.activeElement).toBe(btn('b'));
    btn('b')?.dispatchEvent(key('ArrowRight'));
    expect(document.activeElement).toBe(btn('d')); // the disabled one is skipped
    btn('d')?.dispatchEvent(key('ArrowRight'));
    expect(document.activeElement).toBe(btn('a'));
    btn('a')?.dispatchEvent(key('ArrowLeft'));
    expect(document.activeElement).toBe(btn('d'));
    btn('d')?.dispatchEvent(key('Home'));
    expect(document.activeElement).toBe(btn('a'));
    expect(part('ui.toolbar')?.getAttribute('role')).toBe('toolbar');
  });

  test('nextInToolbar ignores other keys and empty bars', () => {
    expect(nextInToolbar([], 0, 'ArrowRight')).toBeUndefined();
    const a = document.createElement('button');
    expect(nextInToolbar([a], 0, 'x')).toBeUndefined();
  });
});

describe('the rest', () => {
  test('Banner: errors are alerts, others status; dismiss has a name', () => {
    const onDismiss = vi.fn();
    mount(() => (
      <>
        <Banner tone="bad" part="a.bad">Failed</Banner>
        <Banner tone="info" part="a.info" onDismiss={onDismiss}>Update ready</Banner>
      </>
    ));
    expect(part('a.bad')?.getAttribute('role')).toBe('alert');
    expect(part('a.info')?.getAttribute('role')).toBe('status');
    const x = part('a.info.dismiss');
    expect(x?.getAttribute('aria-label')).toBe('Dismiss');
    x?.click();
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  test('banner_with_a_title_over_a_body_is_stacked_so_its_icon_sits_on_the_title_line', () => {
    // r330 #13: a usage limit (title, note, buttons) centred its warn icon on the whole card, below the title
    let built = 0;
    const Body = (): string => (built++, 'Wait for the reset.');
    mount(() => (
      <>
        <Banner tone="warn" part="a.limit" title="Codex hit its usage limit">
          <Body />
        </Banner>
        <Banner tone="info" part="a.title" title="Reconnecting to wisp…" />
        <Banner tone="info" part="a.body">Update ready</Banner>
        <Callout part="a.note" title="Assumption">Read X-Forwarded-For.</Callout>
      </>
    ));
    expect(part('a.limit')?.hasAttribute('data-stacked')).toBe(true);
    expect(part('a.note')?.hasAttribute('data-stacked')).toBe(true);
    // one line: icon, words and action stay centred
    expect(part('a.title')?.hasAttribute('data-stacked')).toBe(false);
    expect(part('a.body')?.hasAttribute('data-stacked')).toBe(false);
    expect(part('a.limit')?.textContent).toContain('Wait for the reset.');
    // the body is built once, not once per read of it
    expect(built).toBe(1);
  });

  test('Stats formats numbers and shows unit and delta', () => {
    mount(() => <Stats items={[{ label: 'tests', value: 1234, unit: 'ms', delta: '+6', tone: 'good' }]} format={(n) => `#${n}`} />);
    const stat = part('ui.stats.item');
    expect(stat?.getAttribute('data-tone')).toBe('good');
    expect(stat?.textContent).toContain('#1234');
    expect(stat?.textContent).toContain('+6');
  });

  test('Steps marks the current step; Checklist keeps undone items', () => {
    mount(() => (
      <>
        <Steps items={[{ title: 'a', status: 'done' }, { title: 'b', status: 'current' }, { title: 'c', status: 'skipped' }]} />
        <Checklist items={[{ text: 'Merged', done: true }, { text: 'Published', done: false }]} statusLabel={(d) => (d ? 'Done' : 'Not done')} />
      </>
    ));
    expect(document.querySelector('[aria-current="step"]')?.textContent).toBe('b');
    expect(document.querySelectorAll('[data-part="ui.checklist.item"]').length).toBe(2);
    expect(document.querySelector('[data-kit="check-box"]')?.getAttribute('aria-label')).toBe('Done');
  });

  test('SettingsRow labels its control; Section names itself by its heading', () => {
    mount(() => (
      <Section title="Appearance">
        <SettingsRow title="Theme" controlId="theme" control={<select id="theme" />} />
      </Section>
    ));
    expect(document.querySelector('label[for="theme"]')?.textContent).toBe('Theme');
    const section = part('ui.section');
    const labelledBy = section?.getAttribute('aria-labelledby') ?? '';
    expect(document.getElementById(labelledBy)?.textContent).toBe('Appearance');
  });

  test('Card with onClick is a button', () => {
    const onClick = vi.fn();
    mount(() => <Card onClick={onClick} header="Head">Body</Card>);
    const card = part('ui.card');
    expect(card?.tagName).toBe('BUTTON');
    card?.click();
    expect(onClick).toHaveBeenCalledOnce();
    expect(part('ui.card.header')?.textContent).toBe('Head');
  });
});

describe('the feed row (v1 #feed .it-head)', () => {
  test('a_feed_row_is_v1s_it_head_element_for_element_the_head_its_one_button', async () => {
    const opened = vi.fn();
    const done = vi.fn();
    const host = mount(() => (
      <ListRow
        layout="feed"
        part="card.head"
        title="Fix the login bug"
        lead={<Dot size="md" color="pc-violet" />}
        sub="No question asked."
        subTone="muted"
        foot={<RowRun name="Claude Code" where={{ icon: 'laptop', text: 'This computer' }} />}
        pills={<span data-kit="it-pill">Finished</span>}
        time={<Time at={0} now={300_000} />}
        actions={<RowButton icon="done" tone="good" label="Mark done" onClick={done} />}
        onOpen={opened}
      />
    ));
    const head = host.querySelector<HTMLElement>('[data-kit="list-row"]');
    // the head's children in v1's order: dot, title, the sub's words, the run line, the pills (the time last), the quick action
    expect([...(head?.children ?? [])].map((e) => (e as HTMLElement).dataset['kit'])).toEqual(['dot', 'list-row-title', 'list-row-sub', 'row-run', 'list-row-pills', 'icon-button']);
    expect(head?.querySelector('[data-kit="list-row-sub"]')?.childElementCount).toBe(0); // its words alone, as .it-sub
    expect([...(head?.querySelector('[data-kit="list-row-pills"]')?.children ?? [])].map((e) => (e as HTMLElement).dataset['kit'])).toEqual(['it-pill', 'time']);
    // the run line: the harness, then where (`·` is CSS), its glyph and words in one box
    expect([...(head?.querySelector('[data-kit="row-run"]')?.children ?? [])].map((e) => (e as HTMLElement).dataset['kit'])).toEqual(['ir-h', 'ir-w']);
    // the head is the button; the quick action is the pointer's (hidden from assistive tech, out of the Tab order)
    expect(head?.getAttribute('role')).toBe('button');
    expect(head?.tabIndex).toBe(0);
    const quick = head?.querySelector<HTMLElement>('[data-kit="icon-button"]');
    expect(quick?.getAttribute('aria-hidden')).toBe('true');
    expect(quick?.hasAttribute('tabindex')).toBe(false);
    expect(quick?.children.length).toBe(1); // the glyph, no box around it
    quick?.click();
    expect(done).toHaveBeenCalledTimes(1);
    expect(opened).not.toHaveBeenCalled(); // a quick action never opens the row
    head?.dispatchEvent(key('Enter'));
    head?.click();
    expect(opened).toHaveBeenCalledTimes(2);
  });
  test('a_rows_third_line_names_harness_model_and_place_each_in_its_own_span_with_the_tip_on_the_line', async () => {
    // F-task-6, v1 feed.js runLine(): .ir-h, .ir-m (it ellipsises first), .ir-w; the tip on the whole .it-run
    const host = mount(() => <RowRun name="Claude Code" model="Opus 4.7" title="Claude Code · Opus 4.7 · High thinking · on This computer" where={{ icon: 'laptop', text: 'This computer' }} />);
    const run = host.querySelector<HTMLElement>('[data-kit="row-run"]');
    expect([...(run?.children ?? [])].map((e) => (e as HTMLElement).dataset['kit'])).toEqual(['ir-h', 'ir-m', 'ir-w']);
    expect(run?.querySelector('[data-kit="ir-h"]')?.textContent).toBe('Claude Code');
    expect(run?.querySelector('[data-kit="ir-m"]')?.textContent).toBe('Opus 4.7');
    expect(run?.querySelector('[data-kit="ir-m"]')?.hasAttribute('data-long')).toBe(true);
    // a short model takes its own width, no 7ch floor (TRY-12: "opus   · This computer")
    const short = mount(() => <RowRun name="Claude Code" model="opus" where={{ icon: 'laptop', text: 'This computer' }} />);
    expect(short.querySelector('[data-kit="ir-m"]')?.hasAttribute('data-long')).toBe(false);
    expect(run?.title).toBe('Claude Code · Opus 4.7 · High thinking · on This computer');
    // no model: no empty span (v1 drops "Default model" from the row, the tip keeps it)
    const bare = mount(() => <RowRun name="Codex" where={{ icon: 'server', text: 'box-1' }} />);
    expect([...(bare.querySelector('[data-kit="row-run"]')?.children ?? [])].map((e) => (e as HTMLElement).dataset['kit'])).toEqual(['ir-h', 'ir-w']);
  });
});

test('a_settings_row_title_too_long_for_its_row_ends_in_an_ellipsis_with_the_whole_on_hover_qa_r1_21', () => {
  const long = 'Ask near the limit: before a task would take a usage window past this percent, ask first (auto)';
  mount(() => <SettingsRow part="a.long" title={long} control={<span />} />);
  const title = part('a.long')?.querySelector('[data-kit="settings-row-title"]');
  // the words in their own box: text-overflow ends a block's text, not a flex container's (the title is one)
  const words = title?.querySelector('[data-kit="settings-row-words"]');
  expect(words?.textContent).toBe(long);
  expect(words?.getAttribute('title')).toBe(long);
});

test('a_settings_row_can_carry_more_under_its_words_under_its_words_and_control_qa_r1_25', () => {
  // Cloud's "Agents on it" sat under the row, flush with the card's border, not with the row's words (v1 .cg-agents:
  // grid-column 2 / -1, a hairline above)
  mount(() => <SettingsRow part="a.below" tile="cloud" title="Cloud" description="m0xoo" below={<p data-testid="more">Agents on it</p>} control={<span />} />);
  const row = part('a.below');
  const below = row?.querySelector(':scope > [data-kit="settings-row-below"]');
  expect(below?.querySelector('[data-testid="more"]')?.textContent).toBe('Agents on it');
  // a row of its own in the row's grid, after the control, from the words' column to the end (v1 grid-column 2 / -1)
  expect(below?.previousElementSibling?.getAttribute('data-kit')).toBe('settings-row-control');
});
