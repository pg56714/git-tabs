# <img src="public/icon.svg" width="32" height="32" alt="" align="absmiddle"> git-tabs

**English** | [繁體中文](README.zh-TW.md)

> [!WARNING]
> **The app is not code-signed**, so the system blocks it the first time. Do this once:
> - **Windows**: "Windows protected your PC" → **More info** → **Run anyway**
> - **macOS**: System Settings → Privacy & Security → find git-tabs and click **Open Anyway**, or run `xattr -cr /Applications/git-tabs.app` in Terminal
> - **Linux**: run `chmod +x` on the AppImage first; the .deb installs as usual

A desktop git client with one tab per repository, built for reviewing changes quickly.

## Download

Grab the file for your system from the [latest release](https://github.com/pg56714/git-tabs/releases/latest).

| System | File | Notes |
|---|---|---|
| Windows | `git-tabs_*_x64-setup.exe` | Installer |
| Windows | `git-tabs_windows_x64.exe` | Portable, runs without installing |
| macOS | `git-tabs_*_universal.dmg` | Intel and Apple Silicon |
| Linux | `git-tabs_*_amd64.AppImage` | Runs anywhere, `chmod +x` first |
| Linux | `git-tabs_*_amd64.deb` | Debian / Ubuntu: `sudo apt install ./git-tabs_*.deb` |

`git-tabs_*_universal.app.tar.gz` is the same macOS app zipped; most people want the `.dmg`.

## What it does

- **Repo tabs**: each tab shows the branch, ahead/behind, uncommitted changes and which identity the repo uses. Drag to reorder, `Ctrl+1…9` / `Ctrl+Tab` to switch.
- **Review**: file tree with a "reviewed" checkbox and progress, unified or side-by-side diff, syntax colors, changed words highlighted inside a line. `↑`/`↓` next/previous file, `←`/`→` previous/next hunk, `Enter` mark reviewed (and jump to the next unreviewed file), `S` stage/unstage.
- **Staging**: stage, unstage or discard a whole file, one hunk, or selected lines (click the line, drag or Shift-click for more, then `S`).
- **Commit**: commit, commit and push, amend (`Ctrl+Enter`, `Ctrl+Shift+Enter`).
- **History** (laid out like Fork's All Commits: list on top, Commit / Changes tabs below): commit graph across all branches, search by message or SHA, commit details, per-file diff, cherry-pick, revert, checkout, reset (soft / mixed / hard), tag, new branch. File history and blame.
- **Branch vs main**: everything a branch changed compared with the default branch, with the same review flow, then merge.
- **Remotes**: Sync (fetch → pull → push), fetch, pull (fast-forward, rebase or merge), push (normal or `--force-with-lease`), background fetch.
- **Branches, tags, stashes, worktrees, reflog** in the sidebar. A worktree opens as its own tab.
- **Conflicts**: a banner while a merge/rebase/cherry-pick is in progress, conflict regions highlighted, "use mine" / "use theirs" / "mark resolved", continue or abort.
- **Identities**: several SSH keys / names / emails, picked automatically by remote URL or folder, or pinned per repo. Test a key against GitHub, generate a new ed25519 key.
- Panes can be resized by dragging the dividers; sizes are remembered. Lots of tabs scroll sideways with the mouse wheel.
- **Command palette** (`Ctrl+K`), command log of every git command that changed something, light/dark theme.

## How identities work

Saving identities writes, to your **global** gitconfig:

- `includeIf "hasconfig:remote.*.url:git@github.com:org/**"` (and the https / ssh:// forms) for each remote rule,
- `includeIf "gitdir/i:D:/work/"` for each folder rule,
- `include.path` for the fallback identity,

each pointing at a small file (`identities/<id>.gitconfig` in the app's config folder) holding `user.name`, `user.email` and `core.sshCommand = ssh -i "<key>" -o IdentitiesOnly=yes`.

Plain `git` in a terminal therefore uses the same identity as the app. The app remembers exactly which lines it added and removes only those when you change or delete identities. Pinning an identity to one repo writes the same three settings into that repo's own `.git/config`, which wins over the global rules.

## Layout

```
crates/gitcore     all git logic (runs the system git), one dispatch(cmd, args) entry point, tests
crates/devserver   dev only: the same commands over HTTP on 127.0.0.1, for testing the UI in a browser
src-tauri          desktop shell: one `call` IPC command + a file watcher
src                Vue 3 UI
```

## Development

Requirements: Rust (stable), Bun 1.4+, git 2.36+ (for `hasconfig:` includes), and on Windows the MSVC build tools and WebView2 (built into Windows 11).

```bash
bun install
bun run tauri dev        # desktop app with hot reload
bun run dev:web          # UI in a browser at http://127.0.0.1:5320 against crates/devserver
npm test                 # vitest + cargo tests
bun run typecheck        # vue-tsc (TypeScript 6) + tsc 7 on the .ts files
bun run lint             # oxlint
bun run tauri build      # release build (add --no-bundle for just the exe)
```

`crates/devserver` accepts `--config-dir <dir>` and `--git-global <file>`; point the latter at a scratch file so testing never touches your real gitconfig.

### Releasing

Actions → **Release** → **Run workflow**. It picks the next version from the Conventional Commits since the last tag (`feat` → minor, `fix`/`perf` → patch, breaking → minor before 1.0), writes `CHANGELOG.md`, commits and tags `chore(release): vX.Y.Z`, builds Windows (installer + portable exe), macOS (universal dmg) and Linux (AppImage + deb), and publishes the release when all three have uploaded. With nothing new since the last tag it stops without releasing. `bun scripts/release.ts --dry-run` shows what it would do.

### Toolchain

Tauri 2, Vue 3, Vite 8 (Rolldown), oxlint / oxfmt, Vitest, shiki for syntax colors.
TypeScript 7 (the native Go compiler) checks the plain `.ts` files; `.vue` files are checked by vue-tsc on TypeScript 6, because vue-tsc can't run on TypeScript 7 yet.

## License

[Apache License 2.0](LICENSE)
