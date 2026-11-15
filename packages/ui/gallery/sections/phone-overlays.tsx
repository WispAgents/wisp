// The phone apps' overlays (ADR-0065 build step 5, design/mobile.md §A.4, §A.7): switch the platform
// (?platform=android|ios). Long-press a row (a finger, or touch emulation) or press "Long-press the first row": Android
// opens the M3 bottom sheet of its actions with Select first, iOS the context menu with the row as its preview. The
// confirm is an M3 basic dialog / an iOS alert; the form is an M3 dialog / an iOS sheet; the sheet drags down to close
// and on iOS rests at medium or large. Toasts are the M3 snackbar / the iOS capsule, above the bars.
import { createSignal, For, type JSX } from 'solid-js';
import { Box, Button, ContextMenu, createToaster, Dialog, IconButton, ListRow, Menu, Popover, Sheet, Stack, Swipe, Text, Toasts, Tooltip, Wrap, usePlatform, type MenuEntry } from '../../src/index.ts';
import { Area, Specimen, State } from '../frame.tsx';

const ROW_MENU: readonly MenuEntry[] = [
  { value: 'open', label: 'Open', icon: 'external' },
  { value: 'rename', label: 'Rename…', icon: 'pen' },
  { value: 'later', label: 'Save for later', icon: 'bookmark' },
  { value: 'done', label: 'Mark done', icon: 'done' },
  { kind: 'separator' },
  { value: 'archive', label: 'Archive', icon: 'archive' },
  { value: 'delete', label: 'Delete task', icon: 'trash', danger: true },
];
const TITLES = ['Fix the flaky reconnect test', 'Port the swipe band', 'Bench the outbox'];

/** a long-press, as Swipe's onLongPress does in TaskRow (env.menuAt): the list menu's contextmenu on the row */
const longPress = (row: HTMLElement): void => {
  const r = row.getBoundingClientRect();
  row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left + 24, clientY: r.bottom }));
};

export function PhoneOverlaysGallery(): JSX.Element {
  const look = usePlatform();
  const [log, setLog] = createSignal('Long-press a row');
  const [confirm, setConfirm] = createSignal(false);
  const [form, setForm] = createSignal(false);
  const [sheet, setSheet] = createSignal(false);
  const toaster = createToaster();
  let first: HTMLElement | undefined;
  return (
    <Area id="phone-overlays" title="Phone apps: overlays">
      {look() === 'web' ? (
        <Text tone="muted" part="gallery.phone-only">The browser keeps v1's overlays: switch Platform to Android or iPhone (?platform=android|ios).</Text>
      ) : (
        <>
          <Specimen title="Long-press menu" note="Android: the M3 modal bottom sheet of the row's actions, Select first, the row's title as its headline. iOS: the context menu, the row lifted as its preview over a blurred page.">
            <State label={log()}>
              <Stack gap="4">
                <Button variant="secondary" size="sm" onClick={() => first && longPress(first)} part="gallery.long-press">
                  Long-press the first row
                </Button>
                <Box part="gallery.phone-box" surface="bg" border="line" radius="lg">
                  <ContextMenu items={ROW_MENU} onSelect={(v) => setLog(`Chose ${v}`)} onSelectMode={() => setLog('Selection mode')} part="gallery.row-menu">
                    <Stack gap="0">
                      <For each={TITLES}>
                        {(title, i) => (
                          <Swipe onSwipe={() => {}} onLongPress={longPress} part="gallery.overlay-row">
                            <div ref={(el) => i() === 0 && (first = el)}>
                              <ListRow title={title} sub={<Text>Studio · wisp</Text>} onOpen={() => {}} />
                            </div>
                          </Swipe>
                        )}
                      </For>
                    </Stack>
                  </ContextMenu>
                </Box>
              </Stack>
            </State>
          </Specimen>
          <Specimen title="Menu from a button" note="Android: the M3 menu (4 dp corners, 48 dp items). iOS: the pull-down menu (13 pt corners, the material, icons trailing).">
            <State label="menu">
              <Menu items={ROW_MENU} onSelect={(v) => setLog(`Chose ${v}`)} trigger={{ icon: 'dots-v', 'aria-label': 'More' }} part="gallery.phone-menu" />
            </State>
          </Specimen>
          <Specimen title="Dialogs" note="A confirm: the M3 basic dialog (28 dp, end-aligned text buttons) / the iOS alert (270 pt, Delete red, Cancel bold). A form: an M3 dialog / an iOS sheet.">
            <State label="confirm and form">
              <Wrap gap="4">
                <Button variant="secondary" onClick={() => setConfirm(true)} part="gallery.open-confirm">
                  Delete…
                </Button>
                <Button variant="secondary" onClick={() => setForm(true)} part="gallery.open-form">
                  New job…
                </Button>
              </Wrap>
              <Dialog
                open={confirm()}
                onOpenChange={setConfirm}
                role="alertdialog"
                title="Delete this task?"
                description="Its worktree and transcript go too."
                footer={
                  <>
                    <Button onClick={() => setConfirm(false)}>Cancel</Button>
                    <Button variant="danger" onClick={() => setConfirm(false)}>
                      Delete
                    </Button>
                  </>
                }
                part="gallery.confirm"
              />
              <Dialog open={form()} onOpenChange={setForm} title="New job" footer={<Button variant="primary" onClick={() => setForm(false)}>Create</Button>} part="gallery.form">
                <Text>Name, schedule and prompt go here.</Text>
              </Dialog>
            </State>
          </Specimen>
          <Specimen title="Sheet" note="Drag it down to close (a draft stays); × is the twin. iOS: medium, then large on a drag up or a tap on the grabber.">
            <State label="sheet">
              <Button variant="secondary" onClick={() => setSheet(true)} part="gallery.open-sheet">
                Open the sheet
              </Button>
              <Sheet open={sheet()} onOpenChange={setSheet} title="New task" footer={<Button variant="primary" onClick={() => setSheet(false)}>Start</Button>} part="gallery.sheet">
                <Text>What should the agent do?</Text>
              </Sheet>
            </State>
          </Specimen>
          <Specimen title="Popover" note="A bottom sheet on a phone; a popover on an iPad.">
            <State label="popover">
              <Popover trigger={{ label: 'Filter' }} title="Filter" part="gallery.phone-popover">
                <Text>Running · Waiting · Done</Text>
              </Popover>
            </State>
          </Specimen>
          <Specimen title="Toast and tooltip" note="Android: the M3 snackbar with Undo above the FAB and the navigation bar; a long-press on an icon button shows its plain tooltip. iOS: the capsule above the tab bar; no tooltip.">
            <State label="toast">
              <Wrap gap="4">
                <Button variant="secondary" onClick={() => toaster.show('Archived', { action: { label: 'Undo', onClick: () => setLog('Undone') } })} part="gallery.show-toast">
                  Archive (toast)
                </Button>
                <Tooltip content="Mark done">
                  <IconButton icon="done" label="Mark done" onClick={() => setLog('Marked done')} />
                </Tooltip>
              </Wrap>
              <Toasts toaster={toaster} part="gallery.phone-toast" />
            </State>
          </Specimen>
        </>
      )}
    </Area>
  );
}
