// A tiny highlighter for code blocks: comments, strings, numbers, keywords and capitalised type
// names, nothing else. It makes spans (never HTML). Plain text and very large blocks render plain; a language it
// doesn't name gets the C-like rules, as v1's did. A diff or patch is coloured a line at a time, as the diff view is.

export type TokenKind = 'k' | 's' | 'c' | 'n' | 't' | 'add' | 'del' | 'hunk' | 'meta';

export interface Token {
  readonly kind: TokenKind | null;
  readonly text: string;
}

const HASH = ['sh', 'bash', 'zsh', 'shell', 'console', 'py', 'python', 'rb', 'ruby', 'yaml', 'yml', 'toml', 'ini', 'dockerfile', 'make', 'makefile', 'r', 'perl', 'pl', 'nix'];

const KEYWORDS = new Set(
  (
    'abstract as async await break case catch class const continue def default defer del delete do elif else enum export extends ' +
    'false final finally fn for from func function go if impl implements import in instanceof interface is lambda let loop match mod ' +
    'module mut namespace new nil none not null of or and package pass private protected pub public raise readonly return satisfies ' +
    'self static struct super switch this throw true try type typeof undefined unless until use var void when where while with yield ' +
    'echo export local then fi esac done elif True False None'
  ).split(' '),
);

// Alternatives in order: block/line comment, hash comment, string, number, identifier.
const C_RULES = /(\/\/[^\n]*|\/\*[\s\S]*?(?:\*\/|$))|("(?:[^"\\\n]|\\.)*"?|'(?:[^'\\\n]|\\.)*'?|`(?:[^`\\]|\\.)*`?)|(\b\d[\w.]*)|([A-Za-z_$][\w$]*)/g;
const HASH_RULES = /(#[^\n]*)|("(?:[^"\\\n]|\\.)*"?|'(?:[^'\\\n]|\\.)*'?)|(\b\d[\w.]*)|([A-Za-z_$][\w$-]*)/g;

const MAX = 50_000;

/** Plain text: never coloured (v1 md.js highlight's first line). */
const PLAIN = ['text', 'txt', 'plain', 'output', 'log'];

// v1 md.js highlight(): every language but plain text is coloured; one it doesn't know (csv, sql, …) gets the C-like
// rules, where `#` isn't a comment
function family(lang: string): RegExp | null {
  const l = lang.toLowerCase();
  if (!l || PLAIN.includes(l)) return null;
  if (HASH.includes(l)) return HASH_RULES;
  return C_RULES;
}

function kindOf(m: RegExpExecArray): TokenKind | null {
  if (m[1] !== undefined) return 'c';
  if (m[2] !== undefined) return 's';
  if (m[3] !== undefined) return 'n';
  const word = m[4] ?? '';
  if (KEYWORDS.has(word)) return 'k';
  // v1 md.js highlight(): any identifier that starts with a capital is a type or constant (Foo, LIMIT, WINDOW_MS)
  return /^[A-Z][\w$-]*$/.test(word) ? 't' : null;
}

const DIFF = ['diff', 'patch'];

/** A diff's kind of line: `+++`/`---` file headers before `+`/`-`, so a header is never an added or removed line. */
function diffKind(line: string): TokenKind | null {
  if (line.startsWith('+++') || line.startsWith('---') || line.startsWith('diff ') || line.startsWith('index ')) return 'meta';
  if (line.startsWith('@@')) return 'hunk';
  if (line.startsWith('+')) return 'add';
  if (line.startsWith('-')) return 'del';
  return null;
}

/** One token per line (the line ends stay plain), and nothing coloured inside a line: it's the change that matters. */
function diffTokens(code: string): Token[] {
  const out: Token[] = [];
  const lines = code.split('\n');
  lines.forEach((line, i) => {
    const nl = i < lines.length - 1 ? '\n' : '';
    const kind = diffKind(line);
    const last = out[out.length - 1];
    if (kind && line) out.push({ kind, text: line });
    else if (last && last.kind === null) out[out.length - 1] = { kind: null, text: last.text + line };
    else if (line) out.push({ kind: null, text: line });
    if (!nl) return;
    const tail = out[out.length - 1];
    if (tail && tail.kind === null) out[out.length - 1] = { kind: null, text: tail.text + nl };
    else out.push({ kind: null, text: nl });
  });
  return out.length > 0 ? out : [{ kind: null, text: code }];
}

/** Splits `code` into tokens; plain text has `kind: null`. Joining the texts gives `code` back. */
export function highlight(code: string, lang: string | undefined): Token[] {
  if (lang && DIFF.includes(lang.toLowerCase()) && code.length <= MAX) return diffTokens(code);
  const rules = lang ? family(lang) : null;
  if (!rules || code.length > MAX) return [{ kind: null, text: code }];
  const re = new RegExp(rules.source, 'g');
  const out: Token[] = [];
  let at = 0;
  const plain = (text: string): void => {
    if (!text) return;
    const last = out[out.length - 1];
    if (last && last.kind === null) out[out.length - 1] = { kind: null, text: last.text + text };
    else out.push({ kind: null, text });
  };
  for (let m = re.exec(code); m; m = re.exec(code)) {
    if (m[0] === '') {
      re.lastIndex++;
      continue;
    }
    plain(code.slice(at, m.index));
    const kind = kindOf(m);
    if (kind) out.push({ kind, text: m[0] });
    else plain(m[0]);
    at = m.index + m[0].length;
  }
  plain(code.slice(at));
  return out;
}
