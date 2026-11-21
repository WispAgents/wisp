// The phone apps' controls (ADR-0065 build step 6, design/mobile.md §A.5–§A.7): switch the platform
// (?platform=android|ios) to see Material 3 or HIG; the browser draws the same specimens in v1's look, unchanged.
// Android: filled / tonal / text / outlined-chip buttons with the ripple, standard / filled / tonal icon buttons, the
// M3 switch with its handle icon, primary and secondary tabs, outlined text fields with the label in the notch, the
// exposed dropdown (a long list opens a bottom sheet of radios). iOS: filled / tinted / gray / plain buttons, plain bar
// buttons, UISwitch, segmented controls, rounded fields, the pull-down menu (a long list opens a checkmark sheet).
import { createSignal, type JSX } from 'solid-js';
import { Box, Button, Field, IconButton, Input, Select, Stack, Switch, Tabs, TextArea, Wrap, type SelectOption } from '../../src/index.ts';
import { Area, Specimen, State } from '../frame.tsx';

const HARNESSES: readonly SelectOption[] = [
  { value: 'claude', label: 'Claude Code' },
  { value: 'codex', label: 'Codex' },
  { value: 'opencode', label: 'opencode', disabled: true },
];
const COMPUTERS: readonly SelectOption[] = ['Studio', 'Laptop', 'Cloud', 'Mini', 'Pi', 'Desk', 'Build box', 'Grid'].map((label) => ({ value: label.toLowerCase(), label }));
const KINDS = [
  { value: 'all', label: 'All', count: 12 },
  { value: 'docs', label: 'Docs' },
  { value: 'media', label: 'Media' },
  { value: 'apps', label: 'Apps' },
];

function Phone(props: { readonly children: JSX.Element }): JSX.Element {
  return (
    <Box part="gallery.phone-box" surface="bg" border="line" radius="lg" pad="7">
      {props.children}
    </Box>
  );
}

export function PhoneControlsGallery(): JSX.Element {
  const [stay, setStay] = createSignal(true);
  const [push, setPush] = createSignal(false);
  const [harness, setHarness] = createSignal('codex');
  const [computer, setComputer] = createSignal('studio');
  const [busy, setBusy] = createSignal(false);
  const [picked, setPicked] = createSignal(false);
  return (
    <Area id="phone-controls" title="Phone apps: controls">
      <Specimen title="Buttons" note="M3: filled, tonal, text, filter chip; 32/40/56 dp, full radius, the ripple while held. HIG: filled, tinted, gray, plain; 28/34/50 pt, the highlight.">
        <State label="variants">
          <Phone>
            <Stack gap="6" part="gallery.phone-buttons">
              <Wrap gap="4">
                <Button variant="primary" icon="plus">New task</Button>
                <Button>Tonal</Button>
                <Button variant="accent">Accent</Button>
                <Button variant="ghost">Text</Button>
                <Button variant="danger">Delete</Button>
              </Wrap>
              <Wrap gap="4">
                <Button variant="chip" pressed={picked()} onClick={() => setPicked(!picked())}>Running</Button>
                <Button variant="ghost" tone="good" icon="done">Mark done</Button>
                <Button variant="link">Turn every plugin off</Button>
              </Wrap>
              <Wrap gap="4">
                <Button variant="primary" size="sm">Small</Button>
                <Button variant="primary" size="md">Medium</Button>
                <Button variant="primary" size="lg">Large</Button>
              </Wrap>
              <Wrap gap="4">
                <Button disabled>Disabled</Button>
                <Button variant="primary" disabled>Disabled</Button>
                <Button variant="primary" loading={busy()} onClick={() => setBusy(!busy())}>Send</Button>
              </Wrap>
              <Button variant="primary" size="lg" fill>Continue</Button>
            </Stack>
          </Phone>
        </State>
      </Specimen>
      <Specimen title="Icon buttons" note="M3 standard / filled / tonal, 40 dp in a 48 dp target; iOS plain bar buttons in the tint, 44 pt, filled and gray circles.">
        <State label="variants">
          <Phone>
            <Wrap gap="4" part="gallery.phone-icon-buttons">
              <IconButton icon="dots-v" label="More" />
              <IconButton icon="search" label="Search" />
              <IconButton icon="plus" label="New" variant="primary" />
              <IconButton icon="x" label="Close" variant="secondary" />
              <IconButton icon="trash" label="Delete" tone="bad" />
              <IconButton icon="share" label="Share" size="xs" />
              <IconButton icon="star" label="Star" disabled />
            </Wrap>
          </Phone>
        </State>
      </Specimen>
      <Specimen title="Switches" note="M3 52 × 32 with the check on the handle; UISwitch 51 × 31 in the accent.">
        <State label="on, off, disabled">
          <Phone>
            <Stack gap="6" part="gallery.phone-switches">
              <Switch checked={stay()} onChange={setStay} label="Stay connected" />
              <Switch checked={push()} onChange={setPush} label="Notifications" />
              <Switch checked disabled onChange={() => {}} label="Locked" />
            </Stack>
          </Phone>
        </State>
      </Specimen>
      <Specimen title="Tabs" note="Android: line = primary tabs, seg/pill = secondary tabs. iOS: every variant is a segmented control.">
        <State label="line, seg">
          <Phone>
            <Stack gap="7" part="gallery.phone-tabs">
              <Tabs items={KINDS} aria-label="Kinds" fill />
              <Tabs items={KINDS.slice(0, 3)} variant="seg" fill aria-label="Members" />
            </Stack>
          </Phone>
        </State>
      </Specimen>
      <Specimen title="Text fields and selects" note="M3 outlined fields (56 dp, the label in the notch, 2 dp primary on focus), the exposed dropdown; iOS rounded fields and the pull-down menu. A long list (Computer) opens a sheet on a phone.">
        <State label="fields">
          <Phone>
            <Stack gap="8" part="gallery.phone-fields">
              <Field label="Task name" hint="Shown in the list.">
                <Input value="Fix the flaky test" />
              </Field>
              <Field label="Search">
                <Input icon="search" type="search" placeholder="Tasks, files, people" />
              </Field>
              <Field label="Branch" error="That branch doesn't exist.">
                <Input value="feat/nope" mono />
              </Field>
              <Field label="Notes">
                <TextArea placeholder="Anything else the agent should know" minRows={2} />
              </Field>
              <Field label="Harness">
                <Select items={HARNESSES} value={harness()} onChange={setHarness} />
              </Field>
              <Field label="Computer">
                <Select items={COMPUTERS} value={computer()} onChange={setComputer} />
              </Field>
              <Input value="Disabled" disabled aria-label="Disabled" />
            </Stack>
          </Phone>
        </State>
      </Specimen>
    </Area>
  );
}
