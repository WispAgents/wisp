// What My Stuff's page opens over itself (v1 openItem, launchApp, appFrame): the media viewer, the reader, an app's
// window; and Edit details… One at a time; Escape, × or a click beside it closes it.
import { Show, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { AppWindow, Button, Dialog, DocReader, docStats, Field, Inline, Input, MediaViewer, Stack, TextArea } from '@wisp/ui';
import { bare, behind } from './model.ts';
import type { Entry } from './items.ts';
import type { Shown } from './page-run.ts';
import type { ActionId } from './actions.ts';
import { previewSandbox } from './preview/spec.ts';
import './i18n/lazy.ts';

export interface WindowsProps {
  readonly client: PluginClient;
  readonly shown: Shown | undefined;
  readonly close: () => void;
  /** the item as it is now (Publish's label follows it) */
  readonly live: (key: string) => Entry | undefined;
  readonly run: (e: Entry, id: ActionId) => void;
  readonly publishTo: () => string | undefined;
}

export function Windows(props: WindowsProps): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const media = () => (props.shown?.kind === 'media' ? props.shown : undefined);
  const doc = () => (props.shown?.kind === 'doc' ? props.shown : undefined);
  const app = () => (props.shown?.kind === 'app' ? props.shown : undefined);
  // Publish, Update link or Copy link, kept current while the window is open (v1 paintPub)
  const pub = (e: Entry) => {
    const now = props.live(e.key) ?? e;
    const it = now.it;
    if (it.kind !== 'app' || (!it.published && !props.publishTo())) return undefined;
    const [label, title] = it.publishing
      ? [t('stuff.app.publishing'), t('stuff.app.publishingTitle')]
      : !it.published
        ? [t('stuff.app.publish'), t('stuff.app.publishTitle', { host: props.publishTo() ?? '' })]
        : behind(it)
          ? [t('stuff.app.update'), t('stuff.app.updateTitle')]
          : [t('stuff.app.copyLink'), t('stuff.app.copyLinkTitle', { at: bare(it.published.url) })];
    return { label, title, icon: 'web' as const, disabled: !!it.publishing, onClick: () => props.run(now, it.published && !behind(it) ? 'pubcopy' : 'publish') };
  };
  return (
    <>
      <Show when={media()}>{(m) => <MediaViewer list={m().list} at={m().at} onClose={() => props.close()} />}</Show>
      <Show when={doc()}>
        {(d) => (
          <DocReader
            open
            onClose={() => props.close()}
            title={d().e.it.title}
            path={d().e.it.name}
            minutes={t('stuff.reader.minutes', { n: docStats(d().text).minutes })}
            markdown={d().text}
            contents={t('stuff.reader.contents')}
            closeLabel={t('stuff.reader.close')}
            copy={{ label: t('stuff.reader.copy'), title: t('stuff.reader.copyTitle'), icon: 'copy', done: t('stuff.reader.copied'), onClick: () => void shell.platform.copy(d().text) }}
          />
        )}
      </Show>
      <Show when={app()}>
        {(a) => (
          <AppWindow
            open
            onClose={() => props.close()}
            title={a().e.it.title}
            version={(a().e.it.rev ?? 1) > 1 ? `v${a().e.it.rev}` : undefined}
            url={a().url}
            src={a().src}
            sandbox={a().url ? `${previewSandbox(a().url ?? '', new URL(document.URL).origin)} allow-modals allow-downloads allow-pointer-lock` : ''}
            starting={t('stuff.app.starting')}
            waiting={t('stuff.app.waiting', { title: a().e.it.title })}
            reload={{ label: t('stuff.app.reload'), title: t('stuff.app.reloadTitle'), icon: 'rerun', onClick: () => {} }}
            tab={shell.platform.phone ? undefined : { label: t('stuff.app.tab'), title: t('stuff.app.tabTitle') }}
            publish={pub(a().e)}
            stop={{
              label: t('stuff.app.stop'),
              title: t('stuff.app.stopTitle'),
              icon: 'stop',
              onClick: () => {
                const e = a().e;
                props.close();
                props.run(props.live(e.key) ?? e, 'stop');
              },
            }}
            closeLabel={t('stuff.app.close')}
          />
        )}
      </Show>
    </>
  );
}

/** Edit details… (v1 editItem): rename, note and tags in one small form. */
export function EditDetails(props: { readonly client: PluginClient; readonly e: Entry | undefined; readonly close: () => void; readonly save: (e: Entry, f: { title: string; note: string; tags: string[] }) => void }): JSX.Element {
  const t = (k: string): string => props.client.t(k);
  let form: HTMLFormElement | undefined;
  const submit = (ev: Event): void => {
    ev.preventDefault();
    const e = props.e;
    if (!e || !form) return;
    const f = new FormData(form);
    const tags = String(f.get('tags') ?? '')
      .split(',')
      .map((x) => x.trim())
      .filter(Boolean);
    props.close();
    props.save(e, { title: String(f.get('title') ?? '').trim() || e.it.title, note: String(f.get('note') ?? ''), tags });
  };
  return (
    <Dialog open={!!props.e} onOpenChange={(o) => !o && props.close()} title={t('stuff.edit.title')} size="sm" part="stuff.edit">
      <Show when={props.e}>
        {(e) => (
          <form ref={(el) => (form = el)} onSubmit={submit}>
            <Stack gap="5">
              <Field label={t('stuff.edit.name')}>
                <Input name="title" value={e().it.title} maxLength={140} autofocus />
              </Field>
              <Field label={t('stuff.edit.note')}>
                <TextArea name="note" value={e().it.note ?? ''} minRows={3} maxLength={2000} placeholder={t('stuff.edit.notePlaceholder')} />
              </Field>
              <Field label={t('stuff.edit.tags')}>
                <Input name="tags" value={(e().it.tags ?? []).join(', ')} placeholder={t('stuff.edit.tagsPlaceholder')} />
              </Field>
              <Inline gap="3" justify="end">
                <Button variant="ghost" size="sm" onClick={() => props.close()}>
                  {t('stuff.edit.cancel')}
                </Button>
                <Button variant="primary" size="sm" type="submit">
                  {t('stuff.edit.save')}
                </Button>
              </Inline>
            </Stack>
          </form>
        )}
      </Show>
    </Dialog>
  );
}
