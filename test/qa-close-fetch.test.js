// qa/close-fetch.mjs: the install gate's requests to a wisp under test. A response whose body the
// caller never reads must not take the process down when the server closes the connection
// (undici on Node 22.23.4 asserted !this.paused and crashed the gate, release run 44085115543).

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import http from 'node:http';

const client = (port) => `
  const { closeFetch } = await import(${JSON.stringify(new URL('../qa/close-fetch.mjs', import.meta.url).href)});
  for (let i = 0; i < 5; i++) {
    const r = await closeFetch('http://127.0.0.1:${port}/');
    if (!r.ok) process.exit(2);
    await new Promise((ok) => setTimeout(ok, 50));
  }
  const r = await closeFetch('http://127.0.0.1:${port}/');
  if ((await r.text()).length !== 65827) process.exit(3);
  const none = await closeFetch('http://127.0.0.1:${port}/none');
  if (none.status !== 204 || (await none.text()) !== '') process.exit(4);
`;

test('a page whose body is never read, on a closed connection, leaves the process standing', async () => {
  const page = Buffer.alloc(65827, 'a'); // the size of wisp's index.html, which crashed it
  const srv = http.createServer((req, res) => {
    if (req.url === '/none') return res.writeHead(204).end();
    res.writeHead(200, { 'content-type': 'text/html', 'content-length': page.length }).end(page);
  });
  await new Promise((ok) => srv.listen(0, '127.0.0.1', ok));
  try {
    const child = spawn(process.execPath, ['--input-type=module', '-e', client(srv.address().port)], { stdio: ['ignore', 'ignore', 'pipe'] });
    let err = '';
    child.stderr.on('data', (d) => { err += d; });
    const code = await new Promise((ok) => child.once('exit', ok));
    assert.equal(code, 0, err.slice(-600));
  } finally {
    srv.close();
  }
});
