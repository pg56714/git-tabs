# <img src="crates/app/icons/icon.svg" width="32" height="32" alt="" align="absmiddle"> git-tabs

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
- **History** (laid out like Fork's All Commits: list on top, Commit / Changes / File Tree tabs below): commit graph across all branches, drawn the way Fork draws it; commits the checked-out branch does not have are greyed, commits no remote has yet carry a dot, and a merge's dot folds away what it brought in. Search by message or SHA, commit details, per-file diff, the whole file tree of a commit with each file as it was then, cherry-pick, revert, checkout (also by double-click), reset (soft / mixed / hard), tag, new branch. For commits of the branch itself: edit the message, squash into the previous commit, drop. Branch labels switch on double-click and have the branch menu. File history and blame.
- **Remotes**: Sync (fetch → pull → push), fetch, pull (fast-forward, rebase or merge), push (normal or `--force-with-lease`), background fetch. Add, re-point or remove a remote; delete a branch on the remote.
- **Branches, remotes, tags, stashes, worktrees, submodules, reflog** in the sidebar, with a filter box. Branches with a `/` in their name are grouped into folders; a branch can be renamed. Clicking a stash shows what it holds. A worktree is added from the sidebar and opens as its own tab; so does a submodule.
- **Conflicts**: a banner while a merge/rebase/cherry-pick is in progress, conflict regions highlighted, "use mine" / "use theirs" / "mark resolved", continue or abort.
- **Identities**: several SSH keys / names / emails, picked by remote URL or folder, or chosen per repo. A repo no rule matches has no identity until you choose one, and committing there first shows the author git would use. Test a key against GitHub, generate a new ed25519 key.
- Panes can be resized by dragging the dividers; their sizes are remembered, and so are the window's size and place. Lots of tabs scroll sideways with the mouse wheel.
- **Command palette** (`Ctrl+K`), command log of every git command that changed something, light/dark theme.

## How identities work

Saving identities writes, to your **global** gitconfig:

- `includeIf "hasconfig:remote.*.url:git@github.com:org/**"` (and the https / ssh:// forms) for each remote rule,
- `includeIf "gitdir/i:D:/work/"` for each folder rule,

each pointing at a small file (`identities/<id>.gitconfig` in the app's config folder) holding `user.name`, `user.email` and `core.sshCommand = ssh -i "<key>" -o IdentitiesOnly=yes`.

Plain `git` in a terminal therefore uses the same identity as the app. The app remembers exactly which lines it added and removes only those when you change or delete identities. A repo that matches no rule gets no identity from the app: git uses whatever your global config says, until you choose one for it. Choosing an identity for one repo writes the same three settings into that repo's own `.git/config`, which wins over the global rules.

## Languages

The interface is available in English and Traditional Chinese. It follows the language of your system at the first start; change it any time in **Settings → General → Language**.

## Layout

```
crates/gitcore   all git logic (runs the system git), typed operations, tests
crates/app       the desktop app, built on GPUI Kit (GPU-rendered, no web view)
```

## Development

Requirements: Rust (stable), git 2.36+ (for `hasconfig:` includes). On Linux also the packages listed in `.github/workflows/ci.yml`.

```bash
cargo run -p git-tabs          # run the app
cargo test --workspace         # unit and end-to-end tests
bun test scripts               # the release script (needs Bun)
```

Useful flags while developing: `--config-dir <dir>` keeps the settings in a scratch folder and `--git-global <file>` points git at a scratch global config, so testing never touches your real settings. `--check <repo>` prints a repo's branch and changed files without opening a window.

### Releasing

Actions → **Release** → **Run workflow**. It picks the next version from the Conventional Commits since the last tag (`feat` → minor, `fix`/`perf` → patch, breaking → minor before 1.0), writes `CHANGELOG.md`, commits and tags `chore(release): vX.Y.Z`, builds Windows (installer + portable exe), macOS (universal dmg) and Linux (AppImage + deb), and publishes the release when all three have uploaded. With nothing new since the last tag it stops without releasing. `bun scripts/release.ts --dry-run` shows what it would do.

The **CI** workflow runs the tests and a trial packaging on all three systems for every push, without publishing.

### Toolchain

Rust, [GPUI Kit](https://github.com/longbridge/gpui-kit) (the UI framework behind the Zed editor, with its diff, list and input components), installers by [cargo-packager](https://github.com/crabnebula-dev/cargo-packager).

## License

[Apache License 2.0](LICENSE)
