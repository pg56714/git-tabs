# <img src="crates/app/icons/icon.svg" width="32" height="32" alt="" align="absmiddle"> git-tabs

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

## 語言

介面有繁體中文和英文。第一次啟動時會跟著系統語言；隨時可以到**設定 → 一般 → 語言**切換。

## 專案結構

```
crates/gitcore   所有 git 邏輯（呼叫系統的 git）、型別化的操作、測試
crates/app       桌面程式，用 GPUI Kit 寫成（GPU 繪製，不含網頁）
```

## 開發

需要：Rust（stable）、git 2.36+（為了 `hasconfig:` include）。Linux 另外需要 `.github/workflows/ci.yml` 裡列的套件。

```bash
cargo run -p git-tabs          # 執行程式
cargo test --workspace         # 單元測試與端到端測試
bun test scripts               # 發版腳本（需要 Bun）
```

開發時好用的參數：`--config-dir <資料夾>` 把設定放在暫存資料夾，`--git-global <檔案>` 讓 git 讀暫存的全域設定，測試時就不會動到你真正的設定。`--check <repo>` 不開視窗，只印出 repo 的分支與有修改的檔案。

### 發版

Actions → **Release** → **Run workflow**。它會依上次 tag 之後的 Conventional Commits 決定下一個版本（`feat` → minor，`fix`/`perf` → patch，破壞性修改在 1.0 之前 → minor），寫入 `CHANGELOG.md`，commit 並打上 `chore(release): vX.Y.Z` 的 tag，建出 Windows（安裝檔加免安裝 exe）、macOS（universal dmg）和 Linux（AppImage 加 deb），三個都上傳完才公開。上次 tag 之後沒有新東西就直接停止，不發版。`bun scripts/release.ts --dry-run` 可以先看它會做什麼。

**CI** 工作流程每次 push 都會跑測試，並在三個系統試打包（不發布）。

### 工具鏈

Rust、[GPUI Kit](https://github.com/longbridge/gpui-kit)（Zed 編輯器背後的 UI 框架，用它現成的 diff、列表、輸入元件），安裝檔由 [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) 產生。

## 授權

[Apache License 2.0](LICENSE)
