export function ago(unix: number): string {
  if (!unix) return ''
  const s = Math.max(0, Date.now() / 1000 - unix)
  if (s < 60) return '剛剛'
  if (s < 3600) return `${Math.floor(s / 60)} 分鐘前`
  if (s < 86400) return `${Math.floor(s / 3600)} 小時前`
  if (s < 86400 * 7) return `${Math.floor(s / 86400)} 天前`
  return dateTime(unix)
}

export function dateTime(unix: number): string {
  const d = new Date(unix * 1000)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`
}

export const short = (sha: string) => sha.slice(0, 7)

export const baseName = (p: string) => p.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? p

/** Display letter + css class for a file's change. */
export function statusLetter(s: string): string {
  if (s === '?') return 'A'
  return s[0] === '.' ? 'M' : s[0]
}

/** Every word of the query appears somewhere in the text, in any order. */
export function fuzzy(query: string, text: string): boolean {
  const t = text.toLowerCase()
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => t.includes(w))
}
