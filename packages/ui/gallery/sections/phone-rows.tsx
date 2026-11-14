// The phone apps' rows and surfaces (ADR-0065 build step 4, design/mobile.md §A.2, §A.4, §A.7): switch the
// platform (?platform=android|ios) to see Material 3 or HIG. Swipe a row with a finger (or touch emulation):
// Android's band warms and fills past a third; iOS reveals Mail's action, rests open on a short pull and runs
// it past half the row. A running row only gives a little.
import { createSignal, For, type JSX } from 'solid-js';
import { Avatar, Banner, Box, Button, Card, EmptyState, ListRow, Pill, PullRefresh, Section, SettingsGroup, SettingsRow, Skeleton, Stack, Swipe, Switch, Text, Wrap, usePlatform } from '../../src/index.ts';
import { Area, Specimen, State } from '../frame.tsx';

const TASKS = [
  { id: 't1', title: 'Fix the flaky reconnect test in the sync outbox', sub: 'Studio · wisp', time: '2m', pill: () => <Pill size="sm" tone="warn" dot>Waiting</Pill>, harness: 'Claude', strong: true },
  { id: 't2', title: 'Port the swipe band', sub: 'Editing swipe.ts', time: '5m', pill: () => <Pill size="sm" tone="accent" dot live>Running</Pill>, harness: 'Codex', running: true },
  { id: 't3', title: 'Bench the outbox', sub: 'Laptop · wisp', time: '1h', pill: () => <Pill size="sm" tone="good" dot>Done</Pill>, harness: 'Claude' },
];

function Phone(props: { readonly children: JSX.Element }): JSX.Element {
  // a phone's width, so the rows look as they will in the app
  return (
    <Box part="gallery.phone-box" surface="bg" border="line" radius="lg">
      {props.children}
    </Box>
  );
}

