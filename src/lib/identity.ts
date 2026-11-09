import type { Identity } from '../types'

export const COLORS = ['blue', 'green', 'amber', 'red', 'purple', 'teal', 'pink', 'gray']

/**
 * Tag shown when the user left it empty: the first Chinese word (first two characters if
 * longer than three), else an all-caps word like "EMU", else the first word.
 */
export function autoShort(name: string): string {
  const s = name.trim()
  if (!s) return '身分'
  const first = s.split(/\s+/)[0]
  if (/^[㐀-鿿]/.test(s)) {
    const cjk = (first.match(/[㐀-鿿]/g) ?? []).join('')
    return cjk.length <= 3 ? cjk : cjk.slice(0, 2)
  }
  const caps = s.split(/\s+/).find((w) => /^[A-Z0-9]{2,5}$/.test(w))
  return caps ?? first.slice(0, 8)
}

export const tagOf = (id: Identity) => id.short.trim() || autoShort(id.name)

/** git@github.com:org/x.git, https://github.com/org/x, ssh://git@host/org/x -> host/org/x(.git) */
export function normUrl(u: string | null | undefined): string {
  return (u ?? '')
    .trim()
    .replace(/^ssh:\/\/(?:[^@/]+@)?/, '')
    .replace(/^git@([^:]+):/, '$1/')
    .replace(/^https?:\/\/(?:[^@/]+@)?/, '')
}

// Tabs re-check their identity on every refresh; compile each rule's pattern once.
const reCache = new Map<string, RegExp>()
const globRe = (g: string) => {
  let re = reCache.get(g)
  if (!re) reCache.set(g, (re = new RegExp('^' + g.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*+/g, '.*') + '$', 'i')))
  return re
}

const normDir = (p: string) => p.replace(/\\/g, '/').replace(/\/?$/, '/').toLowerCase()

/** Which identity the rules give a repo (pins are handled by the caller). */
export function matchIdentity(ids: Identity[], repoPath: string, remoteUrl: string | null): Identity | null {
  const url = normUrl(remoteUrl)
  const dir = normDir(repoPath)
  // Same precedence as git: includeIf lines are written in identity order, so the last match wins.
  let hit: Identity | null = null
  for (const id of ids)
    for (const r of id.rules) {
      const v = r.v.trim()
      if (!v) continue
      if (r.t === 'remote' && url && (globRe(normUrl(v)).test(url) || globRe(normUrl(v)).test(url.replace(/\.git$/, '')))) hit = id
      if (r.t === 'dir' && dir.startsWith(normDir(v))) hit = id
    }
  return hit
}

/** Rule that would match this remote's owner, e.g. github.com/acme/*. Null for a local-path remote. */
export function ruleForUrl(remoteUrl: string): string | null {
  if (/^([a-z]:[\\/]|[\\/.~]|file:)/i.test(remoteUrl.trim())) return null
  const u = normUrl(remoteUrl).replace(/\.git$/, '')
  const parts = u.split('/')
  if (!parts[0].includes('.')) return null
  return parts.length >= 3 ? `${parts[0]}/${parts[1]}/*` : u
}

export interface Found {
  name: string
  email: string
  keys: { path: string; comment: string; hosts: [string, string][] }[]
  includes: { condition: string; path: string; name: string; email: string; key: string }[]
}

const normPath = (p: string) => p.replace(/\\/g, '/').toLowerCase()

/** includeIf condition -> our rule, when it is one we understand. */
export function ruleFromCondition(c: string): Identity['rules'][number] | null {
  const dir = c.match(/^gitdir(?:\/i)?:(.+)$/)
  if (dir) return { t: 'dir', v: dir[1].replace(/\/$/, '').replace(/\//g, '\\') }
  const url = c.match(/^hasconfig:remote\.\*\.url:(.+)$/)
  if (url) return { t: 'remote', v: normUrl(url[1]).replace(/\*\*$/, '*') }
  return null
}

/**
 * Identities for what is already on this computer: one per existing includeIf file, then one
 * per SSH key no include uses. The default key (id_ed25519 / id_rsa) becomes the fallback.
 */
export function fromFound(f: Found, existing: Identity[] = []): { ids: Identity[]; def: string | null } {
  const ids: Identity[] = []
  const add = (partial: Partial<Identity>) => {
    const id = { ...newIdentity([...existing, ...ids]), rules: [], ...partial }
    ids.push(id)
    return id
  }
  const byFile = new Map<string, Identity>()
  for (const inc of f.includes) {
    const rule = ruleFromCondition(inc.condition)
    let id = byFile.get(normPath(inc.path))
    if (!id) {
      const base = inc.path.split(/[\\/]/).pop()!.replace(/\.(git)?config$|^\.gitconfig[-_.]?/g, '') || inc.email.split('@')[1] || '身分'
      id = add({ name: base, user: inc.name || f.name, email: inc.email || f.email, key: inc.key })
      byFile.set(normPath(inc.path), id)
    }
    if (rule) id.rules.push(rule)
  }
  const used = new Set([...existing, ...ids].map((i) => normPath(i.key)).filter(Boolean))
  let def: string | null = null
  for (const k of f.keys) {
    if (used.has(normPath(k.path))) continue
    const file = k.path.split(/[\\/]/).pop()!
    const isDefault = /^id_(ed25519|rsa|ecdsa)$/.test(file)
    const alias = k.hosts.find(([h, n]) => h !== n)?.[0]
    const name = isDefault ? '預設 key' : alias ?? file.replace(/^id_(ed25519|rsa|ecdsa)_?/, '') ?? file
    const id = add({
      name: name || file,
      user: f.name,
      // Key comments are often user@hostname, which is not an email; need a dotted domain.
      email: /^[^@\s]+@[^@\s]+\.[a-z]{2,}$/i.test(k.comment) ? k.comment : f.email,
      key: k.path,
      host: k.hosts[0]?.[1] ?? 'github.com',
      rules: [{ t: 'remote', v: '' }],
    })
    if (isDefault && !def) def = id.id
  }
  return { ids, def: def ?? ids.find((i) => !i.rules.some((r) => r.v))?.id ?? null }
}

export function newIdentity(existing: Identity[]): Identity {
  const used = existing.map((x) => x.color)
  return {
    id: 'id' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6),
    name: '新身分 ' + (existing.length + 1),
    short: '',
    color: COLORS.find((c) => !used.includes(c)) ?? 'gray',
    key: '',
    account: '',
    verified: false,
    user: '',
    email: '',
    host: 'github.com',
    rules: [{ t: 'remote', v: '' }],
  }
}
