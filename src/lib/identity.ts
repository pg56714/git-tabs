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

const globRe = (g: string) => new RegExp('^' + g.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*+/g, '.*') + '$', 'i')

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
