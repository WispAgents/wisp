// The shell's Stage in the phone apps (ADR-0065 step 8): the kit's Frame. Its own chunk, loaded with the look
// (platform/styles.ts), so the browser never downloads it.
import { children, type JSX } from 'solid-js';
import { Frame } from '../shell/Frame.tsx';
import type { StageProps } from './Stage.tsx';
import { settingsPhone } from '../settings/settings-phone-load.ts';
import { afterFirstScreen } from '../controls/lazy-part.ts';

// Settings' phone pages (step 14) come with the phone frame once the first screen is drawn (ADR-0105: loaded with the
// frame they and their stylesheets were on every phone's first screen, perf r1 #2), so opening Settings from the me menu
// still draws them at once. From here, not platform/styles.ts, so the browser's first download doesn't even carry their
// chunk's name.
afterFirstScreen(() => void settingsPhone.load().catch(() => undefined));

/** The phone apps (design/mobile.md §A.1): one page at a time under 900 px, the side pane as its own page over the
 *  task; the crew rail gives way to the Crew destination. */
export function PhoneStage(props: StageProps): JSX.Element {
  // resolved once per change: `side` is a getter that builds its pane on every read
  const side = children(() => props.side);
  const open = (): boolean => side() !== undefined && side() !== null;
  const page = (): 'list' | 'main' | 'side' => (props.page === 'main' && open() ? 'side' : props.page);
  return (
    <Frame
      layout={props.layout}
      page={page()}
      // the list's head is the Tasks screen's TopAppBar (the shell's PhoneHead, step 13), the frame's top itself; a pushed
      // page draws its own head, so the frame's top steps aside there (v1 stack)
      top={props.head}
      list={props.list}
      main={props.main}
      side={side()}
      sideOpen={open()}
      nav={props.nav}
      // New task over the list (stack-page.ts): the platform's full-screen dialog / sheet
      behind={props.layout === 'stack' && page() === 'main' && props.listBehind === true}
      // a task (and its side pane) is a pushed detail: the destinations give way under it (UX 2026-11-18 phone #2)
      pushed={props.pushed === true}
      listLabel={props.listLabel}
      mainLabel={props.mainLabel}
      sideLabel={props.sideLabel}
    />
  );
}

export default PhoneStage;
