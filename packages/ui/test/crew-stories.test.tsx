// The crew's stories on phones (ADR-0065 step 13b, docs/design/crew-stories.html C): the row (Org first, a ring per
// member with its state, Add last), the story's steps (→ / ←, the arrow keys, a tap on either half), its close (× and a
// drag down), a held press pausing it (with Pause as its control), and its reply box being the task page's ReplyBox,
// compact (the same composer: no second one).
import { createSignal } from 'solid-js';
import { expect, test } from 'vitest';
import { CrewStories, CrewStory, StoryCard, StoryText } from '../src/crew/Stories.tsx';
import { ReplyBox } from '../src/app/Task.tsx';
import { frame, mount, part, tick } from './helpers.tsx';

const LABELS = { close: 'Close', prev: 'Previous member', next: 'Next member', pause: 'Pause', resume: 'Resume', where: '2 of 3 · swipe down to close' };

test('stories_row_is_org_first_then_a_ring_per_member_with_its_state_then_add_last', () => {
  const opened: string[] = [];
  let org = 0;
  let add = 0;
  const host = mount(() => (
    <CrewStories
      label="Crew"
      members={[
        { id: 'g', name: 'Grace', avatar: '', ring: 'wait', label: 'Grace, waiting on you' },
        { id: 'h', name: 'Hopper', avatar: '', ring: 'report', label: 'Hopper, a new report' },
        { id: 'l', name: 'Linus', avatar: '', ring: 'work', label: 'Linus, working' },
        { id: 't', name: 'Turing', avatar: '', ring: 'idle', label: 'Turing, idle', dim: true },
      ]}
      onOpen={(id) => opened.push(id)}
      org={{ label: 'Org', onClick: () => org++ }}
      add={{ label: 'Add', title: 'Add a crew member', onClick: () => add++ }}
    />
  ));
  const row = host.querySelector('[data-kit="stories"]');
  expect(row?.getAttribute('role')).toBe('list');
  expect(row?.getAttribute('aria-label')).toBe('Crew');
  const bubbles = [...host.querySelectorAll<HTMLButtonElement>('[data-kit="story-bubble"]')];
  expect(bubbles.map((b) => b.dataset['ring'])).toEqual(['org', 'wait', 'report', 'work', 'idle', 'add']);
  expect(bubbles.map((b) => b.querySelector('[data-kit="story-name"]')?.textContent)).toEqual(['Org', 'Grace', 'Hopper', 'Linus', 'Turing', 'Add']);
  // names are control text (one line); each ring is named for a screen reader
  for (const b of bubbles) expect(b.querySelector('[data-kit="story-name"]')?.getAttribute('data-text')).toBe('control');
  expect(bubbles[1]?.getAttribute('aria-label')).toBe('Grace, waiting on you');
  expect(bubbles[4]?.querySelector('[data-kit="face"]')?.hasAttribute('data-dim')).toBe(true);
  bubbles[0]?.click();
  bubbles[3]?.click();
  bubbles[5]?.click();
  expect([org, opened, add]).toEqual([1, ['l'], 1]);
});

test('no_crew_yet_the_row_is_add_alone', () => {
  const host = mount(() => <CrewStories label="Crew" members={[]} onOpen={() => {}} add={{ label: 'Add', onClick: () => {} }} />);
  expect([...host.querySelectorAll<HTMLElement>('[data-kit="story-bubble"]')].map((b) => b.dataset['ring'])).toEqual(['add']);
});

function story(o: { dwell?: number; reply?: boolean } = {}) {
  const [at, setAt] = createSignal(1);
  let closed = 0;
  mount(() => (
    <CrewStory
      title="Ada's story"
      index={at()}
      count={3}
      onStep={setAt}
      onClose={() => closed++}
      face={<span />}
      name="Ada"
      sub="Reviewer · Review the reconnect fix"
      labels={LABELS}
      dwell={o.dwell ?? 0}
      {...(o.reply && { reply: <ReplyBox compact placeholder="Message Ada…" value="" onInput={() => {}} onSend={() => {}} send="Send" /> })}
    >
      <StoryCard label="Now">
        <StoryText title="Review the reconnect fix" line="Reading docs/sync.md" live />
      </StoryCard>
    </CrewStory>
  ));
  return { at, closed: () => closed };
}

test('story_steps_with_its_arrows_its_keys_and_a_tap_on_either_half_and_stops_at_the_ends', async () => {
  const s = story();
  await frame();
  const page = part('crew.story.page') as HTMLElement;
  expect(page).not.toBeNull();
  expect(document.querySelector('[data-part~="crew.stories.view"]')?.getAttribute('role')).toBe('dialog');
  // where you are: the bars before are done, this one is on
  expect([...page.querySelectorAll('[data-kit="story-bar"]')].map((b) => b.getAttribute('data-at'))).toEqual(['done', 'now', null]);
  part('crew.story.next')?.click();
  expect(s.at()).toBe(2);
  expect((part('crew.story.next') as HTMLButtonElement).disabled).toBe(true);
  part('crew.story.prev')?.click();
  expect(s.at()).toBe(1);
  page.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  expect(s.at()).toBe(0);
  expect((part('crew.story.prev') as HTMLButtonElement).disabled).toBe(true);
  page.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  expect(s.at()).toBe(0);
  // from a focused control (the dialog focuses its first: Pause / Close) the arrows still step; in a field they don't
  part('crew.story.close')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(s.at()).toBe(1);
  // a tap on the body's right half steps on, on its left half back
  const body = part('crew.story.body') as HTMLElement;
  body.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 600, right: 400, bottom: 600, x: 0, y: 0, toJSON: () => ({}) });
  const tap = (x: number): void => {
    body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, isPrimary: true, button: 0, clientX: x, clientY: 100 }));
    body.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, isPrimary: true, button: 0, clientX: x, clientY: 100 }));
  };
  tap(300);
  expect(s.at()).toBe(2);
  tap(50);
  expect(s.at()).toBe(1);
});

