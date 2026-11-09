// Transport to gitcore: Tauri IPC in the desktop app, HTTP to crates/devserver in a browser.

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export async function call<T = unknown>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core')
    try {
      return await invoke<T>('call', { cmd, args })
    } catch (e) {
      throw new Error(String(e))
    }
  }
  const r = await fetch(`/api/${cmd}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-git-tabs': '1' },
    body: JSON.stringify(args),
  })
  const v = await r.json().catch(() => '無法連線到開發伺服器')
  if (!r.ok) throw new Error(typeof v === 'string' ? v : JSON.stringify(v))
  return v as T
}

export async function pickFolder(title: string): Promise<string | null> {
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const r = await open({ directory: true, title })
    return typeof r === 'string' ? r : null
  }
  return window.prompt(title + '（輸入完整路徑）')?.trim() || null
}

export async function pickFile(title: string, defaultPath?: string): Promise<string | null> {
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const r = await open({ directory: false, title, defaultPath })
    return typeof r === 'string' ? r : null
  }
  return window.prompt(title + '（輸入完整路徑）', defaultPath)?.trim() || null
}

export async function watchRepos(paths: string[]): Promise<void> {
  if (!isTauri) return
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('watch', { repos: paths }).catch(() => {})
}

export async function onRepoChanged(cb: (path: string) => void): Promise<void> {
  if (!isTauri) return
  const { listen } = await import('@tauri-apps/api/event')
  await listen<string>('repo-changed', (e) => cb(e.payload))
}

export async function openUrl(url: string) {
  if (isTauri) {
    const opener = await import('@tauri-apps/plugin-opener')
    await opener.openUrl(url)
  } else {
    window.open(url, '_blank', 'noopener')
  }
}

export async function copyText(text: string) {
  await navigator.clipboard?.writeText(text).catch(() => {})
}
