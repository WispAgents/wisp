import { expect, test } from 'vitest';
import { filesOf, named, upload, useAttachments } from '../src/frame/attach.ts';
import type { ShellContext } from '../src/contract.ts';

test('pasted_and_dropped_files_become_chips_and_go_up_to_the_tasks_computer_as_its_attachments', async () => {
  const a = new File(['x'], 'notes.md', { type: 'text/markdown' });
  const shot = new File(['png'], 'image.png', { type: 'image/png' });
  // a drop's files; a paste's image comes as an item in some browsers
  expect(filesOf({ files: [a], items: [] } as unknown as DataTransfer)).toEqual([a]);
  expect(filesOf({ files: [], items: [{ kind: 'string', getAsFile: () => null }, { kind: 'file', getAsFile: () => shot }] } as unknown as DataTransfer)).toEqual([shot]);
  expect(filesOf({ files: [], items: [{ kind: 'string', getAsFile: () => null }] } as unknown as DataTransfer)).toEqual([]);
  expect(filesOf(null)).toEqual([]);
  // a pasted screenshot is named for when it came (v1); a real name stays
  expect(named(shot, Date.UTC(2026, 10, 11, 9, 30)).name).toBe('pasted-2026-11-11T09-30-00-000Z.png');
  expect(named(a, 0)).toBe(a);
  // up through the client's route call to the task's peer, each answer an attachment
  const sent: { peer: string; path: string; body: unknown }[] = [];
  const shell = {
    client: {
      plugin: () => ({
        http: (peer: string) => ({
          send: (r: { path: string; body: unknown }) => {
            sent.push({ peer, path: r.path, body: r.body });
            return Promise.resolve({ id: 'u1', name: 'notes.md', mime: 'text/markdown', size: 1, path: '/x' });
          },
        }),
      }),
    },
  } as unknown as ShellContext;
  expect(await upload(shell, 'studio' as never, [a])).toEqual([{ id: 'u1', name: 'notes.md', mime: 'text/markdown', size: 1 }]);
  expect(sent).toEqual([{ peer: 'studio', path: '/uploads?name=notes.md&type=text%2Fmarkdown', body: a }]);
});

test('take_sends_a_file_again_to_the_launch_target_when_it_changed_after_the_file_was_picked', async () => {
  const sent: string[] = [];
  const shell = {
    toast: { error: () => undefined },
    t: (k: string) => k,
    client: {
      plugin: () => ({
        http: (peer: string) => ({
          send: () => {
            sent.push(peer);
            return Promise.resolve({ id: `${peer}-u${sent.length}`, name: 'notes.md', mime: 'text/markdown', size: 1 });
          },
        }),
      }),
    },
  } as unknown as ShellContext;
  const a = new File(['x'], 'notes.md', { type: 'text/markdown' });
  // picked while New task pointed home, then Run on moved it to studio: studio can't resolve home's upload id
  let target = 'home';
  const moved = useAttachments(shell, () => target as never);
  moved.add([a]);
  target = 'studio';
  expect(await moved.take()).toEqual([{ id: 'studio-u2', name: 'notes.md', mime: 'text/markdown', size: 1 }]);
  expect(sent).toEqual(['home', 'studio']);
  // the target unchanged: what went up is what's sent, no second upload
  sent.length = 0;
  const stayed = useAttachments(shell, () => 'home' as never);
  stayed.add([a]);
  expect(await stayed.take()).toEqual([{ id: 'home-u1', name: 'notes.md', mime: 'text/markdown', size: 1 }]);
  expect(sent).toEqual(['home']);
});
