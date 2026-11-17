// A turn held before it starts (v1 features/hold.js, limit-hold.js): near the limit, a changed harness, project
// trust, a failed setup. It says what holds it and why; Start now (Trust and start for a changed harness) runs it
// anyway, Skip drops it.
import type { JSX } from 'solid-js';
import { Banner, Button, Inline } from '@wisp/ui';
import type { T } from './model.ts';
import '../i18n/lazy.ts';

export function HeldCard(props: { readonly title: string; readonly why: string; readonly start: string; readonly t: T; readonly onStart: () => void; readonly onSkip: () => void }): JSX.Element {
  return (
    <Banner
      part="turn.held"
      tone="warn"
      icon="clock"
      title={props.title}
      action={
        <Inline gap="2">
          <Button size="sm" variant="ghost" onClick={props.onSkip}>
            {props.t('shell-task.held.skip')}
          </Button>
          <Button size="sm" onClick={props.onStart}>
            {props.start}
          </Button>
        </Inline>
      }
    >
      {props.why}
    </Banner>
  );
}
