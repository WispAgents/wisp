// The rows an MCP server event trigger asks (ADR-0145): which remote connector, which of its event types (its
// `events/list`, read when the connector is picked), that type's arguments (from its `inputSchema`: a boolean is Any,
// Yes or No, so it can leave the argument out), and how events come (webhook, push or poll; automatic takes the first
// the server offers).
import { createResource, For, Match, Show, Switch, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { AutoNote, AutoRow, Input, Select } from '@wisp/ui';
import { connectorEvents, connectorsList, type McpConnector } from './commands.ts';
import { t } from './i18n.ts';
import { argFields, DELIVERIES, type ArgField, type TriggerFormState } from './trigger-form.ts';
import './i18n/lazy.ts';

const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;

interface Props {
  readonly f: TriggerFormState;
  readonly set: (patch: Partial<TriggerFormState>) => void;
}

/** The connectors wisp calls itself: remote, switched on. */
const remote = (c: PluginClient): Promise<McpConnector[]> =>
  c
    .run(connectorsList, { peer: c.home })
    .then((v) => v.connectors.filter((x) => x.transport === 'remote' && x.url && x.enabled))
    .catch(() => []);

export function McpEventFields(props: Props & { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const [servers] = createResource(() => remote(c));
  const [events] = createResource(
    () => props.f.connector || undefined,
    (id) => c.run(connectorEvents, { peer: c.home, id }).catch(() => ({ served: false, events: [] })),
  );
  const picked = () => events()?.events.find((e) => e.name === props.f.event);
  return (
    <>
      <AutoRow label={say('automations.form.mcp.server')}>
        <Select
          aria-label={say('automations.form.mcp.server')}
          value={props.f.connector}
          onChange={(v) => props.set({ connector: v, event: '', args: {} })}
          placeholder={say(servers.loading ? 'automations.form.mcp.reading' : 'automations.form.mcp.pickServer')}
          items={(servers() ?? []).map((s) => ({ value: s.id, label: s.name }))}
        />
      </AutoRow>
      <Show when={!servers.loading && (servers() ?? []).length === 0}>
        <AutoNote tone="muted">{say('automations.form.mcp.noServers')}</AutoNote>
      </Show>
      <Show when={props.f.connector}>
        <Switch>
          <Match when={events.loading}>
            <AutoNote tone="muted">{say('automations.form.mcp.reading')}</AutoNote>
          </Match>
          <Match when={events() && !events()?.served}>
            <AutoNote tone="warn">{say('automations.form.mcp.noEvents')}</AutoNote>
          </Match>
          <Match when={events()?.served}>
            <AutoRow label={say('automations.form.mcp.event')}>
              <Select
                aria-label={say('automations.form.mcp.event')}
                value={props.f.event}
                onChange={(v) => props.set({ event: v, args: {} })}
                placeholder={say('automations.form.mcp.pickEvent')}
                items={(events()?.events ?? []).map((e) => ({ value: e.name, label: e.name }))}
              />
            </AutoRow>
          </Match>
        </Switch>
      </Show>
      <For each={argFields(picked()?.inputSchema)}>{(a) => <ArgRow f={props.f} set={props.set} arg={a} />}</For>
      <Show when={props.f.event}>
        <AutoRow label={say('automations.form.mcp.delivery')}>
          <Select
            aria-label={say('automations.form.mcp.delivery')}
            value={props.f.delivery}
            onChange={(v) => props.set({ delivery: v })}
            items={DELIVERIES.map((d) => ({ value: d, label: say(`automations.form.mcp.delivery.${d || 'auto'}`) }))}
          />
        </AutoRow>
      </Show>
    </>
  );
}

/** One argument of the picked event type: a pick (an enum, or Any/Yes/No for a boolean), else a box. */
function ArgRow(props: Props & { readonly arg: ArgField }): JSX.Element {
  const a = props.arg;
  const value = (): string => props.f.args[a.key] ?? '';
  const setArg = (v: string): void => props.set({ args: { ...props.f.args, [a.key]: v } });
  const any = { value: '', label: say('automations.form.mcp.any') };
  const options = (): { value: string; label: string }[] =>
    a.kind === 'boolean'
      ? [any, { value: 'true', label: say('automations.form.mcp.yes') }, { value: 'false', label: say('automations.form.mcp.no') }]
      : [any, ...a.options.map((o) => ({ value: o, label: o }))];
  return (
    <AutoRow label={a.key}>
      <Show
        when={a.kind !== 'text'}
        fallback={<Input value={value()} onInput={setArg} spellcheck={false} placeholder={a.hint || say('automations.form.mcp.optional')} aria-label={a.key} />}
      >
        <Select aria-label={a.key} value={value()} onChange={setArg} items={options()} />
      </Show>
    </AutoRow>
  );
}
