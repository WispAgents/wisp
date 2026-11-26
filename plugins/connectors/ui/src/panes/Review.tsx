// Review what the harnesses' own configs have (v1's import review): each found server with where it is and
// what it runs, ticked by default; Add takes the ticked ones, values and all. The ones "Not now" hid wait behind Show
// hidden, unticked: hidden isn't gone, and adding one unhides it.
import { createEffect, createSignal, For, Show, type JSX } from 'solid-js';
import { Button, Checkbox, Dialog, Inline, SettingsRow, Stack, Text } from '@wisp/ui';
import { t } from '../i18n.ts';
import { foundLine } from '../lib.ts';
import type { FoundServer } from '../types.ts';
import '../i18n/lazy.ts';

export interface ReviewProps {
  readonly open: boolean;
  /** every found server, the hidden ones (`hidden`) too */
  readonly found: readonly FoundServer[];
  /** open with the hidden ones shown (the card had nothing else to offer) */
  readonly showHidden?: boolean;
  readonly onImport: (names: string[]) => void;
  readonly onClose: () => void;
}

export function Review(props: ReviewProps): JSX.Element {
  const [picked, setPicked] = createSignal<ReadonlySet<string>>(new Set());
  const [showing, setShowing] = createSignal(false);
  const shown = (): FoundServer[] => props.found.filter((f) => f.hidden !== true);
  const hidden = (): FoundServer[] => props.found.filter((f) => f.hidden === true);
  createEffect(() => {
    if (props.open) {
      setPicked(new Set(shown().map((f) => f.name)));
      setShowing(props.showHidden === true);
    }
  });
  const flip = (n: string, on: boolean): void => {
    const s = new Set(picked());
    if (on) s.add(n);
    else s.delete(n);
    setPicked(s);
  };
  const row = (f: FoundServer): JSX.Element => (
    <SettingsRow
      part="connectors.review.row"
      title={f.name}
      description={
        <Text tone="muted" truncate>
          {`${foundLine(f)} · ${f.from.join(', ')}`}
        </Text>
      }
      control={<Checkbox aria-label={f.name} checked={picked().has(f.name)} onChange={(on) => flip(f.name, on === true)} />}
    />
  );
  return (
    <Dialog
      open={props.open}
      onOpenChange={(o) => {
        if (!o) props.onClose();
      }}
      title={t('connectors.review.title')}
      description={t('connectors.review.description')}
      part="connectors.review"
      footer={
        <Inline gap="3" justify="end">
          <Button variant="ghost" size="sm" onClick={props.onClose}>
            {t('connectors.cancel')}
          </Button>
          <Button variant="primary" size="sm" part="connectors.review.add" disabled={picked().size === 0} onClick={() => props.onImport([...picked()])}>
            {t('connectors.review.add', { count: picked().size })}
          </Button>
        </Inline>
      }
    >
      <Stack gap="1">
        <For each={shown()}>{row}</For>
        <Show when={hidden().length > 0}>
          <Show
            when={showing()}
            fallback={
              <Inline>
                <Button variant="ghost" size="sm" part="connectors.review.showHidden" onClick={() => setShowing(true)}>
                  {t('connectors.review.showHidden', { count: hidden().length })}
                </Button>
              </Inline>
            }
          >
            <For each={hidden()}>{row}</For>
            <Inline>
              <Button variant="ghost" size="sm" part="connectors.review.hideHidden" onClick={() => {
                  // hidden again: none of them goes with Add unseen
                  const gone = new Set(hidden().map((f) => f.name));
                  setPicked(new Set([...picked()].filter((n) => !gone.has(n))));
                  setShowing(false);
                }}>
                {t('connectors.review.hideHidden')}
              </Button>
            </Inline>
          </Show>
        </Show>
      </Stack>
    </Dialog>
  );
}
