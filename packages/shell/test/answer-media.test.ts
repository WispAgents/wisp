// An answer's pictures load from the task's own computer (src/frame/answer-of.ts mediaUrl, v1 ctx.fileUrl).
import { expect, test } from 'vitest';
import { mediaUrl, uploadUrl } from '../src/frame/answer-of.ts';
import { reveal } from '../src/frame/commands.ts';
import { wireOf } from './command.ts';

test('a file comes from the tasks computer, through this wisp for another; a link stays a link_v1_media_url', () => {
  expect(mediaUrl('home', 'home:s1', 'shots/a b.png')).toBe('/api/sessions/s1/file?path=shots%2Fa%20b.png');
  expect(mediaUrl('home', 'studio:s2', '/tmp/x.png')).toBe('/api/peer/studio/api/sessions/s2/file?path=%2Ftmp%2Fx.png');
  expect(mediaUrl('home', 'home:s1', 'https://x.dev/a.png')).toBe('https://x.dev/a.png');
  expect(mediaUrl('home', 'home:s1', '')).toBeUndefined();
  // Show in folder runs on the task's computer, with the path as the block named it
  expect(wireOf(reveal)({ id: 'studio:s2', path: 'a.png' })).toEqual({ id: 's2', path: 'a.png' });
});

test('a file sent with a prompt loads from the tasks computer by its upload id; no id, no URL_v1_ask_atts', () => {
  expect(uploadUrl('home', 'home:s1', 'u1')).toBe('/api/uploads/u1');
  expect(uploadUrl('home', 'studio:s2', 'u/2')).toBe('/api/peer/studio/api/uploads/u%2F2');
  expect(uploadUrl('home', 'home:s1', '')).toBeUndefined();
});
