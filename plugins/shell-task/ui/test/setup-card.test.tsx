// ADR-0113 (owner-list row 66): an agent asking to change the user's setup gets a card on its task: what it would
// change, each command it would run verbatim, Allow and Deny only (v1 asked each time: no Always), and nothing to type
// for it to go ahead. A runtime ask with `setup` is that card, never a tool approval (whose Allow comes with Always).
import { expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { KitProvider } from '@wisp/ui';
import { AskCard } from '../src/cards/AskCard.tsx';
import { askOf } from '../src/foot/model.ts';
import { answer } from '../src/foot/TaskFoot.tsx';
import en from '../src/i18n/en.lazy.json' with { type: 'json' };

const t = (k: string): string => (en as Record<string, string>)[k] ?? k;

const row = {
  body: {
    ext: {
      'wisp.runtime': {
        ask: {
          id: 'a1',
          question: 'Ada wants to add the connector “linear”: runs npx -y @linear/mcp\nEvery task will start it, on this computer.',
          options: ['Allow', 'Deny'],
          setup: {
            command: 'connectors.agent.add',
            headline: 'Ada wants to add the connector “linear”: runs npx -y @linear/mcp',
            lines: ['Every task will start it, on this computer.', 'needs LINEAR_KEY from you'],
            commands: ['npx -y @linear/mcp'],
            full: 'Every task will start it, on this computer.\nneeds LINEAR_KEY from you',
          },
        },
      },
    },
  },
};

test('a setup ask is its own card, not a tool approval_setup_card_model', () => {
  const a = askOf(row);
  expect(a?.approval).toBe(false);
  expect(a?.setup?.commands).toEqual(['npx -y @linear/mcp']);
  // a plain question whose options include Allow is still an approval (unchanged)
  const plain = askOf({ body: { ext: { 'wisp.runtime': { ask: { id: 'q', question: 'Run it?', options: ['Allow', 'Deny'] } } } } });
  expect(plain?.approval).toBe(true);
  expect(plain?.setup).toBeUndefined();
});

test('the setup card shows what changes and each command verbatim, with Allow and Deny only_setup_card_render', () => {
  const answers: string[] = [];
  const host = document.createElement('div');
  document.body.append(host);
  const a = askOf(row);
  if (!a) throw new Error('no ask');
  const dispose = render(
    () => (
      <KitProvider t={(k) => k}>
        <AskCard ask={a} t={t} onAnswer={(x) => answers.push(x)} />
      </KitProvider>
    ),
    host,
  );
  const card = host.querySelector('[data-part="task.ask"]') as HTMLElement;
  expect(card.textContent).toContain('Wants to change your setup');
  expect(host.querySelector('[data-part="task.ask.setup.headline"]')?.textContent).toBe('Ada wants to add the connector “linear”: runs npx -y @linear/mcp');
  const cmd = host.querySelector('[data-part="task.ask.setup.command"]') as HTMLElement;
  expect(cmd.textContent).toBe('npx -y @linear/mcp');
  expect(cmd.getAttribute('data-mono')).not.toBeNull();
  expect([...host.querySelectorAll('[data-part="task.ask.setup.line"]')].map((l) => l.textContent)).toEqual([
    'Every task will start it, on this computer.',
    'needs LINEAR_KEY from you',
  ]);
  // everything that will be written, word for word, behind Show all (review r194 row 66 #4)
  expect(host.querySelector('[data-part="task.ask.setup.full"]')).toBeNull();
  (host.querySelector('[data-part="task.ask.setup.showAll"]') as HTMLButtonElement).click();
  expect([...host.querySelectorAll('[data-part="task.ask.setup.full"]')].map((l) => l.textContent)).toEqual(['Every task will start it, on this computer.', 'needs LINEAR_KEY from you']);
  const buttons = [...card.querySelectorAll('button')].map((b) => b.textContent).filter((b) => b === 'Allow' || b === 'Deny');
  expect(buttons).toEqual(['Allow', 'Deny']);
  for (const b of card.querySelectorAll('button')) if (b.textContent === 'Allow' || b.textContent === 'Deny') (b as HTMLButtonElement).click();
  expect(answers).toEqual(['Allow', 'Deny']);
  dispose();
  host.remove();
});

/** Review r194 row 66 #1: an answer names the question it answers, so one meant for a card that's gone runs nothing. */
test('the answer names the card it answers_setup_card_answer_names_its_card', () => {
  const wire = answer.wire;
  if (!wire) throw new Error('no wire');
  expect(wire({ key: 'local:s1' as never, id: 's1', text: 'Allow', askId: 'a1' })).toEqual({ id: 's1', text: 'Allow', askId: 'a1' });
});


// Audit G tasks #9: Show all showed the JSON and was offered when nothing was cut; it now swaps the cut lines for the
// whole ones, in the card's words, and only when the server sent them
test('show_all_swaps_the_cut_lines_for_the_whole_ones_and_is_offered_only_when_something_is_cut_audit_g_tasks_9', () => {
  const card = (full: string) => ({
    body: { ext: { 'wisp.runtime': { ask: { id: 'a2', question: 'q', options: ['Allow', 'Deny'], setup: { command: 'crew.agent.add', headline: 'Ada wants to add crew member revbot', lines: ['brief: You review…', 'cwd: /code'], commands: [], full } } } } },
  });
  const host = document.createElement('div');
  document.body.append(host);
  const draw = (full: string) => {
    const a = askOf(card(full));
    if (!a) throw new Error('no ask');
    return render(
      () => (
        <KitProvider t={(k) => k}>
          <AskCard ask={a} t={t} onAnswer={() => undefined} />
        </KitProvider>
      ),
      host,
    );
  };
  let dispose = draw('');
  expect(host.querySelector('[data-part="task.ask.setup.showAll"]')).toBeNull();
  dispose();
  dispose = draw('brief: You review every pull request.\ncwd: /code');
  (host.querySelector('[data-part="task.ask.setup.showAll"]') as HTMLButtonElement).click();
  const lines = [...host.querySelectorAll('[data-part="task.ask.setup.full"], [data-part="task.ask.setup.line"]')].map((l) => l.textContent);
  expect(lines).toEqual(['brief: You review every pull request.', 'cwd: /code']);
  expect(host.textContent).not.toContain('{');
  expect(host.querySelector('[data-part="task.ask.setup.showAll"]')?.textContent).toBe('Show less');
  dispose();
  host.remove();
});

// Audit G tasks #5: a factory question quoting a worktree path ran out of its card on a 360 px phone, cut; the question
// breaks anywhere, like a setup card's lines
test('a_question_quoting_a_long_path_breaks_inside_its_card_audit_g_tasks_5', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const q = "Checks still fail after 3 rounds: Cannot find module '/private/tmp/qa/wisphome/worktrees/app-01a1f8f3ef0372d7aad81a95b648685f/math.test.js'. Keep going?";
  const a = askOf({ body: { ext: { 'wisp.runtime': { ask: { id: 'f2', question: q, options: ['One more round', 'Stop'] } } } } });
  if (!a) throw new Error('no ask');
  const dispose = render(
    () => (
      <KitProvider t={(k) => k}>
        <AskCard ask={a} t={t} onAnswer={() => undefined} />
      </KitProvider>
    ),
    host,
  );
  const text = host.querySelector('[data-part="task.ask.question"]');
  expect(text?.textContent).toBe(q);
  expect(text?.getAttribute('data-break')).not.toBeNull();
  dispose();
  host.remove();
});
