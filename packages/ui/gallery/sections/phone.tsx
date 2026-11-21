// The phone apps' frame and navigation (ADR-0065 build step 3, design/mobile.md §A.1, §A.7): switch the
// platform (?platform=android|ios) to see Material 3 or HIG. Each specimen is a contained Frame at a phone's
// height: scroll its list to shrink the FAB and fold the large title.
import { createSignal, For, type JSX } from 'solid-js';
import { Box, Button, Fab, Frame, ListRow, NavigationBar, Scroll, Stack, Text, TopAppBar, usePlatform, type NavigationItem, type ToolbarItem } from '../../src/index.ts';
import { Area, Specimen, State } from '../frame.tsx';

const DESTINATIONS: readonly NavigationItem[] = [
  { id: 'tasks', label: 'Tasks', icon: 'feed', iconSelected: 'feed-fill', badge: 3 },
  { id: 'crew', label: 'Crew', icon: 'crew', iconSelected: 'crew-fill' },
  { id: 'stuff', label: 'Stuff', icon: 'stuff', iconSelected: 'stuff-fill' },
  { id: 'computers', label: 'Computers', icon: 'laptop', iconSelected: 'laptop-fill' },
];

const TITLES = ['Fix the flaky reconnect test', 'Port the swipe band', 'Rename the rail token', 'Bench the outbox', 'Add Bonjour lookup', 'Draft the store listing', 'Pin host keys from the QR', 'Move tokens to the Keystore'];

function Nav(props: { readonly selected: string; readonly onSelect: (id: string) => void }): JSX.Element {
  return <NavigationBar aria-label="Destinations" items={DESTINATIONS} selected={props.selected} onSelect={props.onSelect} badgeLabel={(n) => `${n} need you`} part="gallery.nav" />;
}

function Rows(props: { readonly n: number; readonly choicesAt?: number }): JSX.Element {
  return (
    <Scroll grow aria-label="Tasks">
      <For each={Array.from({ length: props.n }, (_, i) => i)}>
        {(i) => (
          <Stack gap="2">
            <ListRow title={TITLES[i % TITLES.length] ?? ''} sub={<Text tone="muted">{`Studio · ${i + 2}m`}</Text>}  />
            {props.choicesAt === i ? (
              <Box padX="7" padY="2" part="gallery.choices">
                <Button size="sm" variant="primary">Yes</Button>
              </Box>
            ) : null}
          </Stack>
        )}
      </For>
    </Scroll>
  );
}

const ACTIONS: readonly ToolbarItem[] = [
  { id: 'done', label: 'Mark done', icon: 'done', priority: 3, onSelect: () => {} },
  { id: 'later', label: 'Save for later', icon: 'bookmark', priority: 2, onSelect: () => {} },
  { id: 'share', label: 'Share', icon: 'share', priority: 1, onSelect: () => {} },
];

export function PhoneGallery(): JSX.Element {
  const look = usePlatform();
  const [tab, setTab] = createSignal('tasks');
  const [tab2, setTab2] = createSignal('tasks');
  const [composed, setComposed] = createSignal(0);
  const [away, setAway] = createSignal(false);
  const title = (): string => DESTINATIONS.find((d) => d.id === tab())?.label ?? '';
  return (
    <Area id="phone" title="Phone apps: frame and navigation">
      {look() === 'web' ? (
        <Text tone="muted" part="gallery.phone-only">The browser has none of these: switch Platform to Android or iPhone (?platform=android|ios).</Text>
      ) : (
        <>
          <Specimen title="A destination's root" note="Top app bar (M3 small / iOS large title), the list, the FAB (iOS: the compose button at the bar's end), the navigation bar (rail from 600 dp on Android). Scroll the list.">
            <State label={`root, ${composed()} composed`}>
              <Box grow>
                <Frame
                  contained
                  layout="stack"
                  page="list"
                  top={<TopAppBar kind="root" title={title()} onTitle={() => {}} actions={[{ id: 'search', label: 'Search', icon: 'search', priority: 1, onSelect: () => {} }]} part="gallery.bar" />}
                  list={
                    <>
                      <Rows n={24} />
                      <Fab icon="plus" label="New task" onClick={() => setComposed((n) => n + 1)} part="gallery.fab" />
                    </>
                  }
                  main={null}
                  nav={<Nav selected={tab()} onSelect={setTab} />}
                />
              </Box>
            </State>
          </Specimen>
          <Specimen title="A pushed page" note="M3 small top app bar (the one-line title beside Back) / iOS inline title with Back carrying the previous title. Actions fold label → icon → More (⋮ on Android, ⋯ on iOS).">
            <State label="page">
              <Box grow>
                <Frame
                  contained
                  layout="stack"
                  page="main"
                  top={<TopAppBar kind="page" title="Fix the flaky reconnect test in the sync outbox" back={{ label: 'Tasks', onBack: () => {} }} actions={ACTIONS} part="gallery.page-bar" />}
                  list={null}
                  main={<Rows n={20} />}
                  nav={<Nav selected={tab2()} onSelect={setTab2} />}
                />
              </Box>
            </State>
          </Specimen>
          <Specimen title="The FAB gives way" note="While a row's question choices are on screen it fades and sinks (v1 fab.js giveWay), and comes back. Small FAB: icon only.">
            <Stack gap="4">
              <Button size="sm" variant="secondary" onClick={() => setAway((a) => !a)} part="gallery.give-way">
                {away() ? 'Hide the choices' : 'Show a question'}
              </Button>
              <State label={away() ? 'given way' : 'in place'}>
                <Box grow>
                  <Frame
                    contained
                    layout="stack"
                    page="list"
                    top={<TopAppBar kind="root" title="Crew" part="gallery.crew-bar" />}
                    list={
                      <>
                        <Rows n={6} {...(away() ? { choicesAt: 1 } : {})} />
                        <Fab icon="plus" label="New job" small onClick={() => {}} giveWayTo={'[data-part~="gallery.choices"]'} part="gallery.fab-small" />
                      </>
                    }
                    main={null}
                    nav={<Nav selected="crew" onSelect={() => {}} />}
                  />
                </Box>
              </State>
            </Stack>
          </Specimen>
        </>
      )}
    </Area>
  );
}
