// `npm run gallery` in packages/ui. ?theme=light|dark picks the theme (default: the OS's); ?platform=android|ios
// draws the phone apps' look (ADR-0065 §5), as the shell does in the apps.
import '../src/css/index.css';
import { render } from 'solid-js/web';
import { createSignal, type JSX } from 'solid-js';
import { Box, Button, Heading, PlatformLookProvider, Stack, Text, Wrap, LOOK_ATTR, loadPlatformStyles, lookOf, type Look } from '../src/index.ts';
import { ContentGallery } from './sections/content.tsx';
import { RichGallery } from './sections/rich.tsx';
import { ControlsGallery } from './sections/controls.tsx';
import { CompositesGallery } from './sections/composites.tsx';
import { SettingsGallery } from './sections/settings.tsx';
import { BlocksGallery } from './sections/blocks.tsx';
import { ShellGallery } from './sections/shell.tsx';
import { PhoneGallery } from './sections/phone.tsx';
import { PhoneRowsGallery } from './sections/phone-rows.tsx';
import { PhoneOverlaysGallery } from './sections/phone-overlays.tsx';
import { PhoneControlsGallery } from './sections/phone-controls.tsx';
import { TerminalGallery } from './sections/terminal.tsx';
import { LoadersGallery } from './sections/loaders.tsx';

type Theme = 'light' | 'dark' | 'system';

function initialTheme(): Theme {
  const t = new URLSearchParams(window.location.search).get('theme');
  return t === 'light' || t === 'dark' ? t : 'system';
}

function applyTheme(t: Theme): void {
  if (t === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.setAttribute('data-theme', t);
}

const LOOKS: readonly Look[] = ['web', 'android', 'ios'];
const LOOK_LABEL: Record<Look, string> = { web: 'Platform: browser', android: 'Platform: Android', ios: 'Platform: iPhone' };

function applyLook(l: Look): void {
  if (l === 'web') document.documentElement.removeAttribute(LOOK_ATTR);
  else document.documentElement.setAttribute(LOOK_ATTR, l);
  void loadPlatformStyles(l);
}

function Gallery(): JSX.Element {
  const [look, setLook] = createSignal<Look>(lookOf(new URLSearchParams(window.location.search).get('platform')));
  applyLook(look());
  const nextLook = (): void => {
    const next = LOOKS[(LOOKS.indexOf(look()) + 1) % LOOKS.length] ?? 'web';
    setLook(next);
    applyLook(next);
  };
  const [theme, setTheme] = createSignal<Theme>(initialTheme());
  applyTheme(theme());
  const cycle = (): void => {
    const next: Theme = theme() === 'dark' ? 'light' : 'dark';
    setTheme(next);
    applyTheme(next);
  };
  return (
    <PlatformLookProvider look={look}>
    <Box as="main" part="gallery.page" padX="7" padY="8">
      <Stack gap="9">
        <Wrap gap="6" justify="between">
          <Stack gap="1">
            <Heading level={1}>wisp kit</Heading>
            <Text tone="muted">Every @wisp/ui component in every state.</Text>
          </Stack>
          <Wrap gap="4">
            <Button variant="secondary" size="sm" onClick={nextLook} part="gallery.platform">
              {LOOK_LABEL[look()]}
            </Button>
            <Button variant="secondary" size="sm" icon={theme() === 'dark' ? 'moon' : 'sun'} onClick={cycle} part="gallery.theme">
              {theme() === 'system' ? 'Theme: system' : theme() === 'dark' ? 'Theme: dark' : 'Theme: light'}
            </Button>
          </Wrap>
        </Wrap>
        <LoadersGallery />
        <ContentGallery />
        <RichGallery />
        <ControlsGallery />
        <CompositesGallery />
        <SettingsGallery />
        <BlocksGallery />
        <ShellGallery />
        <PhoneGallery />
        <PhoneRowsGallery />
        <PhoneOverlaysGallery />
        <PhoneControlsGallery />
        <TerminalGallery />
      </Stack>
    </Box>
    </PlatformLookProvider>
  );
}

const root = document.getElementById('root');
if (root) render(() => <Gallery />, root);
