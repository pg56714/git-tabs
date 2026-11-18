# <img src="crates/app/icons/icon.svg" width="32" height="32" alt="" align="absmiddle"> git-tabs

[English](README.md) | **繁體中文**

> [!WARNING]
> **程式沒有簽章**，第一次打開時系統會擋一下，照下面做一次就好：
> - **Windows**：出現「Windows 已保護您的電腦」→ 按「其他資訊」→「仍要執行」
> - **macOS**：系統設定 → 隱私權與安全性 → 找到 git-tabs 按「強制打開」，或在終端機執行 `xattr -cr /Applications/git-tabs.app`
> - **Linux**：AppImage 先 `chmod +x` 再執行；deb 直接安裝
>
> 因為沒有簽章，請自行驗證下載的檔案：見[驗證下載的檔案](#驗證下載的檔案)。

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

## 驗證下載的檔案

每個版本都附有 `SHA256SUMS` 檔和建置證明（GitHub 簽發的紀錄，證明檔案是由本專案的發佈流程建置）。兩者任一都能確認檔案就是流程產出的那一個。

```powershell
# Windows：和 SHA256SUMS 裡你那個檔案的那一行比對
Get-FileHash .\git-tabs_windows_x64.exe -Algorithm SHA256
```

```bash
# macOS / Linux，SHA256SUMS 放在同一個資料夾
sha256sum -c --ignore-missing SHA256SUMS      # macOS：shasum -a 256 -c --ignore-missing SHA256SUMS

# 更可靠：需要 GitHub CLI
gh attestation verify <file> --repo pg56714/git-tabs
```

`SHA256SUMS` 放在版本頁面、和檔案同一個地方，所以能擋住損壞或被調包的下載，但擋不住 GitHub 帳號被入侵；能擋的是建置證明。兩者都不能取代程式碼簽章，所以上面的系統警告仍然會出現。程式內的自動更新也會用 `SHA256SUMS` 檢查下載的檔案（該版本有這個檔案時）。

## 功能

- **Repo 分頁**：每個分頁顯示分支、領先／落後幾個 commit、有沒有未提交的修改，以及這個 repo 用的身分。可以拖曳排序，用 `Ctrl+1…9`／`Ctrl+Tab` 切換。
- **Review**：檔案樹可以勾「已看」並顯示進度；diff 可切換單欄或並排，有語法上色，同一行裡改動的字會特別標出。`↑`/`↓` 上/下一個檔案，`←`/`→` 上/下一段改動，`Enter` 標記已看（並跳到下一個沒看的檔案），`S` Stage／取消 Stage。
- **Stage**：可以 stage、取消 stage 或捨棄整個檔案、單一區段，或選取的幾行（點一行，拖曳或按住 Shift 選多行，再按 `S`）。
- **Commit**：commit、commit 並 push、amend（`Ctrl+Enter`、`Ctrl+Shift+Enter`）。
- **歷史**（版面跟 Fork 的 All Commits 一樣：上面是列表，下面是 Commit／Changes／File Tree 分頁）：涵蓋所有分支的 commit 圖，畫法跟 Fork 一樣；目前分支沒有的 commit 會變灰，還沒 push 的 commit 前面有一個點，點 merge 的圓點可以把它帶進來的 commit 收起來。可用訊息或 SHA 搜尋；有 commit 詳細資訊、每個檔案的 diff、那個 commit 當時完整的檔案樹和檔案內容，以及 cherry-pick、revert、checkout（雙擊也可以）、reset（soft／mixed／hard）、加 tag、開新分支。分支自己的 commit 可以改訊息、併入上一個 commit、丟掉。分支標籤雙擊可切換，右鍵有分支選單。也有單一檔案的歷史和 blame。
- **遠端**：Sync（fetch → pull → push）、fetch、pull（只快轉、rebase 或 merge）、push（一般或 `--force-with-lease`）、背景定時 fetch。可以新增、改網址或移除 remote，也能刪除遠端上的分支。
- 側欄有**分支、遠端、tag、stash、worktree、submodule、reflog**，上面有篩選框。名稱裡有 `/` 的分支會收進資料夾；分支可以重新命名。點 stash 可以看它裡面改了什麼。worktree 可以從側欄新增，並開成自己的分頁；submodule 也一樣能開成分頁。
- **衝突**：merge／rebase／cherry-pick 進行中會顯示提示列，衝突區塊用顏色標出，可選「用我的」／「用對方的」／「標記已解決」，再繼續或中止。
- **身分**：可設定多組 SSH key／名字／email，每個 repo 自己選用哪一組。沒選的 repo 不會有身分；在那之前 commit 會先顯示 git 實際會用的作者。可以測試 key 連 GitHub 對應哪個帳號，也能產生新的 ed25519 key。
- 各個面板之間的分隔線可以拖曳調整，大小會記住；視窗的大小和位置也會記住。分頁很多時，用滑鼠滾輪就能左右捲動。
- **指令面板**（`Ctrl+K`）、會改變東西的 git 指令都記在指令紀錄裡、深色／淺色主題。

在 macOS 上，指令面板和 commit 的快捷鍵用 `Cmd` 而不是 `Ctrl`（`Cmd+K`、`Cmd+Enter`）。

## 身分怎麼運作

每個 repo 自己選身分（工具列的身分按鈕）。選了之後，會把 `user.name`、`user.email` 和 `core.sshCommand = ssh -i "<key>" -o IdentitiesOnly=yes` 寫進那個 repo 自己的 `.git/config`，所以在終端機直接下 `git`，用的也是和 app 一樣的身分。

不會寫任何東西到你的全域 gitconfig。0.10 以前的版本是用規則選身分，會在全域寫 `includeIf`；第一次啟動新版時，app 會把那些規則對到的 repo 各自固定好身分，再移除它當初加的那幾行。

## 語言

介面有繁體中文和英文。第一次啟動時會跟著系統語言；隨時可以到**設定 → 一般 → 語言**切換。

## 專案結構

```
crates/gitcore   所有 git 邏輯（呼叫系統的 git）、型別化的操作、測試
crates/app       桌面程式，用 GPUI Kit 寫成（GPU 繪製，不含網頁）
```

## 開發

需要：Rust（版本見 `rust-toolchain.toml`）、git。Linux 另外需要 `.github/workflows/release.yml` 建置工作裡安裝的那些套件。

```bash
cargo run -p git-tabs          # 執行程式
cargo test --workspace         # 單元測試與端到端測試，包含發版小工具
```

開發時好用的參數：`--config-dir <資料夾>` 把設定放在暫存資料夾，`--git-global <檔案>` 讓 git 讀暫存的全域設定，測試時就不會動到你真正的設定。`--check <repo>` 不開視窗，只印出 repo 的分支與有修改的檔案。

### 發版

Actions → **Release** → **Run workflow**。它會依上次 tag 之後的 Conventional Commits 決定下一個版本（`feat` → minor，`fix`/`perf` → patch，破壞性修改在 1.0 之前 → minor），寫入 `CHANGELOG.md`，commit 並打上 `chore(release): vX.Y.Z` 的 tag，建出 Windows（安裝檔加免安裝 exe）、macOS（universal dmg）和 Linux（AppImage 加 deb），三個都上傳完才公開。上次 tag 之後沒有新東西就直接停止，不發版。`cargo run -p release -- --dry-run` 可以先看它會做什麼。

### 工具鏈

Rust、[GPUI Kit](https://github.com/longbridge/gpui-kit)（Zed 編輯器背後的 UI 框架，用它現成的 diff、列表、輸入元件），安裝檔由 [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) 產生。

## 授權

[Apache License 2.0](LICENSE)
