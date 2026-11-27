// The kit's one spinner (a Button's loading, a control's pending act), drawn as the kit's Liquid (ADR-0140): it always says what it's doing in its tooltip,
// and when it has been spinning a while it says so instead ("Still waiting: …"), so a command waiting on a computer
// that's away is never a silent, endless spin (TRY-12).
import { createSignal, type JSX } from 'solid-js';
import { useT } from '../core/i18n.tsx';
import { useTimeout } from '../blocks/rich-scope.ts';
import { Liquid } from '../content/Liquid.tsx';

/** how long before the tooltip says it's still waiting */
export const SLOW_MS = 15_000;

export function Spinner(props: { readonly label?: string; readonly in?: string }): JSX.Element {
  const t = useT();
  const [slow, setSlow] = createSignal(false);
  // the component's own timeout (rich-scope.ts): cleared when the spinner goes
  useTimeout().start(SLOW_MS, () => setSlow(true));
  const text = (): string => (slow() ? t('loading.slow') : (props.label ?? t('loading')));
  return (
    <span data-kit="spinner" data-in={props.in} data-slow={slow() ? '' : undefined} role="status" aria-label={text()} title={text()}>
      <Liquid />
    </span>
  );
}
