export interface TreeFile {
  path: string
  status: string
  key: string
}

export type TreeRow = { kind: 'dir'; name: string; depth: number; dir: string } | { kind: 'file'; file: TreeFile; name: string; depth: number }

/** Same order as the tree: in each folder, sub-folders first, then files, both A→Z. */
export function treeSort<T extends { path: string }>(files: T[]): T[] {
  // Build each sort key once (not twice per comparison).
  return files
    .map((f) => {
      const s = f.path.split('/')
      return { f, k: s.map((x, i) => (i < s.length - 1 ? '0' : '1') + x.toLowerCase()).join('/') }
    })
    .sort((a, b) => (a.k < b.k ? -1 : a.k > b.k ? 1 : 0))
    .map((x) => x.f)
}

interface Node {
  dirs: Map<string, Node>
  files: TreeFile[]
}

/** Flattened rows for a file tree; single-child folder chains are merged ("apps/backend/app"). */
export function treeRows(files: TreeFile[], collapsed: Set<string>): TreeRow[] {
  const root: Node = { dirs: new Map(), files: [] }
  for (const f of treeSort(files)) {
    const parts = f.path.split('/')
    let n = root
    for (const p of parts.slice(0, -1)) {
      if (!n.dirs.has(p)) n.dirs.set(p, { dirs: new Map(), files: [] })
      n = n.dirs.get(p)!
    }
    n.files.push(f)
  }
  const rows: TreeRow[] = []
  const walk = (n: Node, depth: number, prefix: string) => {
    for (let [name, c] of n.dirs) {
      let full = prefix + name
      while (c.dirs.size === 1 && c.files.length === 0) {
        const [k, v] = [...c.dirs][0]
        name += '/' + k
        full += '/' + k
        c = v
      }
      rows.push({ kind: 'dir', name, depth, dir: full })
      if (!collapsed.has(full)) walk(c, depth + 1, full + '/')
    }
    for (const f of n.files) rows.push({ kind: 'file', file: f, name: f.path.split('/').pop()!, depth })
  }
  walk(root, 0, '')
  return rows
}
