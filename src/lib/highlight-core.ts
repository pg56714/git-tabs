// Syntax colors for diffs via shiki (JS regex engine, languages loaded on demand).
// Runs inside highlight.worker.ts so tokenizing a big file never freezes the UI.

import type { HighlighterCore, ThemedToken } from 'shiki/core'
import type { Hunk } from '../types'

const LANGS: Record<string, () => Promise<unknown>> = {
  python: () => import('shiki/langs/python.mjs'),
  rust: () => import('shiki/langs/rust.mjs'),
  typescript: () => import('shiki/langs/typescript.mjs'),
  tsx: () => import('shiki/langs/tsx.mjs'),
  javascript: () => import('shiki/langs/javascript.mjs'),
  jsx: () => import('shiki/langs/jsx.mjs'),
  vue: () => import('shiki/langs/vue.mjs'),
  json: () => import('shiki/langs/json.mjs'),
  toml: () => import('shiki/langs/toml.mjs'),
  yaml: () => import('shiki/langs/yaml.mjs'),
  markdown: () => import('shiki/langs/markdown.mjs'),
  shellscript: () => import('shiki/langs/shellscript.mjs'),
  powershell: () => import('shiki/langs/powershell.mjs'),
  bat: () => import('shiki/langs/bat.mjs'),
  go: () => import('shiki/langs/go.mjs'),
  java: () => import('shiki/langs/java.mjs'),
  kotlin: () => import('shiki/langs/kotlin.mjs'),
  c: () => import('shiki/langs/c.mjs'),
  cpp: () => import('shiki/langs/cpp.mjs'),
  csharp: () => import('shiki/langs/csharp.mjs'),
  css: () => import('shiki/langs/css.mjs'),
  scss: () => import('shiki/langs/scss.mjs'),
  html: () => import('shiki/langs/html.mjs'),
  xml: () => import('shiki/langs/xml.mjs'),
  sql: () => import('shiki/langs/sql.mjs'),
  dockerfile: () => import('shiki/langs/dockerfile.mjs'),
  ini: () => import('shiki/langs/ini.mjs'),
  lua: () => import('shiki/langs/lua.mjs'),
  php: () => import('shiki/langs/php.mjs'),
  ruby: () => import('shiki/langs/ruby.mjs'),
  swift: () => import('shiki/langs/swift.mjs'),
  make: () => import('shiki/langs/make.mjs'),
}

const EXT: Record<string, string> = {
  py: 'python', pyi: 'python', rs: 'rust', ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'tsx',
  js: 'javascript', mjs: 'javascript', cjs: 'javascript', jsx: 'jsx', vue: 'vue', json: 'json', jsonc: 'json',
  toml: 'toml', yml: 'yaml', yaml: 'yaml', md: 'markdown', sh: 'shellscript', bash: 'shellscript', zsh: 'shellscript',
  ps1: 'powershell', psm1: 'powershell', bat: 'bat', cmd: 'bat', go: 'go', java: 'java', kt: 'kotlin', kts: 'kotlin',
  c: 'c', h: 'c', cc: 'cpp', cpp: 'cpp', cxx: 'cpp', hpp: 'cpp', cs: 'csharp', css: 'css', scss: 'scss', html: 'html',
  htm: 'html', xml: 'xml', svg: 'xml', sql: 'sql', ini: 'ini', cfg: 'ini', lua: 'lua', php: 'php', rb: 'ruby', swift: 'swift',
}

export function langOf(path: string): string | null {
  const name = path.split('/').pop()!.toLowerCase()
  if (name === 'dockerfile') return 'dockerfile'
  if (name === 'makefile') return 'make'
  const ext = name.includes('.') ? name.split('.').pop()! : ''
  return EXT[ext] ?? null
}

let hl: Promise<HighlighterCore> | null = null
const loaded = new Set<string>()

function highlighter(): Promise<HighlighterCore> {
  hl ??= (async () => {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] = await Promise.all([import('shiki/core'), import('shiki/engine/javascript')])
    return createHighlighterCore({
      themes: [import('shiki/themes/github-light.mjs'), import('shiki/themes/github-dark.mjs')],
      langs: [],
      engine: createJavaScriptRegexEngine({ forgiving: true }),
    })
  })()
  return hl
}

export interface Tok {
  text: string
  light?: string
  dark?: string
}

/**
 * Tokens per line of each hunk. Old and new sides are highlighted as separate blocks so
 * multi-line strings and comments inside a hunk still color correctly.
 * Result: tokens[hunk][line] (null when the language is unknown).
 */
export async function tokenizeHunks(path: string, hunks: Hunk[]): Promise<Tok[][][] | null> {
  const lang = langOf(path)
  if (!lang || !LANGS[lang]) return null
  const h = await highlighter()
  if (!loaded.has(lang)) {
    const mod = (await LANGS[lang]()) as { default: Parameters<HighlighterCore['loadLanguage']>[0] }
    await h.loadLanguage(mod.default)
    loaded.add(lang)
  }
  const tok = (code: string) =>
    h.codeToTokens(code, { lang, themes: { light: 'github-light', dark: 'github-dark' }, defaultColor: false }).tokens.map((line: ThemedToken[]) =>
      line.map((t) => ({ text: t.content, light: t.htmlStyle?.['--shiki-light'], dark: t.htmlStyle?.['--shiki-dark'] })),
    )
  return hunks.map((hk) => {
    const oldIdx: number[] = []
    const newIdx: number[] = []
    hk.lines.forEach((l, i) => {
      if (l.t !== '+') oldIdx.push(i)
      if (l.t !== '-') newIdx.push(i)
    })
    const out: Tok[][] = hk.lines.map((l) => [{ text: l.text }])
    const oldT = tok(oldIdx.map((i) => hk.lines[i].text).join('\n'))
    const newT = tok(newIdx.map((i) => hk.lines[i].text).join('\n'))
    newIdx.forEach((li, k) => (out[li] = newT[k] ?? out[li]))
    oldIdx.forEach((li, k) => {
      if (hk.lines[li].t === '-') out[li] = oldT[k] ?? out[li]
    })
    return out
  })
}

