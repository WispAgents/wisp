// A crew story's reply box (the task page's ReplyBox, compact): its own chunk, loaded when a story opens. Imported with
// the stories row it brought the task page's kit chunk, its popovers and their stylesheets to every phone's first screen
// (perf r1 #1/#2).
import type { JSX } from 'solid-js';
import { ReplyBox, type CompactReplyBoxProps } from '@wisp/ui';

export default function StoryReply(props: CompactReplyBoxProps): JSX.Element {
  return <ReplyBox {...props} />;
}
