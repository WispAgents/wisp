// wisp Cloud on the Computers screen (ADR-0065 §2.3-4, design/mobile.md §A.2 Computers and Part B, F-phone-5): one tap
// to sign in (*Continue with GitHub* / *Google*); a grid address only behind *Use your own wisp Grid*. The sign-in is the
// grid's device flow (cloud.ts, run by cloud-accounts.ts): a sheet opens at once with the code (filled in when the grid
// answers), the system browser opens on the provider, and the sheet waits for *Allow* there; coming back to the app asks
// at once. Each account is one card (the kit's Section: M3 list under a subheader / an iOS inset grouped section): its
// name and handle, how it signed in and the grid's name when it isn't wisp Cloud, its cloud wisp and its computers on
// the grid as rows openable like any saved computer, *Sign out* and under it the quiet *Delete your account* link (the
// grid's own page confirms). The device token is sealed in the phone's store (SecureStore) and handed to the native side
// with each computer; nothing secret is in web storage. Kit components only, no CSS.
import { createSignal, For, Show, type JSX } from 'solid-js';
import type { FetchLike } from '@wisp/client';
import type { SecureStore } from '@wisp/shell/platform';
import { Box, Button, Field, Input, Pill, Section, Sheet, Stack, Text, useSignal, Wrap } from '@wisp/ui';
import type { Computer } from '../bridge.ts';
import type { CloudAccounts } from '../cloud-accounts.ts';
import { accountComputers, DEFAULT_GRID, gridUrl, hostOf, providersFor, signInUrl, type GridAddressError, type PollDeps, type Provider } from '../cloud.ts';

export interface CloudDeps {
  readonly look: 'android' | 'ios';
  readonly fetch: FetchLike;
  /** the phone's sealed store (platform.secureStore); none: signing in says this phone can't keep it */
  readonly secure: SecureStore | undefined;
  /** where the account list (no secrets) is kept */
  readonly storage: {
    getItem(k: string): string | null;
    setItem(k: string, v: string): void;
  };
  readonly sleep: PollDeps['sleep'];
  readonly now: () => number;
  /** the provider's page over the app (Custom Tabs, ASWebAuthenticationSession) */
  readonly openInApp: (url: string) => void;
  /** the system browser (the grid's account page, to delete the account) */
  readonly openExternal: (url: string) => void;
  /** the app came back (from the browser): poll at once */
  readonly onResume: (fn: () => void) => () => void;
  readonly copy?: (text: string) => Promise<boolean>;
  /** what this phone calls itself to the grid */
  readonly deviceName: string;
}

type T = (k: string, vars?: Record<string, string>) => string;

const LABEL: Record<Provider, string> = { github: 'GitHub', google: 'Google', apple: 'Apple' };

export interface CloudProps {
  readonly t: T;
  readonly deps: CloudDeps;
  readonly cloud: CloudAccounts;
  /** the saved computers (the screen's list) */
  readonly list: () => readonly Computer[];
  /** a saved computer's row, as the screen draws every one */
  readonly row: (c: Computer) => JSX.Element;
}

