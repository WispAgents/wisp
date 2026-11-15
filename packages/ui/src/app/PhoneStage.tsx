// The shell's Stage in the phone apps (ADR-0065 step 8): the kit's Frame. Its own chunk, loaded with the look
// (platform/styles.ts), so the browser never downloads it.
import { children, type JSX } from 'solid-js';
import { Frame } from '../shell/Frame.tsx';
import type { StageProps } from './Stage.tsx';

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
      listLabel={props.listLabel}
      mainLabel={props.mainLabel}
      sideLabel={props.sideLabel}
    />
  );
}

export default PhoneStage;
