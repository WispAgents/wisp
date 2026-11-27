// Answer blocks: the wisp presentation's renderers (v1 web/js/blocks.js), as kit components.
export { Answer, asBlock, rendererFor, type AnswerProps } from './Answer.tsx';
export { BlockHost, useBlocks, useBlockT } from './context.tsx';
export { BLOCKS, BLOCK_TYPES } from './registry.ts';
export { BLOCK_STRINGS, blockEnglish, type BlockString, type BlockTranslate } from './strings.ts';
export { AskOptions, AskPanel, type AskOption } from './AskOptions.tsx';
export { splitQuestion } from './ask-split.ts';
export { UnknownBlock } from './Unknown.tsx';
export { MediaViewer, type ViewItem } from './Viewer.tsx';
export { copyImage } from './media-copy.ts';
export { docStats } from './rich-doc.ts';
/** Untrusted-field readers for a plugin's own block renderer: `blockRead.str(b.title)`. */
export * as blockRead from './read.ts';
export type { AnswerBlock, AnswerCommentAt, AnswerCommentMark, AnswerComments, BlockContext, BlockedItem, BlockProps, CapturedChange, BlockRenderer, DocText, EnvReveal, KeepHost, KeepWhat, RunResult, StuffRef, TaskRef } from './types.ts';
