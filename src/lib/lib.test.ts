import { describe, expect, it } from 'vitest'
import { hunkRows, sideRows, wordRange } from './diff'
import { layout } from './graph'
import { treeRows, treeSort } from './tree'
import { autoShort, matchIdentity, normUrl, ruleForUrl } from './identity'
import { splitTokens } from './highlight'
import type { Commit, Identity } from '../types'

describe('diff', () => {
  it('marks only the changed words', () => {
    expect(wordRange('    matches = (', '    match_query = (')).toEqual([
      [9, 11],
      [9, 15],
    ])
    expect(wordRange('abc', 'xyz')).toBeNull()
  })
  it('numbers lines and pairs sides', () => {
    const rows = hunkRows({ header: '', old_start: 10, old_lines: 3, new_start: 10, new_lines: 3, lines: [{ t: ' ', text: 'a' }, { t: '-', text: 'foo(1)' }, { t: '+', text: 'foo(2)' }, { t: ' ', text: 'b' }] })
    expect(rows.map((r) => [r.old, r.new])).toEqual([[10, 10], [11, null], [null, 11], [12, 12]])
    expect(rows[1].word).toEqual([4, 5])
    const side = sideRows(rows)
    expect(side.length).toBe(3)
    expect(side[1][0]?.text).toBe('foo(1)')
    expect(side[1][1]?.text).toBe('foo(2)')
  })
})

const c = (sha: string, parents: string[]): Commit => ({ sha, parents, author: '', email: '', time: 0, refs: [], subject: sha })

describe('graph', () => {
  it('puts a side branch in its own column and merges back', () => {
    // M merges B into A's line; history: M -> (A2, B1), A2 -> A1, B1 -> A1
    const rows = layout([c('M', ['A2', 'B1']), c('A2', ['A1']), c('B1', ['A1']), c('A1', [])])
    expect(rows.map((r) => r.col)).toEqual([0, 0, 1, 0])
    expect(rows[0].segs.some((s) => s.x1 === 0 && s.x2 === 1 && s.y2 === 2)).toBe(true)
    // B1's lane bends back into A1.
    expect(rows[3].segs.some((s) => s.x1 === 1 && s.x2 === 0 && s.y1 === 0)).toBe(true)
    // The newest commit has no line above it.
    expect(rows[0].segs.some((s) => s.y1 === 0)).toBe(false)
  })
})

describe('tree', () => {
  it('sorts folders first and merges single-child folders', () => {
    const files = ['apps/backend/app/services/pricing.py', 'apps/backend/app/models.py', 'apps/backend/app/collectors/x.py', 'README.md'].map((p) => ({ path: p, status: 'M', key: p }))
    expect(treeSort(files).map((f) => f.path)).toEqual(['apps/backend/app/collectors/x.py', 'apps/backend/app/services/pricing.py', 'apps/backend/app/models.py', 'README.md'])
    const rows = treeRows(files, new Set())
    expect(rows.map((r) => (r.kind === 'dir' ? r.name + '/' : r.name))).toEqual(['apps/backend/app/', 'collectors/', 'x.py', 'services/', 'pricing.py', 'models.py', 'README.md'])
    expect(treeRows(files, new Set(['apps/backend/app'])).length).toBe(2)
  })
})

describe('identity', () => {
  it('derives tags', () => {
    expect(['個人 GitHub', 'GitHub EMU', '接案客戶', '新身分 4', 'Personal GitHub', 'ACME prod'].map(autoShort)).toEqual(['個人', 'EMU', '接案', '新身分', 'Personal', 'ACME'])
  })
  it('matches remotes and folders like git includeIf', () => {
    expect(normUrl('git@github.com:acme/x.git')).toBe('github.com/acme/x.git')
    expect(normUrl('https://sam@github.com/acme/x')).toBe('github.com/acme/x')
    const id = (id: string, rules: Identity['rules']): Identity => ({ id, name: id, short: '', color: 'blue', key: '', account: '', verified: false, user: '', email: '', host: '', rules })
    const ids = [id('p', [{ t: 'remote', v: 'github.com/sam/*' }]), id('e', [{ t: 'dir', v: 'D:\\work' }])]
    expect(matchIdentity(ids, 'D:/project/x', 'git@github.com:sam/x.git')?.id).toBe('p')
    expect(matchIdentity(ids, 'D:\\work\\y', 'git@gitlab.com:z/y.git')?.id).toBe('e')
    expect(matchIdentity(ids, 'D:\\workshop\\y', null)).toBeNull()
    expect(ruleForUrl('git@gitlab.com:sam/hft.git')).toBe('gitlab.com/sam/*')
    expect(ruleForUrl('C:/x/remotes/tw.git')).toBeNull()
  })
})

describe('highlight', () => {
  it('splits tokens around the changed range', () => {
    const t = splitTokens([{ text: 'let ', light: 'k' }, { text: 'abc', light: 'v' }], [5, 6])
    expect(t.map((x) => [x.text, !!x.mark])).toEqual([['let ', false], ['a', false], ['b', true], ['c', false]])
  })
})

describe('detected identities', () => {
  it('turns includes and keys into identities', async () => {
    const { fromFound } = await import('./identity')
    const r = fromFound({
      name: 'Sam',
      email: 'sam@me.com',
      keys: [
        { path: String.raw`C:\Users\u\.ssh\id_ed25519`, comment: 'sam@me.com', hosts: [] },
        { path: String.raw`C:\Users\u\.ssh\id_ed25519_emu`, comment: '', hosts: [['github-emu', 'github.com']] },
        { path: String.raw`C:\Users\u\.ssh\id_ed25519_work`, comment: 'x', hosts: [] },
      ],
      includes: [
        { condition: 'hasconfig:remote.*.url:git@github.com:acme/**', path: 'C:/Users/u/.gitconfig-work', name: '', email: 'sam@acme.com', key: 'C:/Users/u/.ssh/id_ed25519_work' },
        { condition: 'gitdir/i:D:/acme/', path: 'C:/Users/u/.gitconfig-work', name: '', email: 'sam@acme.com', key: 'C:/Users/u/.ssh/id_ed25519_work' },
      ],
    })
    expect(r.ids.map((i) => i.name)).toEqual(['work', '預設 key', 'github-emu'])
    expect(r.ids[0].rules).toEqual([{ t: 'remote', v: 'github.com/acme/*' }, { t: 'dir', v: String.raw`D:\acme` }])
    expect(r.ids[1].email).toBe('sam@me.com')
    expect(r.ids[2].email).toBe('sam@me.com')
    expect(r.def).toBe(r.ids[1].id)
  })
})
