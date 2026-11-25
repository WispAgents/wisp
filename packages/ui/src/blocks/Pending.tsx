// A block still streaming (a chat message's ```wisp fence whose JSON hasn't closed): a skeleton of fixed height in its
// place, named by the block's type once that has arrived, so the raw JSON never shows and nothing jumps when the
// block replaces it.
import type { JSX } from 'solid-js';
import { Skeleton } from '../content/Skeleton.tsx';
import { useBlockT } from './context.tsx';
import type { BlockProps } from './types.ts';

export function PendingBlock(props: BlockProps): JSX.Element {
  const t = useBlockT();
  const of = (): string | undefined => {
    const v = (props.block as { readonly of?: unknown }).of;
    return typeof v === 'string' && v ? v : undefined;
  };
  return <Skeleton part="block.pending" shape="block" height="home-top" label={of() ? t('block.pending.of', { type: of() ?? '' }) : t('block.pending')} />;
}
