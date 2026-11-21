// The browser scenes of the rows and surfaces that grew a phone look (ADR-0065 build step 4): ListRow, Swipe,
// Section, SettingsGroup, SettingsRow, Card, Banner, Pill, Skeleton, EmptyState and VirtualList, rendered with no
// data-platform. test/goldens/platform-browser-rows.json holds their HTML from before the phone variants landed;
// platform-rows.test.tsx renders them again (with the phone-only props passed too) and expects the same bytes, so
// the browser's look (v1 parity, ADR-0016) can't move.
import type { JSX } from 'solid-js';
import { Avatar, Banner, Button, Card, EmptyState, ListRow, Pill, Section, SettingsGroup, SettingsRow, Skeleton, Swipe, Text, VirtualList } from '../src/index.ts';

const noop = (): void => {};
const ITEMS = ['a', 'b', 'c'];

/** Each scene's view; `extra` adds what a phone-aware caller passes too (the browser must ignore it). */
export function rowScenes(extra: (kind: string) => Record<string, unknown> = () => ({})): ReadonlyArray<readonly [string, () => JSX.Element]> {
  return [
    ['row-one-line', () => <ListRow title="Studio" onOpen={noop} {...extra('row')} />],
    ['row-two-line', () => <ListRow title="Fix the flaky reconnect test" sub={<Text tone="muted">Studio · wisp</Text>} lead={<Avatar name="Claude" />} time={<Text>2m</Text>} pills={<Pill tone="good" dot>Done</Pill>} onOpen={noop} {...extra('row')} />],
    ['row-three-line', () => <ListRow title="Port the swipe band" sub={<Text>Running</Text>} foot={<Text>Editing swipe.ts</Text>} lead={<Avatar name="Codex" />} href="/task/1" current {...extra('row')} />],
    ['row-feed', () => <ListRow layout="feed" title="Bench the outbox" sub={<Text>Studio</Text>} time={<Text>2m</Text>} pills={<Pill size="sm" tone="warn" dot>Waiting</Pill>} strong onOpen={noop} {...extra('row')} />],
    ['swipe', () => <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={noop}><ListRow title="Row" /></Swipe>],
    ['swipe-one-side', () => <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} onSwipe={noop} disabled><ListRow title="Row" /></Swipe>],
    ['section', () => <Section title="Recent" description="Last seven days" actions={<Button size="sm">All</Button>}><Text>body</Text></Section>],
    ['section-plain', () => <Section title="Need you" level={2}><Text>body</Text></Section>],
    ['settings-group', () => <SettingsGroup title="Notifications" description="What reaches this phone." actions={<Button size="sm">Test</Button>}><SettingsRow title="Finished" description="When a task ends." control={<Button size="sm">On</Button>} /></SettingsGroup>],
    ['settings-group-bare', () => <SettingsGroup><SettingsRow title="Stay connected" /></SettingsGroup>],
    ['card', () => <Card header={<Text>Studio</Text>} footer={<Text>Online</Text>} {...extra('card')}><Text>body</Text></Card>],
    ['card-button', () => <Card onClick={noop} tone="accent" {...extra('card')}><Text>open</Text></Card>],
    ['banner', () => <Banner title="Offline" action={<Button size="sm">Retry</Button>} onDismiss={noop}>Last updated 12:04</Banner>],
    ['banner-bad', () => <Banner tone="bad">Couldn't reach Studio</Banner>],
    ['pills', () => <span><Pill tone="good" dot live>Running</Pill><Pill variant="outline">wisp</Pill><Pill variant="outline" tone="accent">tag</Pill><Pill variant="plain" icon="check" tone="good">Signed in</Pill><Pill size="sm" tone="warn">Waiting</Pill></span>],
    ['skeletons', () => <span><Skeleton /><Skeleton shape="block" width="1/2" /><Skeleton shape="circle" /><Skeleton shape="pill" /></span>],
    ['empty', () => <EmptyState title="No tasks yet" icon="feed" action={<Button variant="primary">New task</Button>}>Start one from here.</EmptyState>],
    ['empty-compact', () => <EmptyState title="Nothing here" compact />],
    ['virtual-list', () => <VirtualList aria-label="Tasks" items={ITEMS} key={(s) => s} viewport={400} {...extra('virtual')}>{(s) => <Text>{s()}</Text>}</VirtualList>],
  ];
}
