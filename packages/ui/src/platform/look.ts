// The phone apps' look (ADR-0065 §5): the shell writes `data-platform="android" | "ios"` on <html> from
// `platform.kind`; the browser at any width has none ('web': v1's look, ADR-0016). Only the kit reads it
// (tools/check rule `platform-branch`): CSS through `:root[data-platform=…]` in @layer kit, structure
// (a FAB on Android, a compose button in the iOS navigation bar) through `usePlatform()`. Pure.

export type Look = 'android' | 'ios' | 'web';

/** The attribute on <html> that carries the look. */
export const LOOK_ATTR = 'data-platform';

/** The look an attribute value names: anything but 'android' or 'ios' is the browser's. */
export function lookOf(attr: string | null | undefined): Look {
  return attr === 'android' || attr === 'ios' ? attr : 'web';
}

/** The look of a document as it is now. */
export function documentLook(doc: Pick<Document, 'documentElement'> | undefined): Look {
  return lookOf(doc?.documentElement.getAttribute(LOOK_ATTR));
}
