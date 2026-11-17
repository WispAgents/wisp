// The Thread from the task's menu (`wisp.threads.thread`, an overlay: the shell draws it as a sheet on a phone,
// M3 or iOS as the platform's kit says, and as a dialog in the browser).
import type { JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { Thread } from './Thread.tsx';

export default function ThreadSheet(props: { readonly client: PluginClient; readonly params: { readonly task: string } }): JSX.Element {
  return <Thread client={props.client} task={props.params.task} />;
}
