// The transcript's steps and expanded action groups (transcript/Timeline.tsx).
import { expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { Timeline, TimelineGroup, TimelineStep } from '../src/transcript/Timeline.tsx';

test('an_opened_step_shows_its_whole_title_it_wraps_instead_of_a_cut_control_line', () => {
  const [open, setOpen] = createSignal(false);
  const host = mount(() => (
    <Timeline>
      <TimelineStep icon="terminal" label="Run the whole test suite with coverage and the slow integration tests too" detail={() => <pre>ok</pre>} expanded={open()} onToggle={() => setOpen(!open())} />
    </Timeline>
  ));
  const label = (): HTMLElement | null => host.querySelector('[data-kit="timeline-label"]');
  expect(label()?.getAttribute('data-text')).toBe('control'); // closed: one line and an ellipsis
  host.querySelector<HTMLButtonElement>('[data-kit="timeline-row"]')?.click();
  expect(host.querySelector('[data-kit="timeline-step"]')?.hasAttribute('data-open')).toBe(true);
  expect(label()?.hasAttribute('data-text')).toBe(false); // open: the whole title, wrapping (v1 #779)
});

test('an_expanded_action_group_renders_unified_card_file_chips_with_icons_and_nested_steps', () => {
  const [open, setOpen] = createSignal(false);
  const [stepOpen, setStepOpen] = createSignal(false);
  const host = mount(() => (
    <Timeline chat>
      <TimelineGroup
        icon="pen"
        label="Edited 1 file · Read 2 files · 1 command"
        count={4}
        expanded={open()}
        onToggle={() => setOpen(!open())}
        files={[
          { name: 'Timeline.tsx', title: 'packages/ui/src/transcript/Timeline.tsx' },
          { name: 'transcript.css', title: 'packages/ui/src/css/transcript.css' },
        ]}
      >
        <TimelineStep
          icon="pen"
          kind="edit"
          label="Editing packages/ui/src/transcript/Timeline.tsx"
          nums={{ add: 24, del: 8 }}
        />
        <TimelineStep
          icon="terminal"
          kind="run"
          label="Running tests"
          command="npx vitest run packages/ui/test/timeline.test.tsx"
          detail={() => <pre>2 passed</pre>}
          expanded={stepOpen()}
          onToggle={() => setStepOpen(!stepOpen())}
        />
      </TimelineGroup>
    </Timeline>
  ));
  const group = (): HTMLElement | null => host.querySelector('[data-kit="timeline-step"][data-group]');
  expect(group()?.hasAttribute('data-open')).toBe(false);
  expect(host.querySelector('[data-kit="timeline-group-body"]')).toBeNull();

  // Expand the group: card opens with file chips (each carrying a file icon) separated from the action sublist
  host.querySelector<HTMLButtonElement>('[data-group] > button[data-kit="timeline-row"]')?.click();
  expect(group()?.hasAttribute('data-open')).toBe(true);
  expect(host.querySelector('[data-kit="timeline-group-files"]')?.hasAttribute('data-has-steps')).toBe(true);
  const files = host.querySelectorAll('[data-kit="timeline-file"]');
  expect(files.length).toBe(2);
  expect(files[0]?.querySelector('[data-kit="icon"]')).not.toBeNull();
  expect(files[0]?.querySelector('[data-kit="timeline-file-name"]')?.textContent).toBe('Timeline.tsx');

  // Trailing metadata and chevron sit in timeline-trail for right alignment
  expect(host.querySelectorAll('[data-kit="timeline-sublist"] [data-kit="timeline-trail"]').length).toBe(2);

  // Expanding a nested step inside the group reveals its detail panel
  host.querySelector<HTMLButtonElement>('[data-kit="timeline-sublist"] button[data-kit="timeline-row"]')?.click();
  expect(host.querySelector('[data-kit="timeline-sublist"] [data-kit="timeline-detail"] > pre')?.textContent).toBe('2 passed');
});
