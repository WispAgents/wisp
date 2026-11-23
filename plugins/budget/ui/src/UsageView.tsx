// Usage (v1 features/usage.js, Settings → Usage → Open usage): spend, tokens and outcomes over 7, 30 or 90 days.
// The four summary tiles, daily bars (spend, tokens or turns per day), then breakdowns by harness, by project
// and by harness · model. Every harness's tokens, turns and outcomes show even when the CLI reports no dollar cost.
import { createResource, createSignal, Show, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { DataCellLead, DataTable, DayBars, Inline, SettingsNote, Stack, Stats, Tabs, Text, useSignal } from '@wisp/ui';
import { usage, type UsageRow } from './commands.ts';
import { HARNESSES, tokenBreakdown, tokenWords, usageModel, type ChartMetric } from './plan.ts';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

export { usageModel, type ChartMetric } from './plan.ts';

const money = (n: number): string => `$${n.toFixed(2)}`;
const pct = (n: number | null | undefined): string => (n === null || n === undefined ? '—' : `${Math.round(n)}%`);
/** "2m 05s", "41s" (v1 fmtDuration) */
const dur = (ms: number | null): string => {
  if (ms === null) return '—';
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, '0')}s`;
};

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
            r.priced ? money(r.cost) : '—',
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
  const [u] = createResource(days, (d) => c.run(usage, { peer: c.home, days: d }).catch(() => undefined));
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
          const m = (): ReturnType<typeof usageModel> => usageModel(x(), metric());
          const chartTitle = (): string =>
            m().active === 'cost' ? t('budget.view.spendPerDay') : m().active === 'tokens' ? t('budget.view.tokensPerDay') : t('budget.view.turnsPerDay');
          const spendNote = (): string => {
            const priceNote = m().unpriced ? t('budget.view.unpriced', { count: m().unpriced }) : t('budget.view.allPriced');
            return m().tokens > 0 ? `${t('budget.usage.harness.tokens', { n: tokenWords(m().tokens) })} · ${priceNote}` : priceNote;
          };
          const turnsNote = (): string => {
            const sess = t('budget.view.sessions', { count: x().sessions ?? 0 });
            return m().harnesses.length > 0 ? `${sess} · ${t('budget.view.harnessesCount', { count: m().harnesses.length })}` : sess;
          };
          return (
            <>
              <Stats
                compact
                part="budget.view.tiles"
                items={[
                  {
                    label: t('budget.usage.spend'),
                    value: m().byCost ? money(x().spend) : m().tokens > 0 ? tokenWords(m().tokens) : '—',
                    note: spendNote(),
                  },
                  { label: t('budget.usage.turns'), value: x().turns, note: turnsNote() },
                  { label: t('budget.usage.worked'), value: pct(x().worked), note: t('budget.view.workedNote') },
                  { label: t('budget.usage.questions'), value: x().questions, note: t('budget.view.questionsNote') },
                ]}
              />
              <Stack gap="3">
                <Inline justify="between" align="center" gap="3">
                  <Text weight="medium">{chartTitle()}</Text>
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
                <DayBars
                  part="budget.view.chart"
                  label={chartTitle()}
                  bars={m().bars}
                  axis={[m().first, m().max, t('budget.view.today')]}
                />
              </Stack>
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
