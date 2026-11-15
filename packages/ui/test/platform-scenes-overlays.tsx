// The browser scenes of the overlays that grew a phone look (ADR-0065 build step 5): Dialog, Sheet, Menu,
// ContextMenu, Popover, Toast and Tooltip, rendered with no data-platform, open where they open. Their HTML (the page's,
// portals included) from before the phone variants landed is test/goldens/platform-browser-overlays.json;
// platform-overlays.test.tsx renders them again (with the phone-only props passed too) and expects the same bytes, so
// the browser's look (v1 parity, ADR-0016) can't move.
import type { JSX } from 'solid-js';
import { Button, ContextMenu, Dialog, Menu, Popover, Sheet, Text, Toasts, Tooltip, createToaster, type MenuEntry } from '../src/index.ts';

const noop = (): void => {};
export const MENU_ITEMS: readonly MenuEntry[] = [
  { value: 'open', label: 'Open' },
  { value: 'done', label: 'Mark done', icon: 'done' },
  { kind: 'separator' },
  { value: 'delete', label: 'Delete task', icon: 'trash', danger: true },
];

export interface OverlayScene {
  readonly name: string;
  readonly view: () => JSX.Element;
  /** what opens it after it has drawn (a right-click, a toast) */
  readonly act?: () => void;
}

/** Each scene; `extra` adds what a phone-aware caller passes too (the browser must ignore it). */
export function overlayScenes(extra: (kind: string) => Record<string, unknown> = () => ({})): readonly OverlayScene[] {
  const toaster = createToaster();
  const foot = (): JSX.Element => (
    <>
      <Button>Cancel</Button>
      <Button variant="danger">Delete</Button>
    </>
  );
  return [
    { name: 'dialog', view: () => <Dialog open title="Delete this task?" description="Its worktree goes too." footer={foot()} onOpenChange={noop} {...extra('dialog')}><Text>body</Text></Dialog> },
    { name: 'dialog-alert', view: () => <Dialog open role="alertdialog" size="sm" title="Stop the run?" footer={foot()} onOpenChange={noop} {...extra('dialog')} /> },
    { name: 'sheet', view: () => <Sheet open title="New task" footer={foot()} onOpenChange={noop} {...extra('sheet')}><Text>body</Text></Sheet> },
    { name: 'menu-closed', view: () => <Menu items={MENU_ITEMS} onSelect={noop} trigger={{ icon: 'dots', 'aria-label': 'More' }} {...extra('menu')} /> },
    { name: 'menu-open', view: () => <Menu open items={MENU_ITEMS} onSelect={noop} trigger={{ label: 'Actions' }} {...extra('menu')} /> },
    {
      name: 'context-menu',
      view: () => (
        <ContextMenu items={MENU_ITEMS} onSelect={noop} {...extra('context')}>
          <Text>row</Text>
        </ContextMenu>
      ),
      act: () => document.querySelector("[data-kit='context-area'] [data-kit='text']")?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 10, clientY: 10 })),
    },
    { name: 'popover-closed', view: () => <Popover trigger={{ label: 'Filter' }} title="Filter" {...extra('popover')}><Text>body</Text></Popover> },
    { name: 'popover-open', view: () => <Popover open trigger={{ icon: 'search', 'aria-label': 'Filter' }} title="Filter" {...extra('popover')}><Text>body</Text></Popover> },
    { name: 'tooltip', view: () => <Tooltip content="Mark done" hint="D"><Button>Done</Button></Tooltip> },
    { name: 'toast', view: () => <Toasts toaster={toaster} />, act: () => void toaster.show('Archived', { action: { label: 'Undo', onClick: noop } }) },
  ];
}
