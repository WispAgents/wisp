// What `node qa/layout.mjs --area=<area>` visits in the app (apps/web on the mock peer seeded with v1's
// reference data, design/v1-reference/seed.json, as qa/parity.mjs runs it). One entry per area; add
// yours: each page is a path (navigation state is in the URL) and a selector that shows it's drawn.

import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const SEED = JSON.parse(fs.readFileSync(fileURLToPath(new URL('../design/v1-reference/seed.json', import.meta.url)), 'utf8'));
/** the first seeded task (qa/views.mjs `task` opens it), as the mock names it: `local:<v1 id>` */
const TASK = `/task/${encodeURIComponent(`local:${SEED.seeded.tasks[0]}`)}`;

export const AREAS = {
  shell: [
    // `phoneReady`: the phone apps draw these as their own screens (ADR-0065 step 13: the Tasks bar over the list, the
    // task's bar), with no sidebar or split head, so the browser's selectors never showed there
    { name: 'home', path: '/', ready: '[data-kit="stage"] [data-part="sidebar"] [data-part="card"]', phoneReady: '[data-part~="phone.tasks.bar"]' },
    { name: 'task', path: TASK, ready: '[data-kit="stage"] [data-part="split.head"]', phoneReady: '[data-part~="phone.task.bar"]' },
    { name: 'palette', path: '/?o=dialog:shell.palette', ready: '[data-part="shell.palette"]' },
    { name: 'shortcuts', path: '/?o=dialog:shell.shortcuts', ready: '[data-part="shell.shortcuts"]' },
    { name: 'diagnostics', path: '/?o=viewer:shell.devtools', ready: '[data-part="shell.devtools"]' },
  ],
};
