// Ship it's dialog params (an overlay view's, checked as the app navigates to it): apart from the dialog, which loads on
// first open, so the plugin's index needs only this (ADR-0011).
import type { EntityKey } from '@wisp/client';
import type { ShipGuess } from './model/ship.ts';

export interface ShipParams {
  readonly key: EntityKey;
  readonly guess: ShipGuess;
}

const isGuess = (g: unknown): g is ShipGuess =>
  !!g && typeof g === 'object' && typeof (g as ShipGuess).repos === 'number' && typeof (g as ShipGuess).update === 'boolean' && Array.isArray((g as ShipGuess).branches);

export function parseShipParams(v: unknown): ShipParams {
  const o = (v ?? {}) as { key?: unknown; guess?: unknown };
  if (typeof o.key !== 'string' || !o.key.includes(':')) throw new Error('Ship it needs the task’s key');
  return { key: o.key as EntityKey, guess: isGuess(o.guess) ? o.guess : { repos: 1, update: false, branches: [] } };
}
