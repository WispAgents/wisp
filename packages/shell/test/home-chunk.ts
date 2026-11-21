// Home's chunk (ADR-0105: lazy in the app, frame/Panes.tsx loadHome) loaded before each test file, as the desktop's
// frame waits for it before drawing: a test that starts the app then finds Home at once, and the import isn't timed
// against the test (it compiles Home's whole graph, seconds on a loaded CI runner).
import { loadHome } from '../src/frame/Panes.tsx';

await loadHome();
