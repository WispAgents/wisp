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
            full: '{\n  "name": "linear",\n  "command": "npx"\n}',
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
  expect(host.querySelector('[data-part="task.ask.setup.full"]')?.textContent).toContain('"command": "npx"');
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

