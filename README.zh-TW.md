# <img src="public/icon.svg" width="32" height="32" alt="" align="absmiddle"> git-tabs

[English](README.md) | **繁體中文**

> [!WARNING]
> **程式沒有簽章**，第一次打開時系統會擋一下，照下面做一次就好：
> - **Windows**：出現「Windows 已保護您的電腦」→ 按「其他資訊」→「仍要執行」
> - **macOS**：系統設定 → 隱私權與安全性 → 找到 git-tabs 按「強制打開」，或在終端機執行 `xattr -cr /Applications/git-tabs.app`
> - **Linux**：AppImage 先 `chmod +x` 再執行；deb 直接安裝

一個桌面 git 用戶端：每個 repo 一個分頁，專為快速 review 修改而設計。

## 下載

到[最新版本](https://github.com/pg56714/git-tabs/releases/latest)下載你系統對應的檔案。

| 系統 | 檔案 | 說明 |
|---|---|---|
| Windows | `git-tabs_*_x64-setup.exe` | 安裝檔 |
| Windows | `git-tabs_windows_x64.exe` | 免安裝版，直接執行 |
| macOS | `git-tabs_*_universal.dmg` | Intel 與 Apple Silicon 通用 |
| Linux | `git-tabs_*_amd64.AppImage` | 任何發行版可用，先 `chmod +x` |
| Linux | `git-tabs_*_amd64.deb` | Debian / Ubuntu：`sudo apt install ./git-tabs_*.deb` |

`git-tabs_*_universal.app.tar.gz` 是同一個 macOS 程式的壓縮檔，一般用 `.dmg` 即可。

## 功能

- **Repo 分頁**：每個分頁顯示分支、領先／落後幾個 commit、有沒有未提交的修改，以及這個 repo 用的身分。可以拖曳排序，用 `Ctrl+1…9`／`Ctrl+Tab` 切換。
- **Review**：檔案樹可以勾「已看」並顯示進度；diff 可切換單欄或並排，有語法上色，同一行裡改動的字會特別標出。`↑`/`↓` 上/下一個檔案，`←`/`→` 上/下一段改動，`Enter` 標記已看（並跳到下一個沒看的檔案），`S` Stage／取消 Stage。
- **Stage**：可以 stage、取消 stage 或捨棄整個檔案、單一區段，或選取的幾行（點一行，拖曳或按住 Shift 選多行，再按 `S`）。
- **Commit**：commit、commit 並 push、amend（`Ctrl+Enter`、`Ctrl+Shift+Enter`）。
- **歷史**（版面跟 Fork 的 All Commits 一樣：上面是列表，下面是 Commit／Changes 分頁）：涵蓋所有分支的 commit 圖，可用訊息或 SHA 搜尋；有 commit 詳細資訊、每個檔案的 diff，以及 cherry-pick、revert、checkout、reset（soft／mixed／hard）、加 tag、開新分支。也有單一檔案的歷史和 blame。
- **分支 vs main**：一次看完一條分支相對預設分支改了什麼，用同樣的 review 流程，看完就能 merge。
- **遠端**：Sync（fetch → pull → push）、fetch、pull（只快轉、rebase 或 merge）、push（一般或 `--force-with-lease`）、背景定時 fetch。
- 側欄有**分支、tag、stash、worktree、reflog**。worktree 可以直接開成新分頁。
- **衝突**：merge／rebase／cherry-pick 進行中會顯示提示列，衝突區塊用顏色標出，可選「用我的」／「用對方的」／「標記已解決」，再繼續或中止。
- **身分**：可設定多組 SSH key／名字／email，依 remote 網址或資料夾自動選擇，也能針對單一 repo 手動指定。可以測試 key 連 GitHub 對應哪個帳號，也能產生新的 ed25519 key。
- 各個面板之間的分隔線可以拖曳調整，大小會記住。分頁很多時，用滑鼠滾輪就能左右捲動。
- **指令面板**（`Ctrl+K`）、會改變東西的 git 指令都記在指令紀錄裡、深色／淺色主題。

## 身分怎麼運作

儲存身分時，會寫入你的**全域** gitconfig：

- 每條 remote 規則寫一組 `includeIf "hasconfig:remote.*.url:git@github.com:org/**"`（另外加上 https 和 ssh:// 的寫法），
- 每條資料夾規則寫一個 `includeIf "gitdir/i:D:/work/"`，
- 預設身分寫成 `include.path`，

每一條都指向 app 設定資料夾裡的一個小檔案（`identities/<id>.gitconfig`），裡面是 `user.name`、`user.email` 和 `core.sshCommand = ssh -i "<key>" -o IdentitiesOnly=yes`。

所以在終端機直接下 `git`，用的也是和 app 一樣的身分。app 會記得自己加了哪幾行，修改或刪除身分時只會移除那幾行。對單一 repo 手動指定身分時，會把同樣三個設定寫進那個 repo 自己的 `.git/config`，優先於全域規則。

## 專案結構

```
crates/gitcore     所有 git 邏輯（呼叫系統的 git），單一 dispatch(cmd, args) 入口，含測試
crates/devserver   只供開發：在 127.0.0.1 用 HTTP 提供同樣的指令，方便在瀏覽器測試介面
src-tauri          桌面外殼：一個 `call` IPC 指令＋檔案監看
src                Vue 3 介面
```

## 開發

需要：Rust（stable）、Bun 1.4 以上、git 2.36 以上（`hasconfig:` include 需要），Windows 上還需要 MSVC 編譯工具和 WebView2（Windows 11 已內建）。

```bash
bun install
bun run tauri dev        # 桌面 app，支援熱更新
bun run dev:web          # 在瀏覽器開 http://127.0.0.1:5320，後端是 crates/devserver
npm test                 # vitest + cargo 測試
bun run typecheck        # vue-tsc（TypeScript 6）＋ tsc 7 檢查 .ts 檔
bun run lint             # oxlint
bun run tauri build      # release 版（加 --no-bundle 只產生 exe）
```

`crates/devserver` 接受 `--config-dir <dir>` 和 `--git-global <file>`；後者指到一個暫存檔，測試時就不會動到你真正的 gitconfig。

### 發布

到 Actions → **Release** → **Run workflow**。它會依上一個 tag 之後的 Conventional Commits 決定新版本（`feat` → 第二位 +1，`fix`/`perf` → 第三位 +1，1.0 前的破壞性改動 → 第二位 +1），寫入 `CHANGELOG.md`，commit 並打上 `chore(release): vX.Y.Z` tag，在 Windows（安裝檔＋免安裝 exe）、macOS（通用 dmg）、Linux（AppImage＋deb）上編譯，三個都上傳完就直接公開。上一個 tag 之後沒有新東西時會直接停止。`bun scripts/release.ts --dry-run` 可以先看它會怎麼做。

### 工具鏈

Tauri 2、Vue 3、Vite 8（Rolldown）、oxlint／oxfmt、Vitest，語法上色用 shiki。
TypeScript 7（用 Go 寫的原生編譯器）檢查一般的 `.ts` 檔；`.vue` 檔由 vue-tsc 搭配 TypeScript 6 檢查，因為 vue-tsc 目前還不能在 TypeScript 7 上跑。

## 授權

[Apache License 2.0](LICENSE)
