// Home's chunk (ADR-0105: lazy in the app, @wisp/shell loadHome) and the open task's lazy chunks (TaskView, the list,
// the side panel and the task's foot/turn extensions) loaded before each test file through solid-js lazy `.preload()`,
// as the desktop's frame waits for Home before drawing and warms the task page while idle: a test that starts the app
// or opens a task then finds them at once, and the dynamic imports aren't timed against the test's 5 s clock (compiling
// TaskView's graph takes seconds on a loaded Windows CI runner). Only in a DOM test (happy-dom): the node ones never
// start the app, and the UI graph needs a window.
export {};

const CORE_PLUGINS = new Set(['wisp.shell-list', 'wisp.shell-task']);

if (typeof window !== 'undefined') {
  const [{ loadHome, TaskView }, { pluginUis }] = await Promise.all([import('@wisp/shell'), import('../src/plugins.ts')]);
  const preload = (c: unknown): Promise<unknown> | undefined => (c as { preload?: () => Promise<unknown> } | null | undefined)?.preload?.();
  await Promise.all([
    loadHome(),
    preload(TaskView),
    ...pluginUis()
      .filter(({ def }) => def && CORE_PLUGINS.has(def.id))
      .flatMap(({ def }) => (def?.extensions ?? []).map((e) => preload(e.component))),
  ]);
}
