// Home's chunk (ADR-0105: lazy in the app, @wisp/shell loadHome) loaded before each test file, as the desktop's frame
// waits for it before drawing: a test that starts the app then finds Home at once, and the import isn't timed against
// the test (it compiles Home's whole graph, seconds on a loaded CI runner). Only in a DOM test (happy-dom): the node
// ones never start the app, and Home's graph needs a window.
export {};

if (typeof window !== 'undefined') await (await import('@wisp/shell')).loadHome();
