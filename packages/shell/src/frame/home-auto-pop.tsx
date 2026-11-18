// What will Auto run (v1 openPop), opened from the Auto pick on New task's pill: its pick and why, the facts, a better
// pick worth trying, the others, Never for this kind of task. Its own chunk (budget, ADR-0011): Auto is off unless its
// switch is on (owner #37), so the first page never carries it. Before Auto answers it says it's still looking.
import '../i18n/lazy.ts';
import type { JSX } from 'solid-js';
import { AutoPop } from '@wisp/ui';
import type { HarnessUiDef } from '../contract.ts';
import { useShell } from '../context.tsx';
import { popOf, pct, shortLabel, usd, type AutoAlt, type AutoPreview } from './auto.ts';
import { harnessOf } from './harness.ts';
import { agentName } from './pickers.ts';

type Pick = NonNullable<AutoPreview['pick']>;

export default function HomeAutoPop(props: {
  readonly preview: () => AutoPreview | undefined;
  readonly harnesses: () => readonly HarnessUiDef[];
  readonly neverModel: (pk: Pick) => string;
  readonly typeWord: (type: string) => string;
  readonly onClose: () => void;
  /** run that pick instead (undefined: it's gone) */
  readonly onPick: (c: AutoAlt | undefined) => void;
  readonly onNever: (pk: Pick, type: string) => void;
}): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const p = (): AutoPreview => props.preview() ?? { pick: null, why: '' };
  const pop = () => popOf(p());
  const pk = () => p().pick;
  const row = (a: { agent: string; model: string; key: string; label: string; chance: number; usd: number; why?: string }, sug: boolean) => ({
    key: a.key,
    logo: harnessOf(props.harnesses(), a.agent).logo,
    title: sug ? t('shell.auto.try', { label: shortLabel(a, t('shell.auto.defaultPick')) }) : shortLabel(a, t('shell.auto.defaultPick')),
    sub: sug ? a.why || agentName(t, props.harnesses(), a.agent) : agentName(t, props.harnesses(), a.agent),
    chance: pct(a.chance),
    usd: usd(a.usd),
  });
  const goal = (): string => t(`shell.auto.goal.${p().goal === 'cheap' || p().goal === 'best' ? (p().goal as string) : 'balanced'}`);
  const facts = () => [
    ...(pop().chance ? [{ strong: pop().chance, text: t('shell.auto.likely') }] : []),
    ...(pop().usd ? [{ text: pop().usd }] : []),
    ...(pop().minutes ? [{ text: t('shell.auto.minutes', { n: pop().minutes }) }] : []),
  ];
  return (
    <AutoPop
      anchor={() => document.querySelector('[data-part="launcher.autoPreview"]')}
      label={t('shell.auto.popLabel')}
      head={((x) =>
        x
          ? { logo: harnessOf(props.harnesses(), x.agent).logo, title: shortLabel(x, t('shell.auto.defaultPick')), sub: `${agentName(t, props.harnesses(), x.agent)} · ${goal()}` }
          : { nothing: t('shell.auto.nothing'), why: p().why || t('shell.auto.looking') })(pk())}
      why={pk() ? pop().why : undefined}
      facts={pk() ? facts() : []}
      suggest={((s) => (pk() && s ? { heading: t('shell.auto.worth'), row: row(s, true) } : undefined))(pop().suggest)}
      others={pk() && pop().others.length ? { heading: t('shell.auto.orRun'), rows: pop().others.map((a) => row(a, false)) } : undefined}
      never={((x) => (x ? t('shell.auto.never', { model: props.neverModel(x), kind: props.typeWord(p().type ?? '') }) : undefined))(pk())}
      how={t('shell.auto.how')}
      onClose={props.onClose}
      onHow={() => {
        props.onClose();
        shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'auto' } } });
      }}
      onPick={(key) => props.onPick([p().suggest, ...(p().alternatives ?? [])].find((a) => a?.key === key) ?? undefined)}
      onNever={() => {
        const x = pk();
        if (x) props.onNever(x, p().type ?? '');
        else props.onClose();
      }}
    />
  );
}
