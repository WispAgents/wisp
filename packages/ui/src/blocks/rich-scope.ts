// Scoped helpers for the rich blocks: every timer, observer and listener here is made inside the
// component's owner and freed by onCleanup, so a block leaves nothing running once it unmounts.
import { createSignal, onCleanup, onMount, type Accessor } from 'solid-js';

/** A restartable timeout owned by the calling component: `start` replaces a pending one. */
export function useTimeout(): { readonly start: (ms: number, run: () => void) => void; readonly stop: () => void } {
  let id: ReturnType<typeof setTimeout> | undefined;
  const stop = (): void => {
    if (id !== undefined) clearTimeout(id);
    id = undefined;
  };
  onCleanup(stop);
  return {
    start: (ms, run) => {
      stop();
      id = setTimeout(() => {
        id = undefined;
        run();
      }, ms);
    },
    stop,
  };
}

/** One animation frame, owned by the component; a new request replaces a pending one. */
export function useFrame(): (run: () => void) => void {
  let id: number | undefined;
  onCleanup(() => {
    if (id !== undefined) cancelAnimationFrame(id);
  });
  return (run) => {
    if (id !== undefined) cancelAnimationFrame(id);
    id = requestAnimationFrame(() => {
      id = undefined;
      run();
    });
  };
}

/**
 * True once `el` comes within 600 px of the screen (frames load only near it: a long feed of answers
 * shouldn't start them all). Where there is no IntersectionObserver it is true at once.
 */
export function useNear(el: Accessor<Element | undefined>): Accessor<boolean> {
  const [near, setNear] = createSignal(typeof IntersectionObserver === 'undefined');
  let io: IntersectionObserver | undefined;
  let gone = false;
  const watch = (node: Element): void => {
    if (gone || near() || io) return;
    io = new IntersectionObserver((entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      io?.disconnect();
      setNear(true);
    }, { rootMargin: '600px 0px' });
    io.observe(node);
  };
  // After mount, once the ref is set.
  onMount(() => {
    const node = el();
    if (node) watch(node);
  });
  onCleanup(() => {
    gone = true;
    io?.disconnect();
  });
  return near;
}

/** Copy text, with a "copied" flag that clears after a moment. */
export function useCopy(): { readonly copied: Accessor<boolean>; readonly copy: (text: string) => void } {
  const [copied, setCopied] = createSignal(false);
  const timer = useTimeout();
  return {
    copied,
    copy: (text) => {
      const clip = typeof navigator === 'undefined' ? undefined : navigator.clipboard;
      if (!clip) return;
      clip.writeText(text).then(
        () => {
          setCopied(true);
          timer.start(1200, () => setCopied(false));
        },
        () => undefined,
      );
    },
  };
}

/** `run` once the current task is over (its events and their default actions done): a menu's choice handed on after
 *  the press that made it (its click, the menu's focus going back to its button), so a dialog the choice opens isn't
 *  closed by that same press (QA r14 #4). Never cleared: the menu that chose may unmount as it closes, and the choice
 *  must still happen. */
export function nextTask(run: () => void): void {
  setTimeout(run, 0);
}
