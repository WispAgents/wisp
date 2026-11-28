// The filter rows an issue or review trigger asks (trigger-filters.ts): whose, from whom, its milestone, and for a
// review: drafts, paths and size. Each writes the `match` the CLI's flags write (`--assignee me`, `--reviewer me`,
// `--from people|trusted`, `--milestone`, `--no-drafts`, `--paths`, `--max-size`).
import { Show, type JSX } from 'solid-js';
import { AutoRow, Input, Select } from '@wisp/ui';
import { t } from './i18n.ts';
import { FILTERS_OF, FROM, SIZES, type Filters } from './trigger-filters.ts';
import './i18n/lazy.ts';

const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;

interface Props {
  readonly kind: string;
  readonly f: Filters;
  readonly set: (patch: Partial<Filters>) => void;
}

export function TriggerFilters(props: Props): JSX.Element {
  const offer = () => FILTERS_OF[props.kind];
  const whoseLabel = () => say(props.kind === 'review' ? 'automations.form.whose.review' : 'automations.form.whose');
  return (
    <Show when={offer()}>
      {(o) => (
        <>
          <AutoRow label={whoseLabel()}>
            <Select
              aria-label={whoseLabel()}
              value={props.f.whose}
              onChange={(v) => props.set({ whose: o().whose.find((w) => w === v) ?? '' })}
              items={o().whose.map((w) => ({ value: w, label: say(`automations.form.whose.${w || 'anyone'}`) }))}
            />
          </AutoRow>
          <AutoRow label={say('automations.form.from')}>
            <Select
              aria-label={say('automations.form.from')}
              value={props.f.from}
              onChange={(v) => props.set({ from: FROM.find((x) => x === v) ?? '' })}
              items={FROM.map((x) => ({ value: x, label: say(`automations.form.from.${x || 'anyone'}`) }))}
            />
          </AutoRow>
          <Show when={o().review}>
            {/* a yes/no in this form is a Select, as an MCP event's boolean argument is (McpEventFields) */}
            <AutoRow label={say('automations.form.drafts')}>
              <Select
                aria-label={say('automations.form.drafts')}
                value={props.f.skipDrafts ? 'skip' : ''}
                onChange={(v) => props.set({ skipDrafts: v === 'skip' })}
                items={[
                  { value: '', label: say('automations.form.drafts.include') },
                  { value: 'skip', label: say('automations.form.drafts.skip') },
                ]}
              />
            </AutoRow>
            <AutoRow label={say('automations.form.size')}>
              <Select
                aria-label={say('automations.form.size')}
                value={props.f.size}
                onChange={(v) => props.set({ size: SIZES.find((x) => x === v) ?? '' })}
                items={SIZES.map((x) => ({ value: x, label: x ? say('automations.form.size.upTo', { n: x }) : say('automations.form.size.any') }))}
              />
            </AutoRow>
            <AutoRow label={say('automations.form.paths')}>
              <Input
                mono
                value={props.f.paths}
                onInput={(v) => props.set({ paths: v })}
                spellcheck={false}
                placeholder={say('automations.form.pathsPlaceholder')}
                aria-label={say('automations.form.paths')}
              />
            </AutoRow>
          </Show>
          <AutoRow label={say('automations.form.milestone')}>
            <Input
              value={props.f.milestone}
              onInput={(v) => props.set({ milestone: v })}
              spellcheck={false}
              placeholder={say('automations.form.milestonePlaceholder')}
              aria-label={say('automations.form.milestone')}
            />
          </AutoRow>
        </>
      )}
    </Show>
  );
}
