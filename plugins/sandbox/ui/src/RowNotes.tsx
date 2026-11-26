// Under a sandbox's row in Settings → Computers → Sandboxes: why its Delete was refused, and Codex signing in there
// (starting, then its page and device code, then done or why not), where it was asked for, never in a toast.
import { Show, type JSX } from 'solid-js';
import { Button, Callout, CopyCommand, Stack, Text } from '@wisp/ui';
import { t } from './i18n.ts';
import type { CodexFlow } from './model.ts';
import './i18n/lazy.ts';

export function RowNotes(props: {
  readonly codex: CodexFlow | undefined;
  /** its Delete was refused: the grid's words */
  readonly error: string | undefined;
  readonly onOpen: (url: string) => void;
  readonly onCopy: (code: string) => void;
}): JSX.Element {
  const link = (): Extract<CodexFlow, { state: 'link' }> | undefined => (props.codex?.state === 'link' ? props.codex : undefined);
  const failed = (): string | undefined => (props.codex?.state === 'failed' ? props.codex.error : undefined);
  return (
    <Show when={props.codex || props.error}>
      <Stack gap="2" part="sandbox.codex">
        <Show when={props.error}>
          {(why) => (
            <Callout tone="bad" title={t('sandbox.list.removeFailed')} part="sandbox.row.error">
              {why()}
            </Callout>
          )}
        </Show>
        <Show when={props.codex?.state === 'starting'}>
          <Text as="p" size="sm" tone="muted" part="sandbox.codex.waiting">
            {t('sandbox.codex.waiting')}
          </Text>
        </Show>
        <Show when={link()}>
          {(f) => (
            <Stack gap="2" part="sandbox.codex.flow">
              <span>
                <Button size="sm" variant="secondary" icon="external" part="sandbox.codex.open" onClick={() => props.onOpen(f().url)}>
                  {t('sandbox.codex.open')}
                </Button>
              </span>
              <Show when={f().code}>
                {(code) => (
                  <Stack gap="1">
                    <Text as="p" size="sm" tone="muted">
                      {t('sandbox.codex.code')}
                    </Text>
                    <CopyCommand bare part="sandbox.codex.code" command={code()} label={t('sandbox.codex.copy')} onCopy={props.onCopy} />
                  </Stack>
                )}
              </Show>
            </Stack>
          )}
        </Show>
        <Show when={props.codex?.state === 'done'}>
          <Text as="p" size="sm" tone="good" part="sandbox.codex.done">
            {t('sandbox.codex.done')}
          </Text>
        </Show>
        <Show when={failed() !== undefined}>
          <Callout tone="bad" title={t('sandbox.codex.failed')} part="sandbox.codex.error">
            {failed() ?? ''}
          </Callout>
        </Show>
      </Stack>
    </Show>
  );
}
