// The Delegate panel's kit pieces (app/TeamPop.tsx, v1 team.js teamEditor / subagents.css .team-pop) and a control's
// panel (Act.panel): the lead's strip, a row's place chip, copies and second line, and a button that opens a panel.
import { afterEach, describe, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount, part, until } from './helpers.tsx';
import { TeamCopies, TeamFields, TeamLead, TeamPop, TeamRow, TeamSave, TeamWhere } from '../src/app/TeamPop.tsx';
import { WithAct } from '../src/app/act.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

describe('delegate panel pieces_v1_team_pop', () => {
  test('the wide panel puts the lead strip under its head: LEAD, the run, on, a dashed place that only says', () => {
    mount(() => (
      <TeamPop
        part="d.pop"
        wide
        title="Delegate"
        intro="Agents the lead can hand parts of the task to"
        lead={<TeamLead part="d.lead" tag="Lead" run="Claude Code · Opus · High" on="on" place={{ icon: 'laptop', label: 'This computer', title: 'Where the lead runs' }} />}
        count={0}
        empty={{ icon: 'agent', text: 'No delegates yet.' }}
        add={<button type="button">Add delegate</button>}
        tally="0 agents"
      />
    ));
    const pop = part('d.pop');
    expect(pop?.hasAttribute('data-wide')).toBe(true);
    const lead = part('d.lead');
    // under the head, before the list
    expect(lead?.previousElementSibling?.getAttribute('data-kit')).toBe('tp-head');
    expect(lead?.textContent).toBe('LeadClaude Code · Opus · HighonThis computer');
    const place = part('d.lead.place');
    expect(place?.tagName).toBe('SPAN');
    expect(place?.hasAttribute('data-static')).toBe(true);
    expect(pop?.querySelector('[data-kit="tp-empty"]')?.textContent).toBe('No delegates yet.');
  });

  test('copies step between one and the cap, the ends disabled, accent from two_v1_tp_copies', () => {
    const [n, setN] = createSignal(1);
    const seen: number[] = [];
    mount(() => <TeamCopies part="d.copies" n={n()} max={3} title="Copies" fewer="One copy fewer" more="One copy more" onChange={(x) => (seen.push(x), setN(x))} />);
    const box = part('d.copies');
    const [dec, inc] = [...(box?.querySelectorAll('button') ?? [])];
    expect(dec?.disabled).toBe(true);
    expect(box?.hasAttribute('data-on')).toBe(false);
    inc?.click();
    inc?.click();
    expect(box?.querySelector('b')?.textContent).toBe('×3');
    expect(box?.hasAttribute('data-on')).toBe(true);
    expect(inc?.disabled).toBe(true);
    inc?.click();
    dec?.click();
    expect(seen).toEqual([2, 3, 2]);
  });

  test('a row keeps copies and Remove together at its end, and its second line is the @name then Use for', () => {
    const typed: string[] = [];
    mount(() => (
      <TeamRow
        part="d.row"
        remove={
          <>
            <span data-testid="copies" />
            <button type="button" data-testid="x" />
          </>
        }
        bottom={
          <TeamFields
            part="d.fields"
            name={{ value: '', placeholder: 'claude', title: 'What the lead calls it', onInput: (v) => typed.push(`name:${v}`) }}
            use={{ label: 'Use for', value: '', placeholder: 'e.g. tests', onInput: (v) => typed.push(`use:${v}`) }}
          />
        }
      >
        <TeamWhere part="d.place" place={{ icon: 'agent', label: 'With the lead', title: 'Runs with the lead' }} act={{ onClick: () => undefined }} />
      </TeamRow>
    ));
    const end = part('d.row')?.querySelector('[data-kit="tp-end"]');
    expect([...(end?.children ?? [])].map((e) => e.getAttribute('data-testid'))).toEqual(['copies', 'x']);
    expect(part('d.place')?.tagName).toBe('BUTTON');
    const bottom = part('d.row')?.querySelector('[data-kit="tp-bottom"]');
    expect(bottom?.textContent).toBe('@Use for');
    const name = part('d.fields.name')?.querySelector('input') as HTMLInputElement;
    name.value = 'tester';
    name.dispatchEvent(new InputEvent('input', { bubbles: true }));
    expect(typed).toEqual(['name:tester']);
    expect(name.maxLength).toBe(24);
  });
});

test('a control with a panel opens it on its own button, drawn only while open_v1_team_pop', async () => {
  const drawn = vi.fn(() => <p data-testid="inside">Delegate</p>);
  mount(() => (
    <WithAct
      act={{ panel: { label: 'Delegate', part: 'd.panel', cardPart: 'delegate.panel', content: drawn } }}
      render={(trigger) => (
        <button type="button" {...trigger()} data-part="d.button">
          Delegate
        </button>
      )}
    />
  ));
  const btn = part('d.button') as HTMLButtonElement;
  expect(btn.getAttribute('aria-haspopup')).toBe('dialog');
  expect(drawn).not.toHaveBeenCalled();
  btn.click();
  // the popover loads apart from the first page, then opens on the press it missed
  await until(() => document.querySelector('[data-testid="inside"]') !== null);
  expect(part('delegate.panel')).not.toBeNull();
});

