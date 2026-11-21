// New task's `+` → Formula or task file… (owner-list #61, ADR-0111): what a picked file adds to the task, from the
// peer's `resources.overlay` answer, and what a refused one does (a toast saying what there is, nothing attached).
import { describe, expect, it, vi } from 'vitest';
import { pickArgs, pickFormula, type FormulaPick } from '../src/frame/formula-pick.ts';

describe('formula or task file in New task', () => {
  it('a formula goes as the task overlay; a task file adds its own fields and its prompt', () => {
    expect(pickArgs({ overlay: { agents: [{ name: 'critic', harness: 'codex' }] } })).toEqual({ args: { formula: { agents: [{ name: 'critic', harness: 'codex' }] } } });
    expect(pickArgs({ overlay: {} })).toEqual({ args: {} });
    const t = pickArgs({
      overlay: { agents: [{ name: 'critic' }] },
      task: { prompt: 'fix the build', agent: 'critic', goal: { text: 'green' }, cwd: '/elsewhere', connectors: ['linear'], model: '' },
    });
    // its folder is New task's Where, an empty field isn't sent
    expect(t).toEqual({ args: { formula: { agents: [{ name: 'critic' }] }, agent: 'critic', goal: { text: 'green' }, connectors: ['linear'] }, prompt: 'fix the build' });
  });

  it('reads the picked file on the peer and attaches it; a refusal is a toast and nothing is attached', async () => {
    const file = { name: 'fix.task.yml', text: async () => 'wisp: 1\ntask: { prompt: fix it }\nformula: ./wisp.yml\n' } as unknown as File;
    const run = vi.fn(async (_cmd: unknown, a: { text: string; name: string; cwd?: string }) => {
      expect(a).toMatchObject({ peer: 'p', name: 'fix.task.yml', cwd: '/w/app' });
      return { overlay: { agents: [{ name: 'critic' }] }, task: { prompt: 'fix it', agent: 'critic' } };
    });
    const error = vi.fn();
    const shell = { platform: { pickFiles: async () => [file] }, client: { commands: { run } }, toast: { error } } as never;
    const picked: FormulaPick[] = [];
    const filled: string[] = [];
    await pickFormula(shell, 'p', '/w/app', (p) => picked.push(p), (v) => filled.push(v));
    expect(picked).toEqual([{ name: 'fix.task.yml', args: { formula: { agents: [{ name: 'critic' }] }, agent: 'critic' } }]);
    expect(filled).toEqual(['fix it']);
    expect(error).not.toHaveBeenCalled();

    run.mockResolvedValueOnce({ overlay: {}, problems: ['fix.task.yml: no agent named nosuch: agents has critic (or codex, auto)'] } as never);
    await pickFormula(shell, 'p', '/w/app', (p) => picked.push(p), (v) => filled.push(v));
    expect(error).toHaveBeenCalledWith('fix.task.yml: no agent named nosuch: agents has critic (or codex, auto)');
    expect(picked).toHaveLength(1);

    const none = { platform: { pickFiles: async () => [] }, client: { commands: { run } }, toast: { error } } as never;
    await pickFormula(none, 'p', undefined, (p) => picked.push(p), (v) => filled.push(v));
    expect(run).toHaveBeenCalledTimes(2);
  });

  // review r194 #5: a picked file has no path, so its `./x` is read from the project: New task says so
  it('says when a picked task file was read from the project folder', async () => {
    const file = { name: 'fix.task.yml', text: async () => 'wisp: 1\ntask: { prompt: x }\nformula: ./wisp.yml\n' } as unknown as File;
    const note = 'fix.task.yml: ./wisp.yml was read from the project folder /w/app: a picked file\'s own folder isn\'t known here';
    const run = vi.fn(async () => ({ overlay: { agents: [{ name: 'a' }] }, task: { prompt: 'x' }, notes: [note] }));
    const show = vi.fn();
    const shell = { platform: { pickFiles: async () => [file] }, client: { commands: { run } }, toast: { error: vi.fn(), show } } as never;
    await pickFormula(shell, 'p', '/w/app', () => undefined, () => undefined);
    expect(show).toHaveBeenCalledWith(note, { tone: 'info' });
  });
});
