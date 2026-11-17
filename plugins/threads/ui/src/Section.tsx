// One section of the Notes (variant A, v1's Thread tab): its name, who wrote it last and when, its earlier
// versions, and its text; the pen (or Add) opens it in place as a box: Enter saves, Shift+Enter is a new line, Esc
// puts it back. A list section is one item a line, shown as bullets.
import { For, Show, createSignal, type JSX } from 'solid-js';
import { Box, Button, IconButton, Inline, Markdown, Popover, Stack, Text, TextArea, Time } from '@wisp/ui';
import type { Key, Vars } from './i18n.ts';
import { shownText, type SectionView } from './model.ts';
import type { Revision } from './types.ts';
import './i18n/lazy.ts';

type T = (key: Key, vars?: Vars) => string;

export interface SectionProps {
  readonly section: SectionView;
  readonly revisions: readonly Revision[];
  readonly readOnly: boolean;
  readonly now: number;
  /** open as a box (an empty section picked from the Add row) */
  readonly editing: boolean;
  readonly onEdit: (on: boolean) => void;
  readonly onSave: (text: string) => void;
  readonly t: T;
}

function History(props: { readonly section: SectionView; readonly revisions: readonly Revision[]; readonly readOnly: boolean; readonly now: number; readonly onUse: (text: string) => void; readonly t: T }): JSX.Element {
  const name = (): string => props.t(`threads.section.${props.section.id}`);
  const [open, setOpen] = createSignal(false);
  return (
    <Popover
      part="threads.history"
      title={props.t('threads.history.title', { section: name() })}
      trigger={{ icon: 'clock', 'aria-label': props.t('threads.history'), variant: 'ghost', size: 'xs' }}
      open={open()}
      onOpenChange={setOpen}
    >
      <Stack gap="4" part="threads.history.list">
        <For each={props.revisions} fallback={<Text tone="muted" size="sm">{props.t('threads.history.none')}</Text>}>
          {(r) => (
            <Stack gap="2" part="threads.history.item">
              <Inline gap="2" align="center" part="threads.history.meta">
                <Text size="xs" tone="faint" nowrap>
                  {props.t(`threads.by.${r.by}`)}
                </Text>
                <Text size="xs" tone="faint">
                  ·
                </Text>
                <Time part="threads.history.time" at={r.at} now={Math.max(props.now, r.at)} layout="inline" />
                <Box grow />
                <Show when={!props.readOnly}>
                  <Button
                    part="threads.history.use"
                    variant="ghost"
                    size="xs"
                    icon="replay"
                    onClick={() => {
                      setOpen(false);
                      props.onUse(r.text);
                    }}
                  >
                    {props.t('threads.history.use')}
                  </Button>
                </Show>
              </Inline>
              <Show when={r.text.trim()} fallback={<Text size="sm" tone="faint">{props.t('threads.history.empty')}</Text>}>
                <Markdown part="threads.history.text" source={shownText(r.section, r.text)} />
              </Show>
            </Stack>
          )}
        </For>
      </Stack>
    </Popover>
  );
}

function Editor(props: { readonly section: SectionView; readonly onSave: (text: string) => void; readonly onCancel: () => void; readonly t: T }): JSX.Element {
  const [text, setText] = createSignal(props.section.text);
  const save = (): void => props.onSave(text());
  const onKey = (e: KeyboardEvent): void => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      save();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation(); // the box, not the panel or the sheet behind it
      props.onCancel();
    }
  };
  return (
    <Stack gap="3" part="threads.section.editor">
      <TextArea
        part="threads.section.input"
        value={text()}
        onInput={setText}
        onKeyDown={onKey}
        placeholder={props.t(`threads.hint.${props.section.id}`)}
        aria-label={props.t('threads.edit', { section: props.t(`threads.section.${props.section.id}`) })}
        autofocus
        autosize
        minRows={props.section.one ? 2 : 3}
        maxRows={14}
      />
      <Inline gap="2" align="center" part="threads.section.actions">
        <Text size="xs" tone="faint" truncate title={props.t('threads.keys')}>
          {props.t('threads.keys')}
        </Text>
        <Box grow />
        <Button part="threads.section.cancel" variant="ghost" size="sm" onClick={() => props.onCancel()}>
          {props.t('threads.cancel')}
        </Button>
        <Button part="threads.section.save" variant="primary" size="sm" onClick={save}>
          {props.t('threads.save')}
        </Button>
      </Inline>
    </Stack>
  );
}

export function SectionItem(props: SectionProps): JSX.Element {
  const name = (): string => props.t(`threads.section.${props.section.id}`);
  const who = (): string => props.t(`threads.by.${props.section.by}`);
  return (
    <Stack as="section" gap="2" part="threads.section" aria-label={name()}>
      <Inline gap="2" align="center" part="threads.section.head">
        <Text weight="semibold" size="sm" nowrap part="threads.section.label">
          {name()}
        </Text>
        <Box grow />
        <Show when={props.section.rev > 0}>
          <Inline gap="1" align="center" part="threads.section.meta">
            <Text size="xs" tone="faint" nowrap title={props.t('threads.by.title', { who: who() })} part="threads.section.by">
              {who()}
            </Text>
            <Text size="xs" tone="faint">
              ·
            </Text>
            {/* the clock ticks every 30 s: a write you just made is "now", never a time ahead of it */}
            <Time part="threads.section.time" at={props.section.at} now={Math.max(props.now, props.section.at)} layout="inline" />
          </Inline>
        </Show>
        <Show when={props.revisions.length > 0}>
          <History section={props.section} revisions={props.revisions} readOnly={props.readOnly} now={props.now} onUse={props.onSave} t={props.t} />
        </Show>
        <Show when={!props.readOnly && !props.editing}>
          <IconButton part="threads.section.edit" icon="pen" size="xs" label={props.t('threads.edit', { section: name() })} onClick={() => props.onEdit(true)} />
        </Show>
      </Inline>
      <Show
        when={props.editing}
        fallback={<Markdown part="threads.section.text" source={shownText(props.section.id, props.section.text)} />}
      >
        <Editor section={props.section} onSave={props.onSave} onCancel={() => props.onEdit(false)} t={props.t} />
      </Show>
    </Stack>
  );
}
