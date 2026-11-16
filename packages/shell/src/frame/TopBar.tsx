// The top bar (design/layout.md §2): the brand, then one Toolbar of every extension on the top
// bar's inputs, in the inputs' order (v1's #topbar left to right). Each declares its icon, label and
// priority, so the bar folds label → icon → More as it narrows and never grows a second row.

import { createMemo, type JSX } from 'solid-js';
import { Inline, Toolbar, type IconName, type ToolbarItem } from '@wisp/ui';
import { INPUTS, type InputId, type ResolvedExtension } from '../contract.ts';
import { useShell } from '../context.tsx';
import { Area, ExtView, extensionProps, usePluginClient } from '../slot.tsx';
import { useSignal } from '@wisp/ui';

const BAR_INPUTS = (Object.keys(INPUTS) as InputId[]).filter((k) => k.startsWith('shell.topbar:') && k !== 'shell.topbar:brand');

function Item(props: { readonly ext: ResolvedExtension; readonly form: 'full' | 'icon' }): JSX.Element {
  return <ExtView ext={props.ext} form={props.form} />;
}

/** One bar item: the extension draws the form it's given; More runs its `select`. */
function useItem(ext: ResolvedExtension): ToolbarItem {
  const shell = useShell();
  const client = usePluginClient(ext.plugin);
  const bar = ext.def.bar;
  const label = shell.t(bar?.label ?? ext.id);
  const icon = (bar?.icon ?? 'dots') as IconName;
  const render = (form: 'full' | 'icon'): JSX.Element => <Item ext={ext} form={form} />;
  const select = (): void => bar?.select?.(extensionProps(ext, client, {}));
  return bar?.priority === 'pin' || bar === undefined
    ? { id: ext.id, label, icon, priority: 'pin', render, onSelect: select }
    : { id: ext.id, label, icon, priority: bar.priority, render, onSelect: select };
}

export function TopBar(): JSX.Element {
  const shell = useShell();
  const lists = BAR_INPUTS.map((input) => useSignal(shell.extensions(input)));
  const items = createMemo(() => lists.flatMap((l) => l()).map((ext) => useItem(ext)));
  return (
    <Inline gap="4" grow part="shell.topbar.row">
      <Area input="shell.topbar:brand" fallback={null} form="full" />
      <Toolbar aria-label={shell.t('shell.topbar')} items={items()} justify="end" part="shell.topbar.items" t={(k) => shell.t(`kit.${k}`)} />
    </Inline>
  );
}

/** The sidebar head's lead (split view): the extensions on the bar's inputs after the brand (the update pill, This
 *  computer's chip, Machines, …) as one Toolbar that fills what the brand leaves, folding label → icon → More by their
 *  priority, so a full bar never runs under Search and the avatar (B8; design/layout.md §2). */
export function HeadLead(props: { readonly inputs: readonly InputId[] }): JSX.Element {
  const shell = useShell();
  const lists = props.inputs.map((input) => useSignal(shell.extensions(input)));
  const items = createMemo(() => lists.flatMap((l) => l()).map((ext) => useItem(ext)));
  return <Toolbar aria-label={shell.t('shell.topbar')} items={items()} justify="start" gap="1" part="shell.topbar.lead" t={(k) => shell.t(`kit.${k}`)} />;
}
