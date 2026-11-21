// apps/web/dev/dev-proxy.ts: what Origin `npm run dev`'s /api proxy hands the real server.
import { describe, expect, it } from 'vitest';
import { proxiedOrigin } from '../dev/dev-proxy.ts';

describe('proxiedOrigin', () => {
  it('the_proxy_gives_vites_own_page_the_servers_origin_and_passes_any_other_through', () => {
    const api = 'http://127.0.0.1:4801';
    expect(proxiedOrigin('http://localhost:5181', 5181, api)).toBe(api);
    expect(proxiedOrigin('http://127.0.0.1:5181', 5181, api)).toBe(api);
    expect(proxiedOrigin(undefined, 5181, api), 'a GET without Origin stays without').toBeUndefined();
    expect(proxiedOrigin('http://localhost:3000', 5181, api), 'a pane on another port is still refused by the edge').toBe('http://localhost:3000');
    expect(proxiedOrigin('https://evil.example', 5181, api)).toBe('https://evil.example');
    expect(proxiedOrigin('http://localhost:51810', 5181, api)).toBe('http://localhost:51810');
  });
});
