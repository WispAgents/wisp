// A CLI sign-in's steps (v1 agent-login.js loginPanel): the page to open, the code, the paste box, in the kit's
// SettingsSignIn box. Settings → Agents and the harness picker's sign-in (SignInHere) show the same.
import { Show, type JSX } from 'solid-js';
import type { PeerId, PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { SettingsSignIn } from '@wisp/ui';
import { loginInput } from '../commands.ts';
import { t } from '../i18n.ts';
import type { LoginRow } from '../types.ts';
import { loginStep, signInWith } from '../words.ts';
import '../i18n/lazy.ts';

const quiet = (p: Promise<unknown>): void => {
  p.catch(() => {}); // the shell's onError shows the rollback toast
};

/** A sign-in under way: the page to open, the code, the paste box. `peer`: the computer the sign-in runs on (a cloud
 *  computer's agents, signed in to from here); this wisp's by default. `inset`: under an account's row. */
export function Steps(props: {
  readonly c: PluginClient;
  readonly agent: string;
  readonly account: string;
  readonly login: LoginRow | undefined;
  readonly peer?: PeerId;
  readonly inset?: boolean;
}): JSX.Element {
  const shell = useShell();
  const step = () => loginStep(props.agent, props.login);
  const peer = props.peer ?? props.c.home;
  const s = () => step() as { kind: string; url?: string; code?: string; paste?: boolean; checking?: boolean; error?: string };
  return (
    <Show when={step().kind !== 'none'}>
      <SettingsSignIn
        part="accounts.login.steps"
        inset={props.inset === true}
        url={s().url ?? ''}
        code={s().code}
        paste={s().paste === true}
        checking={s().checking === true}
        error={step().kind === 'failed' ? s().error : undefined}
        labels={{
          starting: t('accounts.login.starting'),
          openStep: ((who) => (who ? t('accounts.login.openStepWith', { brand: who }) : t('accounts.login.openStep')))(signInWith(props.agent)),
          open: t('accounts.login.openPage'),
          codeStep: t('accounts.login.codeStep'),
          copy: t('accounts.login.copy'),
          copied: t('accounts.login.copied'),
          waiting: t('accounts.login.waiting'),
          pasteStep: t('accounts.login.pasteStep'),
          paste: t('accounts.login.paste'),
          send: t('accounts.login.continue'),
          checking: t('accounts.login.checking'),
        }}
        onOpen={(url) => shell.platform.openExternal(url)}
        onCopy={(code) => navigator.clipboard?.writeText(code) ?? Promise.reject(new Error('no clipboard'))}
        onSend={(code) => quiet(props.c.run(loginInput, { peer, agent: props.agent, account: props.account, code }))}
      />
    </Show>
  );
}
