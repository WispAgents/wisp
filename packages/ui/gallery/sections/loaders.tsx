// Every loader in the kit, side by side: they all draw the one Liquid (ADR-0140).
import type { JSX } from 'solid-js';
import { BtwList, Button, FileChip, IconButton, LauncherSetup, Liquid, Orb, Steps, Text, WorkingPanel } from '../../src/index.ts';
import { Area, Specimen, State } from '../frame.tsx';

const noop = (): void => {};

export function LoadersGallery(): JSX.Element {
  return (
    <Area id="loaders" title="Loaders">
      <Specimen title="Liquid" note="The one busy mark: working, waiting on you (amber), still (failed, idle).">
        <State label="working · waiting · still">
          <Liquid />
          <Liquid waiting />
          <Liquid still />
        </State>
      </Specimen>
      <Specimen title="Working">
        <State label="inline (chat) · panel · waiting">
          <WorkingPanel inline status="Claude Code is working" elapsed="9s" count="0 actions" />
          <WorkingPanel status="Claude Code is working" elapsed="1m 4s" count="12 actions" />
          <WorkingPanel waiting status="Claude Code needs you" elapsed="2m" />
          <Orb live label="Watching" />
        </State>
      </Specimen>
      <Specimen title="Pending controls">
        <State label="button · icon button · re-check">
          <Button loading>Sending</Button>
          <Button variant="secondary" size="sm" loading>Saving</Button>
          <IconButton icon="x" label="Close" loading />
          <LauncherSetup line="No coding agent on this computer" busy={false} recheck={{ label: 'Re-check', busy: true, onClick: noop }} install={{ label: 'Install opencode', onClick: noop }} />
        </State>
      </Specimen>
      <Specimen title="Progress">
        <State label="current step · upload · side question">
          <Steps items={[{ title: 'Build', status: 'done' }, { title: 'Test', status: 'current' }, { title: 'Ship', status: 'pending' }]} />
          <FileChip name="screenshot.png" status="uploading" size="1.2 MB" removeLabel="Remove" />
          <BtwList items={[{ id: 'b', question: 'Why is the CI red?', pending: true, tip: "Side question" }]} waiting="Answering on the side" dismiss={() => 'Cancel'} onDismiss={noop} />
        </State>
      </Specimen>
    </Area>
  );
}
