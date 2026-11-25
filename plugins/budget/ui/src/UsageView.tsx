// Usage (v1 features/usage.js, Settings → Usage → Open usage): spend, tokens and outcomes over 7, 30 or 90 days.
// The four summary tiles, the bars per local day or ISO week (spend, tokens or turns, stacked by harness with a
// legend that hides a series), a card per harness (tokens, spend, cache hit, turns, worked, and the fullest plan
// window's share left with its reset) that Compare narrows to two or three side by side, then the breakdowns by
// harness, by project and by harness · model. Every harness's tokens, turns and outcomes show even when the CLI
// reports no dollar cost.
import { createResource, createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Card, DataCellLead, DataTable, DayBars, Grid, Inline, SettingsNote, Stack, Stats, Tabs, Text, useSignal, Wrap } from '@wisp/ui';
import { plans, TOPIC, usage, type AccountPlan, type UsageRow } from './commands.ts';
import { cliLine, HARNESSES, tokenBreakdown, tokenWords } from './plan.ts';
import { cacheHit, costCell, limitLeft, spendTile, stackedModel, usageModel, type ChartMetric, type UsageScale } from './usage-model.ts';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

export { usageModel, type ChartMetric } from './usage-model.ts';

const pct = (n: number | null | undefined): string => (n === null || n === undefined ? '—' : `${Math.round(n)}%`);
/** "2m 05s", "41s" (v1 fmtDuration) */
const dur = (ms: number | null): string => {
  if (ms === null) return '—';
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, '0')}s`;
};
/** Compare lays out at most this many harnesses side by side */
const COMPARE_MAX = 3;

function Table(props: {
  readonly rows: readonly UsageRow[];
  readonly lead: (r: UsageRow) => JSX.Element;
  readonly part: `${string}.${string}`;
}): JSX.Element {
  return (
    <DataTable
      part={props.part}
      columns={['', t('budget.usage.tokens'), t('budget.usage.spend'), t('budget.usage.turns'), t('budget.usage.worked'), t('budget.view.asked'), t('budget.view.avg')]}
      empty={t('budget.view.nothing')}
      rows={props.rows.map((r) => {
        const tok = r.tokens ?? 0;
        const tip = tokenBreakdown(r);
        return {
          key: r.key,
          cells: [
            props.lead(r),
            tok > 0 ? <span title={tip || undefined}>{tokenWords(tok)}</span> : '—',
            ((c) => (c.tip ? <span title={c.tip}>{c.text}</span> : c.text))(costCell(r)),
            String(r.turns),
            pct(r.worked),
            String(r.questions),
            dur(r.avgMs),
          ],
        };
      })}
    />
  );
}

export default function UsageView(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const marks = useSignal(useShell().harnesses());
  const nameOf = (id: string): string => marks().find((m) => m.id === id)?.name ?? HARNESSES.find((h) => h.agent === id)?.name ?? id;
  const logoOf = (id: string): string | undefined => marks().find((m) => m.id === id)?.logo;
  const [days, setDays] = createSignal(30);
  const [metric, setMetric] = createSignal<ChartMetric | undefined>(undefined);
  const [scale, setScale] = createSignal<UsageScale>('day');
  const [hidden, setHidden] = createSignal<ReadonlySet<string>>(new Set());
  const [picked, setPicked] = createSignal<readonly string[]>([]);
  // the reader's local days: the runtime starts the window at local midnight and buckets by it
  const tzOffsetMin = -new Date().getTimezoneOffset();
  const [u] = createResource(days, (d) => c.run(usage, { peer: c.home, days: d, tzOffsetMin }).catch(() => undefined));
  // each harness's plan windows (budget.view), for its card's limit and reset; none read is 'not reported'. A view of
  // Cursor, opencode or Grok starts a background read of its CLI (ADR-0119): when it lands, the harness gets its first
  // budget_acc row and this reads again, so the card shows the tier or spend the CLI gave.
  const scope = c.scope.child('usage-view');
  onCleanup(() => scope.dispose());
  c.watch(c.home, TOPIC, scope, { passive: true });
  const accs = useSignal(c.entities.list('budget_acc'));
  const [limits] = createResource(() => accs().length + 1, async () => {
    const out = new Map<string, readonly AccountPlan[]>();
    await Promise.all(HARNESSES.map(async (h) => out.set(h.agent, await c.run(plans, { peer: c.home, agent: h.agent }).catch(() => [] as AccountPlan[]))));
    return out;
  });
  const toggleSeries = (key: string): void => {
    const next = new Set(hidden());
    if (!next.delete(key)) next.add(key);
    setHidden(next);
  };
  // a fourth pick drops the oldest: Compare stays two or three columns wide
  const togglePick = (key: string): void => {
    const p = picked();
    setPicked(p.includes(key) ? p.filter((x) => x !== key) : p.length >= COMPARE_MAX ? [...p.slice(1), key] : [...p, key]);
  };
  return (
    <Stack gap="6" part="budget.view">
      <Tabs
        variant="seg"
        part="budget.view.range"
        aria-label={t('budget.view.range')}
        value={String(days())}
        onChange={(v) => setDays(Number(v))}
        items={[7, 30, 90].map((d) => ({ value: String(d), label: t('budget.view.days', { n: d }) }))}
      />
      <Show when={u()} fallback={<Text tone="muted">{t('budget.view.adding')}</Text>}>
        {(x) => {
          // the week scale: the runtime's ISO-week rollup, stacked or not
          const weeks = (): boolean => (x().weekly?.length ?? 0) > 0 || stacked();
          const m = (): ReturnType<typeof usageModel> => usageModel(x(), metric(), weeks() ? scale() : 'day');
          // an older peer answers without the per-harness days: its bars stay one colour, per day
          const stacked = (): boolean => (x().dailyByHarness?.length ?? 0) > 0;
          const s = (): ReturnType<typeof stackedModel> => stackedModel(x(), m().active, { hidden: hidden(), scale: scale(), nameOf });
          const chartTitle = (): string => {
            const week = weeks() && scale() === 'week';
            if (m().active === 'cost') return week ? `${t('budget.usage.spend')} · ${t('budget.view.perWeek')}` : t('budget.view.spendPerDay');
            if (m().active === 'tokens') return week ? `${t('budget.usage.tokens')} · ${t('budget.view.perWeek')}` : t('budget.view.tokensPerDay');
            return week ? `${t('budget.usage.turns')} · ${t('budget.view.perWeek')}` : t('budget.view.turnsPerDay');
          };
          const spendNote = (): string => {
            const priceNote = m().unpriced ? t('budget.view.unpriced', { count: m().unpriced }) : t('budget.view.allPriced');
            return m().tokens > 0 ? `${t('budget.usage.harness.tokens', { n: tokenWords(m().tokens) })} · ${priceNote}` : priceNote;
          };
          const turnsNote = (): string => {
            const sess = t('budget.view.sessions', { count: x().sessions ?? 0 });
            return m().harnesses.length > 0 ? `${sess} · ${t('budget.view.harnessesCount', { count: m().harnesses.length })}` : sess;
          };
          const cards = (): readonly UsageRow[] => {
            const p = picked();
            const rows = m().harnesses;
            return p.length >= 2 ? p.flatMap((k) => rows.filter((r) => (r.agent || r.key) === k)) : rows;
          };
          const limitOf = (agent: string): string => {
            const h = HARNESSES.find((x) => x.agent === agent) ?? { agent, name: nameOf(agent), key: '', planned: false, priced: false };
            const l = limitLeft(h, limits()?.get(agent) ?? [], Date.now());
            // no windows: say so, and what its CLI said of its plan and spend (Cursor's tier, opencode / Grok's own sum)
            if ('none' in l) return [l.none, cliLine(h, limits()?.get(agent) ?? [])].filter(Boolean).join(' · ');
            return [l.full ? t('budget.view.atLimit') : l.left, l.resets].filter(Boolean).join(' · ');
          };
          return (
            <>
              <Stats
                compact
                part="budget.view.tiles"
                items={[
                  { ...spendTile(x()), note: spendNote() },
                  { label: t('budget.usage.turns'), value: x().turns, note: turnsNote() },
                  { label: t('budget.usage.worked'), value: pct(x().worked), note: t('budget.view.workedNote') },
                  { label: t('budget.usage.questions'), value: x().questions, note: t('budget.view.questionsNote') },
                ]}
              />
              <Stack gap="3">
                <Inline justify="between" align="center" gap="3">
                  <Text weight="medium">{chartTitle()}</Text>
                  <Inline gap="3" align="center">
                    <Show when={weeks()}>
                      <Tabs
                        variant="pill"
                        part="budget.view.scale"
                        aria-label={t('budget.view.scale')}
                        value={scale()}
                        onChange={(v) => setScale(v as UsageScale)}
                        items={[
                          { value: 'day', label: t('budget.view.scale.day') },
                          { value: 'week', label: t('budget.view.scale.week') },
                        ]}
                      />
                    </Show>
                    <Show when={m().byCost || m().byTokens}>
                      <Tabs
                        variant="pill"
                        part="budget.view.metric"
                        aria-label={chartTitle()}
                        value={m().active}
                        onChange={(v) => setMetric(v as ChartMetric)}
                        items={[
                          ...(m().byCost ? [{ value: 'cost', label: t('budget.usage.spend') }] : []),
                          ...(m().byTokens ? [{ value: 'tokens', label: t('budget.usage.tokens') }] : []),
                          { value: 'turns', label: t('budget.usage.turns') },
                        ]}
                      />
                    </Show>
                  </Inline>
                </Inline>
                <Show
                  when={stacked()}
                  fallback={<DayBars part="budget.view.chart" label={chartTitle()} bars={m().bars} axis={[m().first, m().max, t('budget.view.today')]} />}
                >
                  <DayBars
                    part="budget.view.chart"
                    label={chartTitle()}
                    bars={s().bars}
                    axis={[s().first, s().max, t('budget.view.today')]}
                    legend={s().series}
                    onToggle={toggleSeries}
                  />
                </Show>
              </Stack>
              <Show when={m().harnesses.length > 0}>
                <Stack gap="3">
                  <Inline justify="between" align="center" gap="3">
                    <Text weight="medium">{t('budget.view.cards')}</Text>
                    <Show when={picked().length > 0}>
                      <Button variant="link" size="sm" part="budget.view.compare.clear" onClick={() => setPicked([])}>
                        {t('budget.view.compareAll')}
                      </Button>
                    </Show>
                  </Inline>
                  <Show when={m().harnesses.length > 1}>
                    <Wrap gap="2" part="budget.view.compare" aria-label={t('budget.view.compare')}>
                      <For each={m().harnesses}>
                        {(r) => {
                          const id = r.agent || r.key;
                          return (
                            <Button variant="chip" size="sm" logo={logoOf(id)} pressed={picked().includes(id)} onClick={() => togglePick(id)}>
                              {nameOf(id)}
                            </Button>
                          );
                        }}
                      </For>
                    </Wrap>
                  </Show>
                  <Grid part="budget.view.cards" gap="4" {...(picked().length >= 2 ? { columns: picked().length as 2 | 3 } : { min: 'popover' as const })}>
                    <For each={cards()}>
                      {(r) => {
                        const id = r.agent || r.key;
                        return (
                          <Card part="budget.view.card" header={<DataCellLead logo={logoOf(id)} label={nameOf(id)} sub={limitOf(id)} />}>
                            <Stats
                              compact
                              part="budget.view.card.stats"
                              items={[
                                { label: t('budget.usage.tokens'), value: (r.tokens ?? 0) > 0 ? tokenWords(r.tokens ?? 0) : '—' },
                                { label: t('budget.usage.spend'), value: costCell(r).text },
                                { label: t('budget.view.cacheHit'), value: cacheHit(r) },
                                { label: t('budget.usage.turns'), value: r.turns },
                                { label: t('budget.usage.worked'), value: pct(r.worked) },
                              ]}
                            />
                          </Card>
                        );
                      }}
                    </For>
                  </Grid>
                </Stack>
              </Show>
              <Stack gap="3">
                <Text weight="medium">{t('budget.view.byHarness')}</Text>
                <Table
                  part="budget.view.harnesses"
                  rows={m().harnesses}
                  lead={(r) => {
                    const id = r.agent || r.key;
                    return (
                      <DataCellLead
                        logo={logoOf(id)}
                        label={nameOf(id)}
                        sub={r.sessions ? t('budget.view.tasksCount', { count: r.sessions }) : undefined}
                      />
                    );
                  }}
                />
              </Stack>
              <Stack gap="3">
                <Text weight="medium">{t('budget.view.byProject')}</Text>
                <Table part="budget.view.projects" rows={x().projects ?? []} lead={(r) => <DataCellLead label={r.name} />} />
              </Stack>
              <Stack gap="3">
                <Text weight="medium">{t('budget.view.byRunner')}</Text>
                <Table
                  part="budget.view.runners"
                  rows={x().runners ?? []}
                  lead={(r) => (
                    <DataCellLead
                      logo={logoOf(r.agent ?? '')}
                      label={`${nameOf(r.agent ?? '')} · ${r.name || t('budget.view.defaultModel')}`}
                    />
                  )}
                />
              </Stack>
              <Show when={m().unpriced > 0}>
                <SettingsNote part="budget.view.note">{t('budget.view.unpricedNote')}</SettingsNote>
              </Show>
            </>
          );
        }}
      </Show>
    </Stack>
  );
}
