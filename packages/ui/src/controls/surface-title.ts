// The title of the dialog or sheet a piece is drawn in (QA r7 #6): a panel pushed as a sheet of its own name (the
// phone's Delegate) doesn't say that name again under the sheet's head. Dialog.tsx provides it; nothing else does, so
// a panel in a popover or on a page keeps its title.
import { createContext, useContext, type Accessor } from 'solid-js';

export const SurfaceTitle = createContext<Accessor<string> | undefined>(undefined);

/** whether `title` is the one the surrounding dialog or sheet already shows */
export function repeatsSurfaceTitle(title: () => string): Accessor<boolean> {
  const surface = useContext(SurfaceTitle);
  return () => surface !== undefined && surface().trim() === title().trim();
}
