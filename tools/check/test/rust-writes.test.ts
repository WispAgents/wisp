// rust-swallowed-write (rules/rust-swallowed-write.ts): a store write never drops its error.
import assert from 'node:assert/strict';
import test from 'node:test';
import { rustSwallowedWrite } from '../rules/rust.ts';
import { messages, run, writeTree } from './helpers.ts';

test('rust-swallowed-write: `let _ =` or `.ok();` on a store write fails unless a `// write: best-effort <why>` says why; handled writes, stdin writes and tests do not', async () => {
  const root = writeTree(
    new Map([
      ['Cargo.toml', '[workspace]\nmembers = ["crates/*", "plugins/*/host"]\n'],
      ['plugins/till/host/Cargo.toml', '[package]\nname = "wisp-till"\n'],
      [
        'plugins/till/host/src/lib.rs',
        [
          'async fn a(st: &St, ctx: &Ctx) -> Result<()> {',
          '    let _ = st.store.write(move |w| store::put(w, &a)).await;', // 2: the finding
          '    let _ = store', // 3: spans lines: the finding
          '        .write(move |w| {',
          '            w.upsert(K, r).map(|_| ())',
          '        })',
          '        .await;',
          '    st.store.write(move |w| w.upsert(K, r)).await.map_err(store_err)?;', // propagated: none
          '    if let Err(e) = st.store.write(|w| put(w)).await { tracing::warn!("{e}"); }', // logged: none
          "    let _ = handle.write(bytes);", // a child's stdin, not the store: none
          '    // write: best-effort an empty write as a barrier',
          '    let _ = store.write(|_| Ok(())).await;', // a reason above: none
          '    let _ = ctx.run::<wisp_runtime_api::SetExt>(&call, args).await;', // 13: a writing command: the finding
          '    let _ = ctx.run::<wisp_telemetry_api::Track>(&call, args).await;', // not a writing name: none
          '    store.write(move |w| w.upsert(K, r)).await.ok();', // 15: .ok() drops it: the finding
          '    let saved = store.write(move |w| w.upsert(K, r)).await.ok();', // kept as a value: none
          '    // write: best-effort',
          '    let _ = store.write(|w| put(w)).await;', // 18: no reason given: the finding
          '    let _ = store.write(|w| put(w)).await; // write: best-effort the row is rebuilt at boot', // a reason on its line: none
          '    // let _ = store.write(|w| put(w)).await;', // a comment: none
          '    let _ = st.store.write(move |w| {', // 21: the reason inside a string is not a comment: the finding
          '        w.upsert(K, "// write: best-effort no")',
          '    }).await;',
          '    saved = store.write(move |w| w.upsert(K, r)).await.ok();', // assigned: none
          '    // write: best-effort the pane rebuilds it,',
          '    // and nothing reads it back',
          '    let _ = store.write(|w| put(w)).await;', // a reason in the comment block above: none
          '    return store.write(move |w| w.upsert(K, r)).await.ok();', // returned: none
          '}',
          '#[cfg(test)]',
          'mod tests { fn t() { let _ = store.write(|w| put(w)); } }', // tests: none
        ].join('\n'),
      ],
      ['plugins/till/host/tests/x.rs', 'fn t() { let _ = store.write(|w| put(w)); }'], // a test file: none
      ['crates/wisp-thing/Cargo.toml', '[package]\nname = "wisp-thing"\n'],
      ['crates/wisp-thing/src/lib.rs', 'fn f() {\n    let _ = db.upsert(K, r);\n}'], // 2: base crates too: the finding
    ]),
  );
  assert.deepEqual(await run(rustSwallowedWrite, root), [
    'crates/wisp-thing/src/lib.rs:2 rust-swallowed-write',
    'plugins/till/host/src/lib.rs:13 rust-swallowed-write',
    'plugins/till/host/src/lib.rs:15 rust-swallowed-write',
    'plugins/till/host/src/lib.rs:18 rust-swallowed-write',
    'plugins/till/host/src/lib.rs:2 rust-swallowed-write',
    'plugins/till/host/src/lib.rs:21 rust-swallowed-write',
    'plugins/till/host/src/lib.rs:3 rust-swallowed-write',
  ]);
  const [m] = await messages(rustSwallowedWrite, root);
  assert.ok(m?.includes('best-effort'), m);
});
