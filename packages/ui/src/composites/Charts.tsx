// Numbers over time and in rows (v1 usage.js, Settings → Usage → Open usage): DayBars, one bar per day with its
// tip on hover and the axis under it (.us-chart, .us-axis); DataTable, a breakdown whose first column names the
// row and the rest are numbers on the right (.us-table).
import { createSignal, For, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';

export interface DayBar {
  readonly key: string;
  /** 0–1 of the tallest */
  readonly share: number;
  /** what hovering it says: "Nov 12: $0.40 · 3 turns" */
  readonly tip: string;
  /** a stacked bar: its parts bottom up, each 0–1 of the tallest bar and coloured by its series (0–8) */
  readonly stack?: readonly { readonly series: number; readonly share: number }[];
}

/** One series of a stacked DayBars, in its legend: pressing it hides or shows the series. */
export interface ChartSeries {
  readonly key: string;
  readonly label: string;
  /** 0–8: the colour, the same in the bars */
  readonly series: number;
  readonly off?: boolean;
}

export interface DayBarsProps extends KitProps {
  readonly bars: readonly DayBar[];
  readonly label: string;
  /** under the bars: the first day, the scale, today */
  readonly axis: readonly [string, string, string];
  /** a stacked chart's series: a legend under the axis whose items toggle */
  readonly legend?: readonly ChartSeries[];
  readonly onToggle?: (key: string) => void;
}

export function DayBars(props: DayBarsProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.day-bars';
  const [tip, setTip] = createSignal<{ text: string; at: number } | undefined>();
  return (
    <div id={props.id} data-kit="day-bars" data-part={part()}>
      <div data-kit="day-bars-plot" role="img" aria-label={props.label} onMouseLeave={() => setTip(undefined)}>
        <For each={props.bars}>
          {(b, i) => (
            <div data-kit="day-bar" title={b.tip} onMouseEnter={() => setTip({ text: b.tip, at: (i() + 0.5) / Math.max(1, props.bars.length) })}>
              <Show when={b.stack} fallback={<i style={{ '--k-v': `${b.share > 0 ? Math.max(2, b.share * 100) : 0}%` }} />}>
                {(parts) => (
                  <span data-kit="day-bar-stack" style={{ '--k-v': `${b.share > 0 ? Math.max(2, b.share * 100) : 0}%` }}>
                    <For each={parts()}>
                      {(p) => <i data-series={p.series % 9} style={{ '--k-v': `${b.share > 0 ? (p.share / b.share) * 100 : 0}%` }} />}
                    </For>
                  </span>
                )}
              </Show>
            </div>
          )}
        </For>
        <Show when={tip()}>
          {(x) => (
            <span data-kit="day-bars-tip" style={{ '--k-x': `${x().at * 100}%` }}>
              {x().text}
            </span>
          )}
        </Show>
      </div>
      <div data-kit="day-bars-axis" data-part={sub(part(), 'axis')}>
        <span>{props.axis[0]}</span>
        <span>{props.axis[1]}</span>
        <span>{props.axis[2]}</span>
      </div>
      <Show when={props.legend?.length}>
        <div data-kit="day-bars-legend" data-layout="wrap" data-part={sub(part(), 'legend')}>
          <For each={props.legend}>
            {(s) => (
              <button type="button" data-kit="day-bars-key" data-series={s.series % 9} aria-pressed={!s.off} onClick={() => props.onToggle?.(s.key)}>
                <i />
                {s.label}
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

export interface DataCellLeadProps {
  readonly logo?: string | undefined;
  readonly label: string;
  readonly sub?: string | undefined;
}

/** First cell of a breakdown row: optional 14px harness logo, name, and quiet secondary note. */
export function DataCellLead(props: DataCellLeadProps): JSX.Element {
  return (
    <span data-kit="data-cell-lead">
      <Show when={props.logo}>{(src) => <img data-kit="data-cell-logo" src={src()} alt="" />}</Show>
      <span data-kit="data-cell-label">{props.label}</span>
      <Show when={props.sub}>{(s) => <small data-kit="data-cell-sub">{s()}</small>}</Show>
    </span>
  );
}

export interface DataTableProps extends KitProps {
  readonly columns: readonly string[];
  readonly rows: readonly { readonly key: string; readonly cells: readonly JSX.Element[] }[];
  /** no rows: this line instead */
  readonly empty: string;
}

export function DataTable(props: DataTableProps): JSX.Element {
  return (
    <Show when={props.rows.length > 0} fallback={<p data-kit="data-table-empty">{props.empty}</p>}>
      <table id={props.id} data-kit="data-table" data-part={props.part ?? 'ui.data-table'}>
        <thead>
          <tr>
            <For each={props.columns}>{(c) => <th scope="col">{c}</th>}</For>
          </tr>
        </thead>
        <tbody>
          <For each={props.rows}>
            {(r) => (
              <tr>
                {r.cells.map((c) => (
                  <td>{c}</td>
                ))}
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </Show>
  );
}
