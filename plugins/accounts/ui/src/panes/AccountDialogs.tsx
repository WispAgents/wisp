// Settings → Agents' account boxes (v1 web/js/features/accounts.js addBox, the rename form, the Remove ask): Add another
// account asks a name, Personal or Work, and shows the notice the providers' terms call for before its Sign in
// (design/accounts.md *(legal)*); Rename; Remove asks first, since the login is lost. Kit only.
import { createSignal, type JSX } from 'solid-js';
import { Button, Dialog, Inline, Input, Stack, Tabs, Text } from '@wisp/ui';
import { t } from '../i18n.ts';
import type { AccountKind } from '../types.ts';
import '../i18n/lazy.ts';

/** Add another account: its name (blank: wisp names it), Personal or Work, the notice, then Sign in. */
export function AddAccount(props: { readonly brand: string; readonly onAdd: (label: string, kind: AccountKind) => void; readonly onClose: () => void }): JSX.Element {
  const [label, setLabel] = createSignal('');
  const [kind, setKind] = createSignal<AccountKind>('personal');
  let input: HTMLInputElement | undefined;
  const go = (): void => {
    props.onAdd(label().trim(), kind());
    props.onClose();
  };
  return (
    <Dialog
      open
      size="sm"
      onOpenChange={(o) => !o && props.onClose()}
      title={t('accounts.account.add', { brand: props.brand })}
      part="accounts.add.dialog"
      initialFocus={() => input ?? null}
      footer={
        <Inline gap="3" justify="end">
          <Button variant="ghost" size="sm" onClick={() => props.onClose()}>
            {t('accounts.cancel')}
          </Button>
          <Button variant="primary" size="sm" icon="lock" part="accounts.add.signIn" onClick={go}>
            {t('accounts.login.signIn')}
          </Button>
        </Inline>
      }
    >
      <Stack gap="4">
        <Input
          ref={(el) => (input = el)}
          size="sm"
          aria-label={t('accounts.account.name')}
          placeholder={t('accounts.account.namePlaceholder')}
          value={label()}
          maxLength={40}
          onInput={setLabel}
          onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), go())}
        />
        <Tabs
          variant="seg"
          part="accounts.add.kind"
          aria-label={t('accounts.account.kind')}
          value={kind()}
          onChange={(v) => setKind(v as AccountKind)}
          items={(['personal', 'work'] as const).map((k) => ({ value: k, label: t(`accounts.kind.${k}`) }))}
        />
        <Text as="p" size="sm">
          <b>{t('accounts.account.noticeTitle')}</b> {t('accounts.account.notice')}
        </Text>
        <Text as="p" size="sm" tone="muted">
          {t('accounts.account.noticeLimits')}
        </Text>
      </Stack>
    </Dialog>
  );
}

/** Rename an account (v1: in place on its row; Enter keeps, Escape leaves it). */
export function RenameAccount(props: { readonly label: string; readonly onRename: (label: string) => void; readonly onClose: () => void }): JSX.Element {
  const [label, setLabel] = createSignal(props.label);
  let input: HTMLInputElement | undefined;
  const go = (): void => {
    const v = label().trim();
    if (v && v !== props.label) props.onRename(v);
    props.onClose();
  };
  return (
    <Dialog
      open
      size="sm"
      onOpenChange={(o) => !o && props.onClose()}
      title={t('accounts.account.renameTitle', { name: props.label })}
      part="accounts.rename"
      initialFocus={() => input ?? null}
      footer={
        <Inline gap="3" justify="end">
          <Button variant="ghost" size="sm" onClick={() => props.onClose()}>
            {t('accounts.cancel')}
          </Button>
          <Button variant="primary" size="sm" disabled={label().trim() === ''} onClick={go}>
            {t('accounts.account.save')}
          </Button>
        </Inline>
      }
    >
      <Input ref={(el) => (input = el)} size="sm" aria-label={t('accounts.account.name')} value={label()} maxLength={40} onInput={setLabel} onKeyDown={(e) => e.key === 'Enter' && (e.preventDefault(), go())} />
    </Dialog>
  );
}

/** Remove asks first (v1): wisp signs it out and forgets it; tasks that ran on it keep their history. */
export function RemoveAccount(props: { readonly label: string; readonly onRemove: () => void; readonly onClose: () => void }): JSX.Element {
  return (
    <Dialog
      open
      size="sm"
      role="alertdialog"
      onOpenChange={(o) => !o && props.onClose()}
      title={t('accounts.account.removeTitle', { name: props.label })}
      part="accounts.remove.confirm"
      footer={
        <Inline gap="3" justify="end">
          <Button variant="ghost" size="sm" onClick={() => props.onClose()}>
            {t('accounts.cancel')}
          </Button>
          <Button
            variant="danger"
            size="sm"
            onClick={() => {
              props.onRemove();
              props.onClose();
            }}
          >
            {t('accounts.account.removeOk')}
          </Button>
        </Inline>
      }
    >
      <Text as="p" tone="muted">
        {t('accounts.account.removeDetail')}
      </Text>
    </Dialog>
  );
}
