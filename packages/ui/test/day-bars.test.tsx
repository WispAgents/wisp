// DayBars (composites/Charts.tsx): a stacked bar draws a part per series, coloured by its series, each sized against
// its bar; the legend lists the series, marks a hidden one and hands its key to onToggle.
import { createSignal } from 'solid-js';
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { DayBars, type ChartSeries, type DayBar } from '../src/composites/Charts.tsx';
import { untilLabel } from '../src/content/until-label.ts';

const bars: DayBar[] = [
  { key: 'a', share: 1, tip: 'Nov 1: Claude 3 · Codex 1', stack: [{ series: 0, share: 0.75 }, { series: 1, share: 0.25 }] },
  { key: 'b', share: 0, tip: 'Nov 2: 0', stack: [] },
];

test('day bars: a stacked bar draws its parts by series, sized within the bar', () => {
  const host = mount(() => <DayBars label="Turns per day" bars={bars} axis={['Nov 1', 'max 4', 'Today']} />);
  const parts = [...host.querySelectorAll<HTMLElement>('[data-kit="day-bar-stack"] i')];
  expect(parts.map((p) => p.dataset['series'])).toEqual(['0', '1']);
  expect(parts.map((p) => p.style.getPropertyValue('--k-v'))).toEqual(['75%', '25%']);
  const stacks = [...host.querySelectorAll<HTMLElement>('[data-kit="day-bar-stack"]')];
  expect(stacks.map((s) => s.style.getPropertyValue('--k-v'))).toEqual(['100%', '0%']);
  expect(host.querySelector('[data-kit="day-bars-legend"]')).toBeNull();
});

test('day bars: the legend toggles a series and shows it hidden', () => {
  const [hidden, setHidden] = createSignal<ReadonlySet<string>>(new Set());
  const toggled: string[] = [];
  const legend = (): ChartSeries[] => [
    { key: 'claude', label: 'Claude Code', series: 0, off: hidden().has('claude') },
    { key: 'codex', label: 'Codex', series: 1, off: hidden().has('codex') },
  ];
  const host = mount(() => (
    <DayBars
      label="Turns per day"
      bars={bars}
      axis={['Nov 1', 'max 4', 'Today']}
      legend={legend()}
      onToggle={(k) => {
        toggled.push(k);
        const n = new Set(hidden());
        if (!n.delete(k)) n.add(k);
        setHidden(n);
      }}
    />
  ));
  const keys = (): HTMLButtonElement[] => [...host.querySelectorAll<HTMLButtonElement>('[data-kit="day-bars-key"]')];
  expect(keys().map((k) => [k.textContent, k.dataset['series'], k.getAttribute('aria-pressed')])).toEqual([
    ['Claude Code', '0', 'true'],
    ['Codex', '1', 'true'],
  ]);
  keys()[1]?.click();
  expect(toggled).toEqual(['codex']);
  expect(keys()[1]?.getAttribute('aria-pressed')).toBe('false');
  keys()[1]?.click();
  expect(keys()[1]?.getAttribute('aria-pressed')).toBe('true');
});

test('until label: minutes, hours and minutes, days and hours', () => {
  expect(untilLabel(10_000)).toBe('1m');
  expect(untilLabel(45 * 60_000)).toBe('45m');
  expect(untilLabel((2 * 60 + 10) * 60_000)).toBe('2h 10m');
  expect(untilLabel((3 * 24 + 4) * 3_600_000)).toBe('3d 4h');
});