// size-ok: the cards, the sign-in choices and the sign-in sheet, one screen's section
export function Cloud(props: CloudProps): JSX.Element {
  const t = props.t;
  const d = props.deps;
  const accounts = useSignal(() => props.cloud.accounts());
  const signIn = useSignal(() => props.cloud.signIn());
  const [own, setOwn] = createSignal(false);
  const [address, setAddress] = createSignal('');
  const [addrError, setAddrError] = createSignal<GridAddressError | undefined>();
  const [adding, setAdding] = createSignal(false);
  const begin = (grid: string, via: Provider | undefined): void => {
    setAdding(false);
    setOwn(false);
    void props.cloud.begin(grid, via);
  };
  const cancel = (): void => props.cloud.cancel();
  const ownGrid = (): void => {
    const g = gridUrl(address());
    if (typeof g !== 'string') return void setAddrError(g.error);
    begin(g, undefined);
  };
  // a card per account key: Sign out or a refresh replaces the account object, not the card
  const keys = (): string[] => accounts().map((a) => a.key);
  const byKey = (key: string) => accounts().find((a) => a.key === key);

  const providers = (): JSX.Element => (
    <Wrap gap="4" padX="7" padY="3" part="phone.cloud.providers">
      <For each={providersFor(d.look)}>
        {(p, i) => (
          <Button
            variant={i() === 0 ? 'primary' : 'secondary'}
            {...(p === 'github' && { icon: 'github' as const })}
            part={`phone.cloud.${p}`}
            onClick={() => begin(DEFAULT_GRID, p)}
          >
            {t('phone.cloud.continue', { provider: LABEL[p] })}
          </Button>
        )}
      </For>
      <Show
        when={own()}
        fallback={
          <Button variant="link" part="phone.cloud.own" onClick={() => setOwn(true)}>
            {t('phone.cloud.own')}
          </Button>
        }
      >
        <Field label={t('phone.cloud.address')} hint={t('phone.cloud.addressHint')} {...(addrError() && { error: t(`phone.cloud.addressError.${addrError()}`) })}>
          <Input
            value={address()}
            inputMode="url"
            placeholder="wisp.example.com"
            onInput={(v) => {
              setAddress(v);
              setAddrError(undefined);
            }}
            part="phone.cloud.address"
          />
        </Field>
        <Button variant="primary" part="phone.cloud.own-go" onClick={ownGrid}>
          {t('phone.cloud.signIn')}
        </Button>
      </Show>
    </Wrap>
  );

  return (
    <>
      <For each={keys()}>
        {(key) => (
          <Show when={byKey(key)}>
            {(acct) => {
              const a = acct();
              return (
                <Section
                  part="phone.cloud.account"
                  title={acct().name && acct().name !== acct().handle ? `${acct().name} · @${acct().handle}` : `@${acct().handle}`}
                  description={
                    [acct().via ? t('phone.cloud.via', { provider: LABEL[acct().via as Provider] }) : '', a.grid === DEFAULT_GRID ? '' : hostOf(a.grid)]
                      .filter(Boolean)
                      .join(' · ') || undefined
                  }
                >
                  <Show when={acct().signedOut}>
                    <Box padX="7" padY="3">
                      <Text tone="warn" part="phone.cloud.signedOutNote">
                        {t('phone.cloud.signedOutHere')}
                      </Text>
                    </Box>
                  </Show>
                  {/* keyed by id: a list reload's new objects keep each row (its open dialog, its state) */}
                  <For each={accountComputers(a, props.list()).map((c) => c.id)}>{(id) => <Show when={props.list().find((c) => c.id === id)}>{(c) => props.row(c())}</Show>}</For>
                  <Stack gap="3" align="start" padX="7" padY="3" part="phone.cloud.account.foot">
                    <Wrap gap="4" part="phone.cloud.account.acts">
                      <Show when={acct().signedOut}>
                        <Button variant="primary" size="sm" part="phone.cloud.again" onClick={() => begin(a.grid, acct().via)}>
                          {t('phone.cloud.again')}
                        </Button>
                      </Show>
                      <Button variant="secondary" size="sm" part="phone.cloud.signout" onClick={() => void props.cloud.signOut(key)}>
                        {t('phone.cloud.signOut')}
                      </Button>
                    </Wrap>
                    <Button variant="link" tone="bad" part="phone.cloud.delete" onClick={() => d.openExternal(`${a.grid}/account#delete`)}>
                      {t('phone.cloud.delete')}
                    </Button>
                  </Stack>
                </Section>
              );
            }}
          </Show>
        )}
      </For>
      <Show
        when={accounts().length === 0 || adding()}
        fallback={
          <Section title={t('phone.cloud.title')} part="phone.cloud.more">
            <Box padX="7" padY="3">
              <Button variant="secondary" icon="plus" part="phone.cloud.add" onClick={() => setAdding(true)}>
                {t('phone.cloud.addAccount')}
              </Button>
            </Box>
          </Section>
        }
      >
        <Section title={t(accounts().length ? 'phone.cloud.addAccount' : 'phone.cloud.title')} description={t('phone.cloud.how')} part="phone.cloud">
          {providers()}
        </Section>
      </Show>
      <Sheet
        title={(() => {
          const s = signIn();
          return s?.via
            ? t('phone.cloud.continue', { provider: LABEL[s.via] })
            : t('phone.cloud.signInTo', {
                host: hostOf(s?.grid ?? DEFAULT_GRID),
              });
        })()}
        open={!!signIn()}
        onOpenChange={(o) => !o && cancel()}
        part="phone.cloud.sheet"
      >
        <Show when={signIn()}>
          {(s) => (
            <Show
              when={!s().error}
              fallback={
                <Section title={t('phone.cloud.failed')} part="phone.cloud.error">
                  <Text tone="bad" part="phone.cloud.error.text">
                    {s().error}
                  </Text>
                  <Wrap gap="4">
                    <Button variant="primary" part="phone.cloud.retry" onClick={() => begin(s().grid, s().via)}>
                      {t('phone.cloud.retry')}
                    </Button>
                    <Button variant="secondary" onClick={cancel}>
                      {t('phone.cancel')}
                    </Button>
                  </Wrap>
                </Section>
              }
            >
              <Section
                title={t('phone.cloud.finish')}
                description={
                  s().via
                    ? t('phone.cloud.finishVia', {
                        provider: LABEL[s().via as Provider],
                      })
                    : t('phone.cloud.finishGrid', { host: hostOf(s().grid) })
                }
                part="phone.cloud.wait"
              >
                <Text mono size="lg" part="phone.cloud.code">
                  {s().flow?.userCode ?? '…'}
                </Text>
                <Pill tone="info" dot live variant="plain" size="sm">
                  {t(s().flow ? 'phone.cloud.waiting' : 'phone.cloud.asking', {
                    host: hostOf(s().grid),
                  })}
                </Pill>
                <Wrap gap="4">
                  <Button
                    variant="primary"
                    part="phone.cloud.reopen"
                    loading={!s().flow}
                    onClick={() => {
                      const f = s().flow;
                      if (f) d.openInApp(signInUrl(f, s().via, d.look));
                    }}
                  >
                    {s().via
                      ? t('phone.cloud.reopenVia', {
                          provider: LABEL[s().via as Provider],
                        })
                      : t('phone.cloud.reopen')}
                  </Button>
                  <Show when={d.copy && s().flow}>
                    <Button variant="secondary" part="phone.cloud.copy" onClick={() => void d.copy?.(s().flow?.userCode ?? '')}>
                      {t('phone.cloud.copy')}
                    </Button>
                  </Show>
                  <Button variant="ghost" part="phone.cloud.cancel" onClick={cancel}>
                    {t('phone.cancel')}
                  </Button>
                </Wrap>
              </Section>
            </Show>
          )}
        </Show>
      </Sheet>
    </>
  );
}
