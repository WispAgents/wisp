// The Thread tab beside a task (ShellApi.sideTabs, after Changes · PR · Shell · Preview): the task's Thread, edge to
// edge in the panel's body.
import type { JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { Thread } from './Thread.tsx';

export default function ThreadTab(props: { readonly client: PluginClient; readonly task: string; readonly value: string }): JSX.Element {
  return <Thread client={props.client} task={props.task} />;
}
