// A submenu's filter (owner-list #63: Goal… and Recipes grow with the formula): a box over its rows that narrows them
// as you type, on each row's label and second line (./menu-entries.ts filteredItems). Its keys: ↓ goes to the rows,
// Enter picks the first match, Escape is the menu's (one level back); every other key is the box's own, never the
// menu's typeahead. Loaded with the menus' machines (desktop, the drill-in pane, the phone's sheet), apart from the
// first page.
import { type JSX } from 'solid-js';
import { sub, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { MenuFilter } from './menu-entries.ts';
import '../css/menu-filter.css';

export interface MenuFilterBoxProps {
  readonly filter: MenuFilter;
  readonly value: string;
  readonly onInput: (value: string) => void;
  /** ↓: to the first row */
  readonly onDown: () => void;
  /** Enter: the first match */
  readonly onEnter: () => void;
  readonly part: PartName;
  /** the list it narrows (its id), for a screen reader */
  readonly controls?: string;
  readonly ref?: (el: HTMLInputElement) => void;
}

export function MenuFilterBox(props: MenuFilterBoxProps): JSX.Element {
  const keys = (e: KeyboardEvent): void => {
    if (e.isComposing) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      props.onDown();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      props.onEnter();
    } else if (e.key !== 'Escape' && e.key !== 'Tab') {
      // the box's own (a letter, ←, Home): never the menu's typeahead or its ← back
      e.stopPropagation();
    }
  };
  return (
    <div data-kit="menu-filter" data-part={sub(props.part, 'filter')} role="none">
      <Icon name="search" size="sm" tone="faint" />
      <input
        ref={(el) => props.ref?.(el)}
        type="search"
        value={props.value}
        placeholder={props.filter.placeholder ?? props.filter.label}
        aria-label={props.filter.label}
        aria-controls={props.controls}
        autocomplete="off"
        spellcheck={false}
        enterkeyhint="go"
        onInput={(e) => props.onInput(e.currentTarget.value)}
        onKeyDown={keys}
        // a press in the box is the box's: the menu neither closes nor takes the focus back
        onPointerDown={(e) => e.stopPropagation()}
      />
    </div>
  );
}