test('an open panel is drawn once while its caller rebuilds the act around it, so a field keeps its caret', async () => {
  // r330 #27: Home rebuilds its tools on every team change; the panel was drawn again and the Use for field lost focus
  const [n, setN] = createSignal(0);
  let drawn = 0;
  mount(() => (
    <WithAct
      act={{
        panel: {
          label: `Delegate ${n()}`,
          part: 'd.panel2',
          content: () => {
            drawn++;
            return <input data-testid="field" value={String(n())} />;
          },
        },
      }}
      render={(trigger) => (
        <button type="button" {...trigger()} data-part="d.button2">
          Delegate
        </button>
      )}
    />
  ));
  (part('d.button2') as HTMLButtonElement).click();
  await until(() => document.querySelector('[data-testid="field"]') !== null);
  const field = document.querySelector('[data-testid="field"]') as HTMLInputElement;
  setN(1);
  setN(2);
  expect(drawn).toBe(1);
  expect(document.querySelector('[data-testid="field"]')).toBe(field);
  // what it shows still follows its own signals
  expect(field.value).toBe('2');
});

test('presets sit at the head end and Save as preset swaps the foot for its name form_v1_tp_saveform', () => {
  const [saving, setSaving] = createSignal(false);
  const saved: string[] = [];
  mount(() => (
    <TeamPop
      part="d.pop3"
      wide
      title="Delegate"
      intro="…"
      head={<button type="button" data-testid="presets">Presets</button>}
      count={1}
      empty={{ icon: 'agent', text: '' }}
      add={<button type="button">Add delegate</button>}
      tally="1 agent"
      clear={{ label: 'Clear', onClick: () => undefined }}
      end={<button type="button" data-testid="save" onClick={() => setSaving(true)}>Save as preset</button>}
      form={saving() ? <TeamSave part="d.save" value="" placeholder="Preset name" save="Save" cancel="Cancel" onSave={(n) => (saved.push(n), setSaving(false))} onCancel={() => setSaving(false)} /> : undefined}
    >
      <span />
    </TeamPop>
  ));
  const pop = part('d.pop3') as HTMLElement;
  expect(pop.querySelector('[data-kit="tp-head"] [data-testid="presets"]')).not.toBeNull();
  const foot = pop.querySelector('[data-kit="tp-foot"]') as HTMLElement;
  expect(foot.textContent).toBe('Add delegate1 agentClearSave as preset');
  (pop.querySelector('[data-testid="save"]') as HTMLButtonElement).click();
  expect(foot.textContent).toBe('SaveCancel');
  const form = part('d.save') as HTMLFormElement;
  const input = form.querySelector('input') as HTMLInputElement;
  // an empty name saves nothing
  form.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }));
  expect(saved).toEqual([]);
  input.value = '  Review crew ';
  input.dispatchEvent(new InputEvent('input', { bubbles: true }));
  form.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }));
  expect(saved).toEqual(['Review crew']);
  expect(foot.textContent).toBe('Add delegate1 agentClearSave as preset');
});

// QA desktop r4 #1 (third try): with no presets, Presets' "No presets yet" floated over the panel's words, then the
// launcher's bar, then the lead's row. It is a line of the panel now, under its head (v1's empty menu row, said in
// place): it pushes the panel down and covers nothing.
test('a_note_under_the_head_is_part_of_the_panel_not_a_bubble_over_it_qa_desktop_r4_1', () => {
  const [on, setOn] = createSignal(false);
  mount(() => (
    <TeamPop
      part="d.pop"
      wide
      title="Delegate"
      intro="Agents the lead can hand parts of the task to"
      head={<button type="button" onClick={() => setOn(!on())}>Presets</button>}
      note={on() ? { text: 'No presets yet. Set up a team, then Save as preset.', part: 'd.none' } : undefined}
      lead={<TeamLead part="d.lead" tag="Lead" run="Claude Code" on="on" place={{ icon: 'laptop', label: 'This computer', title: 'Where' }} />}
      count={0}
      empty={{ icon: 'agent', text: 'No delegates yet.' }}
      add={<button type="button">Add delegate</button>}
      tally="0 agents"
    />
  ));
  expect(part('d.none')).toBeNull();
  setOn(true);
  const note = part('d.none');
  expect(note?.textContent).toBe('No presets yet. Set up a team, then Save as preset.');
  // in the panel's flow, after its head and before the lead: no portal, no positioner
  const pop = part('d.pop');
  const kids = [...(pop?.children ?? [])].map((e) => e.getAttribute('data-kit'));
  expect(kids.indexOf('tp-note')).toBe(kids.indexOf('tp-head') + 1);
  expect(note?.parentElement).toBe(pop);
  expect(note?.closest('[data-kit="positioner"]')).toBeNull();
  expect(note?.getAttribute('role')).toBe('status');
});
