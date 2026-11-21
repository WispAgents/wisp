// The harness a prompt bubble is marked with: the one its own turn ran on (runtime's Turn.agent, on the turn's row
// body), not the reply box's pick for the next follow-up (audit B #6: picking Codex re-marked turn 1, which ran on
// Claude Code). The follow-up just sent has no row of the peer's yet: it shows the pick it was sent with, else the
// task's harness. Pure.

export function promptHarness(body: { readonly agent?: unknown } | undefined, picked: string | undefined, task: string): string {
  const own = body?.agent;
  if (typeof own === 'string' && own) return own;
  return picked || task;
}
