---
title: Your first plugin
description: Write a plugin in any language that adds a command to wisp, then call it from the CLI, the app, the SDKs and MCP.
sidebar:
  order: 2
---

This page builds **Hello**, a plugin with one command, `user.hello.greet`. It's a small Node program, but an exec
plugin can be written in any language that reads and writes lines.

## 1. Start it

```sh
wisp plugin new hello
```

This writes `~/.wisp/plugins/hello/wisp-plugin.json`, a plugin with the id `user.hello` that does nothing yet. A
plugin of yours always has an id that starts with `user.`, and each of its commands starts with that id.

## 2. The manifest

Replace `wisp-plugin.json` with:

```json
{
  "wispPlugin": 1,
  "id": "user.hello",
  "name": "Hello",
  "version": "1.0.0",
  "api": "^1",
  "description": "Says hello.",
  "server": { "exec": ["node", "hello.mjs"] },
  "activation": ["onCommand:user.hello.greet"],
  "permissions": [],
  "contributes": {
    "commands": [{ "id": "user.hello.greet", "title": "Say hello to someone" }]
  }
}
```

- `server.exec` is the program wisp starts, in the plugin's folder.
- `activation` says when to start it: here, on the first call of its command. Until then it costs nothing.
- `permissions` is what it may do. Hello calls nothing, so it asks for nothing. Every key is in the
  [manifest reference](../../reference/plugin-api/).

## 3. The program

wisp and the program talk JSON-RPC 2.0, one message per line, over the program's stdin and stdout. wisp sends
`initialize`. The program registers its commands, then answers. After that, each call is a `command.run`.

```js
// hello.mjs
import readline from 'node:readline';

const commands = {
  'user.hello.greet': async (args) => ({ text: `Hello, ${args?.name || 'you'}!` }),
};

let next = 0;
const waiting = new Map();
const send = (m) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...m }) + '\n');
// a request to wisp, answered by a message with the same id
const request = (method, params) =>
  new Promise((ok, fail) => {
    const id = ++next;
    waiting.set(id, { ok, fail });
    send({ id, method, params });
  });

readline.createInterface({ input: process.stdin }).on('line', async (line) => {
  const m = JSON.parse(line);
  if (!m.method) {
    const w = waiting.get(m.id);
    waiting.delete(m.id);
    return m.error ? w?.fail(m.error) : w?.ok(m.result);
  }
  try {
    let result = null;
    if (m.method === 'initialize') {
      for (const id of Object.keys(commands)) await request('command.register', { id });
      result = { ok: true };
    } else if (m.method === 'command.run') {
      result = await commands[m.params.id](m.params.args);
    }
    if (m.id !== undefined) send({ id: m.id, result });
  } catch (e) {
    if (m.id !== undefined) send({ id: m.id, error: { code: -32000, message: String(e?.message ?? e) } });
  }
});
```

Write anything you want to log to stderr: stdout is only for messages.

The program can ask wisp for more with the same `request`: `run` (call another command, within the plugin's
permissions), `emit` (send one of its events), `on` (hear an event), `settings.get` and `hook` (check a call before it
runs).

## 4. Switch it on and call it

A plugin you install starts off, so you see what it asks for before it runs.

```sh
wisp plugin enable user.hello
wisp user hello greet --name Ada
```

```
text  Hello, Ada!
```

The command is everywhere at once, with no more work:

- `wisp help user` lists it, and `wisp user hello greet --help` shows its flags.
- The SDKs and your agent's MCP tools can call `user.hello.greet`.
- Other plugins can call it, if their manifest asks for `user:hello.greet`.

## 5. Share it

Put the folder in a git repo. Anyone can then install it, off, and switch it on:

```sh
wisp plugin install https://github.com/you/wisp-hello
```

## A plugin with UI

A plugin can also draw: a Settings page, a pane, a card. Its UI is one `ui.js` that runs sandboxed and describes
wisp's own components, so it looks like wisp in light and dark. [Plugin examples](../../reference/plugin-examples/)
has complete ones to copy, and [the manifest reference](../../reference/plugin-api/) lists every component.
