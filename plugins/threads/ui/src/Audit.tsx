// The compaction audit (ADR-0096, setting `threads.compactAudit`): what a compaction kept of the goal, constraints,
// decisions and what failed. One audit as a line ("Kept 6 of 8 after the compaction", when) with what it lost, item
// by item (each under its section); still asking, or skipped and why. The Thread tab lists a task's audits, newest
// first; under the compaction's turn shows that turn's (AuditFoot.tsx).
import { For, Match, Show, Switch, type JSX } from 'solid-js';
import { Icon, Inline, Pill, Stack, Text, Time } from '@wisp/ui';
import type { Key, Vars } from './i18n.ts';
import './i18n/lazy.ts';
import { lostItem } from './model.ts';
import type { Audit } from './types.ts';

type T = (key: Key, vars?: Vars) => string;

export function AuditLine(props: { readonly audit: Audit; readonly now: number; readonly t: T; readonly lost?: boolean }): JSX.Element {
  const a = (): Audit => props.audit;
  const all = (): boolean => a().kept >= a().total;
  return (
    <Stack gap="2" part="threads.audit" aria-label={props.t('threads.audit.label')}>
      {/* the time right after its mark, so the words are what gives way on a narrow panel (a time is never cut) */}
      <Inline gap="2" align="center" part="threads.audit.line">
        <Switch>
          <Match when={a().status === 'running'}>
            <Icon name="clock" size="sm" tone="faint" />
            <Time part="threads.audit.time" at={a().at} now={Math.max(props.now, a().at)} layout="inline" />
            <Text size="sm" tone="muted" truncate part="threads.audit.running">
              {props.t('threads.audit.running')}
            </Text>
          </Match>
          <Match when={a().status === 'skipped'}>
            <Icon name="minus" size="sm" tone="faint" />
            <Time part="threads.audit.time" at={a().at} now={Math.max(props.now, a().at)} layout="inline" />
            <Text size="sm" tone="muted" truncate title={a().why ?? ''} part="threads.audit.skipped">
              {a().why ? props.t('threads.audit.skippedWhy', { why: a().why ?? '' }) : props.t('threads.audit.skipped')}
            </Text>
          </Match>
          <Match when={a().status === 'done'}>
            <Pill part="threads.audit.kept" size="sm" tone={all() ? 'good' : 'warn'} title={props.t('threads.audit.keptTip')}>
              {props.t('threads.audit.kept', { kept: a().kept, total: a().total })}
            </Pill>
            <Time part="threads.audit.time" at={a().at} now={Math.max(props.now, a().at)} layout="inline" />
            <Text size="sm" tone="muted" truncate part="threads.audit.what">
              {props.t(all() ? 'threads.audit.all' : 'threads.audit.after')}
            </Text>
          </Match>
        </Switch>
      </Inline>
      <Show when={props.lost !== false && a().status === 'done' && a().lost.length > 0}>
        <Stack gap="1" part="threads.audit.lost">
          <Text size="xs" tone="faint" part="threads.audit.lostHead">
            {props.t(a().told ? 'threads.audit.lostTold' : 'threads.audit.lostNext')}
          </Text>
          <For each={a().lost}>
            {(x) => {
              const item = lostItem(x);
              // one line of content that wraps as a whole: its section, then the item
              return (
                <Text size="sm" as="div" part="threads.audit.item">
                  <Show when={item.section}>
                    <Text tone="faint" part="threads.audit.section">
                      {item.section}
                    </Text>
                    {' · '}
                  </Show>
                  <Text part="threads.audit.text">{item.text}</Text>
                </Text>
              );
            }}
          </For>
        </Stack>
      </Show>
    </Stack>
  );
}

/** The Thread tab's Compactions: every audit, newest first; only the newest lists what it lost. */
export function Compactions(props: { readonly audits: readonly Audit[]; readonly now: number; readonly t: T }): JSX.Element {
  return (
    <Show when={props.audits.length > 0}>
      <Stack as="section" gap="3" part="threads.compactions" aria-label={props.t('threads.compactions')}>
        <Text weight="semibold" size="sm" nowrap part="threads.compactions.label">
          {props.t('threads.compactions')}
        </Text>
        <For each={props.audits}>{(a, i) => <AuditLine audit={a} now={props.now} t={props.t} lost={i() === 0} />}</For>
      </Stack>
    </Show>
  );
}