export function PhoneRowsGallery(): JSX.Element {
  const look = usePlatform();
  const [log, setLog] = createSignal('Swipe a row');
  const [pull, setPull] = createSignal(0.6);
  return (
    <Area id="phone-rows" title="Phone apps: rows and surfaces">
      {look() === 'web' ? (
        <Text tone="muted" part="gallery.phone-only">The browser keeps v1's rows: switch Platform to Android or iPhone (?platform=android|ios).</Text>
      ) : (
        <>
          <Specimen title="List rows" note="M3 one/two/three-line items (56/72/88 dp, leading 40 dp, trailing time over the pill) / iOS cells (44 pt min, inset separators, a chevron only where the row pushes a page).">
            <State label="rows">
              <Phone>
                <Stack gap="0" part="gallery.rows">
                  <ListRow title="Notifications" push onOpen={() => {}} />
                  <ListRow title="Keys on this phone" sub={<Text>2 keys</Text>} push onOpen={() => {}} />
                  <For each={TASKS}>
                    {(t) => <ListRow title={t.title} sub={<Text>{t.sub}</Text>} lead={<Avatar name={t.harness} />} time={<Text>{t.time}</Text>} pills={t.pill()} strong={t.strong === true} onOpen={() => {}} />}
                  </For>
                  <ListRow title="Studio" sub={<Text>Online</Text>} foot={<Text>wisp 2.0 · 3 tasks running</Text>} lead={<Avatar name="Studio" />} onOpen={() => {}} />
                </Stack>
              </Phone>
            </State>
          </Specimen>
          <Specimen title="Swipe" note="Right: Mark done; left: Save for later. The running row (second) has no right action: it only gives a little, with no label.">
            <State label={log()}>
              <Phone>
                <Stack gap="0" part="gallery.swipes">
                  <For each={TASKS}>
                    {(t) => (
                      <Swipe
                        right={t.running ? undefined : { label: 'Mark done', icon: 'done', tone: 'good' }}
                        left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }}
                        onSwipe={(d) => setLog(`${d === 'right' ? 'Mark done' : 'Save for later'}: ${t.title}`)}
                        onArm={(a) => a && setLog('tick')}
                        part="gallery.swipe"
                      >
                        <ListRow title={t.title} sub={<Text>{t.sub}</Text>} lead={<Avatar name={t.harness} />} time={<Text>{t.time}</Text>} pills={t.pill()} onOpen={() => {}} />
                      </Swipe>
                    )}
                  </For>
                </Stack>
              </Phone>
            </State>
          </Specimen>
          <Specimen title="Pull to refresh" note="M3 indicator / iOS refresh control, as the pull comes down and while every computer reconnects.">
            <Wrap gap="4">
              <Button size="sm" variant="secondary" onClick={() => setPull((p) => (p >= 1 ? 0.2 : Math.round((p + 0.2) * 10) / 10))} part="gallery.pull-step">{`Pull ${Math.round(pull() * 100)}%`}</Button>
            </Wrap>
            <Stack gap="4">
              <Text size="xs" tone="faint">pulling · refreshing</Text>
              <Phone>
                <Stack gap="0" part="gallery.pull">
                  <PullRefresh progress={pull()} refreshing={false} label="Reconnecting" />
                  <Skeleton shape="block" width="full" />
                  <ListRow title="Bench the outbox" sub={<Text>Laptop · wisp</Text>} />
                </Stack>
              </Phone>
              <Phone>
                <Stack gap="0" part="gallery.pull-on">
                  <PullRefresh progress={1} refreshing label="Reconnecting" />
                  <Skeleton shape="block" width="full" />
                  <ListRow title="Bench the outbox" sub={<Text>Laptop · wisp</Text>} />
                </Stack>
              </Phone>
            </Stack>
          </Specimen>
          <Specimen title="Section and settings group" note="M3 subheader in primary, no card / iOS inset grouped with the help text as the footer.">
            <Phone>
              <Stack gap="6" part="gallery.groups" padY="6">
                <SettingsGroup title="Notifications" description="What reaches this phone when the app is closed.">
                  <SettingsRow title="Finished" description="When a task ends." control={<Switch checked label="Finished" onChange={() => {}} />} />
                  <SettingsRow title="Asks" description="When a task needs an answer." control={<Switch checked={false} label="Asks" onChange={() => {}} />} />
                </SettingsGroup>
                <Section title="Recent" description="The last seven days.">
                  <ListRow title="Bench the outbox" push onOpen={() => {}} />
                  <ListRow title="Port the swipe band" push onOpen={() => {}} />
                </Section>
              </Stack>
            </Phone>
          </Specimen>
          <Specimen title="Cards" note="M3 outlined (default) and filled, 12 dp / iOS grouped, 12 pt continuous corners.">
            <Phone>
              <Stack gap="6" pad="7" part="gallery.cards">
                <Card header={<Text>wisp Cloud · you</Text>} footer={<Text>2 computers</Text>}>
                  <Text>Studio · Online</Text>
                </Card>
                <Card variant="filled" onClick={() => {}}>
                  <Text>Plan: release notes</Text>
                </Card>
              </Stack>
            </Phone>
          </Specimen>
          <Specimen title="Banner, chips, skeleton, empty state">
            <Phone>
              <Stack gap="6" padY="6" part="gallery.surfaces">
                <Banner title="Offline" icon="cloud" action={<Button size="sm">Retry</Button>}>Last updated 12:04</Banner>
                <Banner tone="bad">Couldn't reach Studio</Banner>
                <Box padX="7">
                  <Wrap gap="4">
                    <Pill tone="good" dot>Done</Pill>
                    <Pill tone="warn" dot>Waiting</Pill>
                    <Pill variant="outline">wisp</Pill>
                    <Pill variant="outline" tone="accent" icon="check">Studio</Pill>
                    <Pill size="sm" tone="accent">Running</Pill>
                  </Wrap>
                </Box>
                <Stack gap="4" padX="7">
                  <Skeleton width="3/4" />
                  <Skeleton width="1/2" />
                  <Skeleton shape="block" />
                </Stack>
                <EmptyState title="No tasks yet" icon="feed" action={<Button variant="primary" onClick={() => {}}>New task</Button>}>
                  Start one here or from your harness.
                </EmptyState>
              </Stack>
            </Phone>
          </Specimen>
        </>
      )}
    </Area>
  );
}
