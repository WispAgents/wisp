import { expect, test } from 'vitest';
import { boxPreviewUrl } from '../src/preview.ts';

const T = 'https://p{port}--ada.grid.example';
const APP = 'https://ada.grid.example/t/1';

test('a_box_s_localhost_shows_at_its_grid_preview_address', () => {
  expect(boxPreviewUrl('http://localhost:5173/settings?tab=1#top', T, APP)).toBe('https://p5173--ada.grid.example/settings?tab=1#top');
  expect(boxPreviewUrl('http://127.0.0.1:3000/', T, APP)).toBe('https://p3000--ada.grid.example/');
  expect(boxPreviewUrl('http://[::1]:8000/x', T, APP)).toBe('https://p8000--ada.grid.example/x');
  expect(boxPreviewUrl('http://0.0.0.0:4321/', 'http://p{port}--ada.wisp.localhost:8080', 'http://ada.wisp.localhost:8080/')).toBe('http://p4321--ada.wisp.localhost:8080/');
});

test('anything_else_keeps_the_plain_url', () => {
  // not a loopback page, or no template (not in a box)
  expect(boxPreviewUrl('https://example.com/', T, APP)).toBeUndefined();
  expect(boxPreviewUrl('http://localhost:5173/', '', APP)).toBeUndefined();
  expect(boxPreviewUrl('http://localhost:5173/', undefined, APP)).toBeUndefined();
  // a port the grid never shows (system ports, the default 80 / 443)
  expect(boxPreviewUrl('http://localhost/', T, APP)).toBeUndefined();
  expect(boxPreviewUrl('https://localhost/', T, APP)).toBeUndefined();
  expect(boxPreviewUrl('http://localhost:1023/', T, APP)).toBeUndefined();
  expect(boxPreviewUrl('http://localhost:1024/', T, APP)).toBe('https://p1024--ada.grid.example/');
  // a template that isn't a grid's preview address: never followed
  for (const bad of ['https://evil.example/{port}', 'javascript:alert({port})', 'https://p{port}--ada.grid.example/path', 'https://p{port}--x@evil.example']) {
    expect(boxPreviewUrl('http://localhost:5173/', bad, APP), bad).toBeUndefined();
  }
  // this window is itself on loopback: its localhost is the box's, so the URL loads as written
  expect(boxPreviewUrl('http://localhost:5173/', T, 'http://localhost:8080/')).toBeUndefined();
  expect(boxPreviewUrl('not a url', T, APP)).toBeUndefined();
});
