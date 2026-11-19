// SettingsSignIn: a CLI sign-in under way (v1 agent-login.js loginPanel, .sx-login): a box under the row it belongs
// to, its steps numbered. 1. Open the sign-in page (a button, never the address spelled out: an OAuth link is
// hundreds of characters). 2. The code to enter there, with Copy, and "Waiting for you to finish there…"; or the box
// to paste the code the page shows back, then "Checking the code…". Before the page is known: "Starting the
// sign-in…". Under an account in a SettingsInset it takes the inset's look (v1 .sx-accts .sx-login).
import { createSignal, Show, type JSX } from 'solid-js';
import { createTimer } from '../blocks/content-scope.ts';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { Button, IconButton } from '../controls/Button.tsx';
import { Input } from '../controls/Input.tsx';
import { Spinner } from '../controls/Spinner.tsx';
import '../css/settings.css';

export interface SettingsSignInLabels {
  /** "Starting the sign-in…" */
  readonly starting: string;
  /** "Open the sign-in page and sign in with your Claude account." */
  readonly openStep: string;
  /** "Open the sign-in page" */
  readonly open: string;
  /** "Enter this code there:" */
  readonly codeStep: string;
  /** "Copy the code" */
  readonly copy: string;
  /** "Copied": the copy button's name while it shows its check (audit D1); without it, `copy` */
  readonly copied?: string;
  /** "Waiting for you to finish there…" */
  readonly waiting: string;
  /** "Paste the code it shows you:" */
  readonly pasteStep: string;
  /** the paste box's placeholder */
  readonly paste: string;
  /** "Continue" */
  readonly send: string;
  /** "Checking the code…" */
  readonly checking: string;
}

export interface SettingsSignInProps extends KitProps {
  /** the sign-in page; empty while the CLI hasn't printed it yet */
  readonly url: string;
  /** a code to enter on the page (a device sign-in) */
  readonly code?: string | undefined;
  /** the page shows a code to paste back here */
  readonly paste?: boolean;
  /** a pasted code is being checked */
  readonly checking?: boolean;
  /** it failed: the reason, shown instead of the steps */
  readonly error?: string | undefined;
  /** under an account's row in a SettingsInset */
  readonly inset?: boolean;
  readonly labels: SettingsSignInLabels;
  readonly onSend: (code: string) => void;
  /** copies the code; a promise that rejects (no clipboard) turns the check back at once */
  readonly onCopy: (code: string) => void | Promise<unknown>;
  /** opens the page where the platform does it (the desktop app's browser); without it, the link's own new tab */
  readonly onOpen?: (url: string) => void;
}

/** How long Copy shows its check (audit D1: the feedback, as a toast is for errors only). */
const COPIED_MS = 1500;

export function SettingsSignIn(props: SettingsSignInProps): JSX.Element {
  const part = (): PartName => props.part ?? 'settings.signin';
  const [draft, setDraft] = createSignal('');
  const [copied, setCopied] = createSignal(false);
  const timer = createTimer();
  const copy = (code: string): void => {
    setCopied(true);
    timer.after(COPIED_MS, () => setCopied(false));
    const done = props.onCopy(code);
    if (done) {
      done.catch(() => {
        timer.clear();
        setCopied(false);
      });
    }
  };
  const send = (e: SubmitEvent): void => {
    e.preventDefault();
    const c = draft().trim();
    if (c) props.onSend(c);
  };
  const wait = (label: string, p: string): JSX.Element => (
    <p data-kit="settings-signin-wait" data-part={sub(part(), p)}>
      <Spinner label={label} />
      {label}
    </p>
  );
  return (
    <div id={props.id} data-kit="settings-signin" data-part={part()} data-inset={flag(props.inset)} data-bad={flag(!!props.error)}>
      <Show
        when={!props.error}
        fallback={
          <p role="alert" data-part={sub(part(), 'error')}>
            {props.error}
          </p>
        }
      >
        <Show when={props.url} fallback={wait(props.labels.starting, 'starting')}>
          <p>
            <b>1.</b> {props.labels.openStep}
          </p>
          <div data-kit="settings-signin-row">
            <a data-kit="button" data-variant="primary" data-size="sm" data-part={sub(part(), 'open')} href={props.url}
              target="_blank"
              rel="noopener noreferrer"
              onClick={(e) => {
                if (!props.onOpen) return;
                e.preventDefault();
                props.onOpen(props.url);
              }}
            >
              <span data-kit="button-content">
                <Icon name="external" size="base" />
                <span data-kit="button-label" data-text="control">
                  {props.labels.open}
                </span>
              </span>
            </a>
          </div>
          <Show when={props.code}>
            {(c) => (
              <>
                <p>
                  <b>2.</b> {props.labels.codeStep}
                </p>
                <div data-kit="settings-signin-code" data-part={sub(part(), 'code')}>
                  <span data-kit="settings-signin-code-text">{c()}</span>
                  <IconButton
                    icon={copied() ? 'check' : 'copy'}
                    size="sm"
                    variant="ghost"
                    tone={copied() ? 'good' : undefined}
                    label={copied() ? (props.labels.copied ?? props.labels.copy) : props.labels.copy}
                    part={sub(part(), 'copy')}
                    onClick={() => copy(c())}
                  />
                </div>
                {wait(props.labels.waiting, 'waiting')}
              </>
            )}
          </Show>
          <Show when={!props.code && (props.paste || props.checking)}>
            <p>
              <b>2.</b> {props.labels.pasteStep}
            </p>
          </Show>
          <Show when={props.checking}>{wait(props.labels.checking, 'checking')}</Show>
          <Show when={!props.code && props.paste && !props.checking}>
            <form data-kit="settings-signin-row" data-layout="wrap" data-part={sub(part(), 'paste')} onSubmit={send}>
              <Input size="sm" mono placeholder={props.labels.paste} aria-label={props.labels.paste} value={draft()} onInput={setDraft} />
              <Button size="sm" variant="primary" type="submit" disabled={draft().trim() === ''}>
                {props.labels.send}
              </Button>
            </form>
          </Show>
        </Show>
      </Show>
    </div>
  );
}