test('story_closes_with_its_close_button_and_with_escape', async () => {
  const s = story();
  await frame();
  part('crew.story.close')?.click();
  expect(s.closed()).toBe(1);
  // the drag down's twin is the close button, named on the page for qa/platform.mjs
  const page = part('crew.story.page') as HTMLElement;
  expect(page.dataset['gesture']).toBe('story.drag-down story.long-press');
  expect(page.dataset['twin']).toBe('crew.story.close crew.story.pause');
  (document.querySelector('[data-part~="crew.stories.view"]') as HTMLElement).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await tick();
  expect(s.closed()).toBeGreaterThanOrEqual(2);
});

test('a_held_press_pauses_the_story_its_pause_button_too_and_it_moves_on_when_the_bar_fills', async () => {
  const s = story({ dwell: 8000 });
  await frame();
  const page = part('crew.story.page') as HTMLElement;
  const body = part('crew.story.body') as HTMLElement;
  expect(page.hasAttribute('data-paused')).toBe(false);
  const down = new PointerEvent('pointerdown', { bubbles: true, isPrimary: true, button: 0, clientX: 10, clientY: 10 });
  body.dispatchEvent(down);
  expect(page.hasAttribute('data-paused')).toBe(true);
  // let go after a long hold (past the long-press delay): it carries on where it was, no step
  const up = new PointerEvent('pointerup', { bubbles: true, isPrimary: true, button: 0, clientX: 10, clientY: 10 });
  Object.defineProperty(up, 'timeStamp', { value: down.timeStamp + 600 });
  const before = s.at();
  body.dispatchEvent(up);
  expect(page.hasAttribute('data-paused')).toBe(false);
  expect(s.at()).toBe(before);
  // Pause is the hold's control on screen
  const pause = part('crew.story.pause') as HTMLButtonElement;
  expect(pause.getAttribute('aria-label')).toBe('Pause');
  pause.click();
  expect(page.hasAttribute('data-paused')).toBe(true);
  expect(pause.getAttribute('aria-label')).toBe('Resume');
  pause.click();
  // the bar filled: the next member
  const at = s.at();
  page.querySelector('[data-kit="story-fill"]')?.dispatchEvent(new Event('animationend', { bubbles: true }));
  expect(s.at()).toBe(at + 1);
});

test('the_story_reply_is_the_task_pages_reply_box_compact_and_typing_in_it_holds_the_story', async () => {
  story({ dwell: 8000, reply: true });
  await frame();
  const box = part('crew.story.reply')?.querySelector<HTMLElement>('[data-kit="reply-box"]');
  expect(box, 'the kit’s ReplyBox, not a second composer').not.toBeNull();
  expect(box?.hasAttribute('data-compact')).toBe(true);
  expect(box?.querySelector('[data-kit="reply"]')?.getAttribute('placeholder')).toBe('Message Ada…');
  expect(box?.querySelector('[data-part="composer.send"]')).not.toBeNull();
  // compact: none of the task's tools
  for (const p of ['composer.attach', 'composer.mode', 'composer.harness', 'composer.team', 'composer.goal', 'composer.sendLater']) expect(box?.querySelector(`[data-part="${p}"]`), p).toBeNull();
  box?.querySelector<HTMLTextAreaElement>('textarea')?.focus();
  expect(part('crew.story.page')?.hasAttribute('data-paused')).toBe(true);
  // one box (a JSX prop read twice would build a second, orphaned one)
  expect(document.querySelectorAll('[data-kit="reply-box"]').length).toBe(1);
  const at = document.querySelector('[data-kit="story-where"]')?.textContent;
  box?.querySelector('textarea')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(document.querySelector('[data-kit="story-where"]')?.textContent).toBe(at);
});

test('a_full_reply_box_keeps_its_tools', () => {
  const host = mount(() => <ReplyBox placeholder="Follow up" value="" onInput={() => {}} onSend={() => {}} attach="Attach" mode={{ label: 'Auto', title: 'Mode' }} runner={{ title: 'Claude Code' }} team="Team" goal="Goal" later="Later" send="Send" />);
  const box = host.querySelector('[data-kit="reply-box"]');
  expect(box?.hasAttribute('data-compact')).toBe(false);
  expect(box?.getAttribute('data-part')).toBe('composer');
  for (const p of ['composer.attach', 'composer.mode', 'composer.harness', 'composer.team', 'composer.goal', 'composer.sendLater', 'composer.send']) expect(box?.querySelector(`[data-part="${p}"]`), p).not.toBeNull();
});
