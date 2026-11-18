// The phone's Crew section at the top of the list (v1 #secCrew): with no rail, the crew is reached here. A folding
// Crew heading with Org and +, a row per member opening their thread, a dashed row to add the first one. Under a member,
// its jobs for other tasks (delegated, @mentions, checks) as the list's own cards, the most urgent first, 3 at most
// (v1 crew.js paintCrew / memberJobs): the list lends its card (ShellApi.taskCards) and leaves those jobs out.
import { lazy, Show, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { useShell } from '@wisp/shell';
import { useSignal } from '@wisp/ui';

// the phone apps (data-platform android | ios): the crew as stories under the Tasks top bar (step 13b); its own chunk
const Stories = lazy(() => import('./Stories.tsx'));
// the browser's section: its own chunk with the kit's Org pieces (the phone apps never load it)
const CrewSection = lazy(() => import('./CrewSection.tsx'));

/** The browser (a phone's too) keeps v1's Crew section; the phone apps draw the stories row in its place. */
export default function CrewListTop(props: ExtensionProps): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  return (
    <Show when={shell.platform.phone} fallback={<CrewSection {...props} />}>
      <Show when={nav().layout === 'stack'}>
        <Stories {...props} />
      </Show>
    </Show>
  );
}
