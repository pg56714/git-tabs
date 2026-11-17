# Changelog

## v0.8.0 (2026-11-17)

### Features

- **changes:** keep the Unstaged / Staged / commit panes when the tree is clean, as Fork does ([492467d](https://github.com/pg56714/git-tabs/commit/492467df195e5d91a4e02c6c857cae3b9d38df2f))
- **ui:** the tab row is the window's title bar ([823e9e3](https://github.com/pg56714/git-tabs/commit/823e9e3891aac3de8761e4abd39a5dfd148d4ffa))
- **ui:** Fork-style ref labels and date column ([b04ab0a](https://github.com/pg56714/git-tabs/commit/b04ab0a3d9d48a652110278d7ce6dc216bac3b81))

### Fixes

- **history:** checking out a commit that carries a branch label lands on that branch ([1dcc0d0](https://github.com/pg56714/git-tabs/commit/1dcc0d0b1c00eff0e358b3f2729c985b8dc679a5))

## v0.7.0 (2026-11-17)

### Features

- **ui:** syntax colours from GitHub's light and dark themes ([1380102](https://github.com/pg56714/git-tabs/commit/1380102f4054a563ecdde75ae8d6f04434b17863))
- **history:** SHA row with copy and open-on-GitHub icons, as in Fork ([d1c12ae](https://github.com/pg56714/git-tabs/commit/d1c12ae2b074372635f7cf74d6809e9cfbdb2a56))
- **ui:** fade a branch label with its row when the checked-out branch lacks the commit; softer added/removed line background ([9d3616d](https://github.com/pg56714/git-tabs/commit/9d3616dda91a729dfb3c90cd13a9d9ea277c9cdc))
- **ui:** branch labels with an orange outline and near-white fill, as Fork draws them ([5375c85](https://github.com/pg56714/git-tabs/commit/5375c85fa2873a05e24c163b2c2ec47343303f3f))
- **ui:** brighter red and green for added and removed lines, as Fork draws them ([6e80cd3](https://github.com/pg56714/git-tabs/commit/6e80cd338014b97eca96fd81b88ccd92e25f6e8d))
- **ui:** syntax colours for more languages; tighter diff rows (vendored gpui-component, row height 1.35x) ([dcfa1f3](https://github.com/pg56714/git-tabs/commit/dcfa1f3199207e2941fea430d719aa4942d6406c))
- **ui:** code font 12.5 -> 14px, closer to Fork's diff ([82b8c42](https://github.com/pg56714/git-tabs/commit/82b8c42103b72e5de9a2963a6f28589b2d111d8d))

### Fixes

- **ui:** use GitHub's current comment and entity colours ([d8f72e1](https://github.com/pg56714/git-tabs/commit/d8f72e106eefe1c91c283c8bf753d490f6708aa6))
- **ui:** YAML highlighting: keys green, plain values uncoloured, as on GitHub ([fc42262](https://github.com/pg56714/git-tabs/commit/fc422624539ef5101c42bbc4b8008f647c342be4))
- **ui:** tags back to blue; draw the current-branch check as an icon; bolder orange label outlines ([431b201](https://github.com/pg56714/git-tabs/commit/431b201fbd812423e3e10ea24c417d58abf116d0))

## v0.6.0 (2026-11-17)

### Features

- **update:** manual "Check for updates" instead of 6-hourly polling; pin Rust 1.99.1 and cargo-packager 0.11.8 ([f0f3488](https://github.com/pg56714/git-tabs/commit/f0f3488423dbdfb07b49cdf57d7adb2ae4d201a1))

### Fixes

- **ui:** jump to the first change once a new file's diff is measured ([0354ca0](https://github.com/pg56714/git-tabs/commit/0354ca07be8eef6a4cab16ed80354b74c6b6aaeb))

## v0.5.0 (2026-11-16)

### Features

- **history:** use the GitHub CLI's logins to find author pictures in private repos ([e678496](https://github.com/pg56714/git-tabs/commit/e6784966001de739815fdeaa1354062528194a8b))
- **history:** fall back to GitHub's account for a commit when Gravatar has no picture ([de83568](https://github.com/pg56714/git-tabs/commit/de835684a6e14157b1daed0356ca35c588fd03dd))
- **history:** author pictures from GitHub/Gravatar; AUTHOR and COMMITTER blocks side by side ([8375bcf](https://github.com/pg56714/git-tabs/commit/8375bcfe3cda687eeb189dd7adbfc6bd4ea8e437))
- self-update, author avatars, wider date column; keep tabs of unplugged drives ([edca35d](https://github.com/pg56714/git-tabs/commit/edca35defb97a636047ff7d23043278d492fa0f4))

### Fixes

- **ui:** always show scrollbars so a wide diff can be scrolled sideways ([5b9d0c6](https://github.com/pg56714/git-tabs/commit/5b9d0c67f590a72a0e46f9c25d3746d381559c73))
- **ui:** wrap lines when switching to side-by-side; old avatar misses no longer block lookups ([b13c3d5](https://github.com/pg56714/git-tabs/commit/b13c3d5392227eaca1975eeb8995a5b13ef2658a))
- **ui:** start a fresh diff view per file in side-by-side mode ([dcf1662](https://github.com/pg56714/git-tabs/commit/dcf16620faf728d43869c32af410d759483a3dcc))
- **identity:** match remote URLs the way git's includeIf does ([457c97f](https://github.com/pg56714/git-tabs/commit/457c97f5513ca941b84f1fe61e9797350e1b687e))
- **gitcore:** rebase side labels, force-push safety, stale line ops, cmd injection, CRLF, # lines ([e05b9da](https://github.com/pg56714/git-tabs/commit/e05b9da6ef681e106930981329177774231e761c))

## v0.4.1 (2026-11-15)

### Fixes

- **identity:** refuse to save an empty rule ([990bda1](https://github.com/pg56714/git-tabs/commit/990bda1bc52d7d1459ba284504b295e2561a6e10))
- **identity:** clean up include lines left in the global gitconfig ([8be95ff](https://github.com/pg56714/git-tabs/commit/8be95fff07b29c652dfd7c5ce9ef95b387bde4d5))

## v0.4.0 (2026-11-13)

### Features

- **ui:** toolbar icons; unpushed dot between graph and text ([283fb2f](https://github.com/pg56714/git-tabs/commit/283fb2f2321109b740ffe7fe868dc24b578be873))
- rewrite older commits, file tree, fold merges, manage remotes, sidebar filter and more ([db78990](https://github.com/pg56714/git-tabs/commit/db789901fd4fceb8f551149097248aa1f47ad15b))
- **ui:** one label per branch, sidebar icons, new worktree, no Auto identity ([9de547c](https://github.com/pg56714/git-tabs/commit/9de547cd8044f072bd78b14778387443c196a1b9))
- **history:** draw the graph the way Fork does; vector icons on ref labels ([6f6b6f0](https://github.com/pg56714/git-tabs/commit/6f6b6f0aebc88994af39b8286e7e0df76e0a50e3))
- **ui:** Fork-style branches and history ([c16a7f3](https://github.com/pg56714/git-tabs/commit/c16a7f3ccbd816e762b4f70aae5d7414bf157d52))
- **identity:** remove the default identity; repos without a match are chosen by hand ([ead5254](https://github.com/pg56714/git-tabs/commit/ead5254cfe557143e59355c38b334be44bbceab5))

### Fixes

- **ui:** only a row with an unpushed dot makes room for it, as in Fork ([db5cace](https://github.com/pg56714/git-tabs/commit/db5cace477d35c8f3bf3cfa597ea8653df1d6d0d))

## v0.3.0 (2026-11-11)

### Features

- remove identity tags, name-only identity menu, debug log, static CRT, fix menu layering and open-folder path ([d91b8e2](https://github.com/pg56714/git-tabs/commit/d91b8e2a5b406e7d9130b8bd147f618e24473a37))

### Other

- remove the CI workflow (packaging is in release.yml) ([3efa463](https://github.com/pg56714/git-tabs/commit/3efa463e217181411b66e4f75864375c53a3de20))

## v0.2.0 (2026-11-10)

### Features

- **app:** English and Traditional Chinese interface with automatic detection and a setting; centred tab-bar icons ([71e7b8e](https://github.com/pg56714/git-tabs/commit/71e7b8e692ec5129c1d87eaa742d40c5eb05b3ba))
- **app:** port the whole UI to GPUI (tabs, sidebar, changes, history, range, reflog, dialogs) ([e991481](https://github.com/pg56714/git-tabs/commit/e991481d54fec17c0b9c4363b1c4194116bab121))
- **app:** repo tabs, changes view, line staging, commit and remotes ([639584f](https://github.com/pg56714/git-tabs/commit/639584f51adcaa7125e688d8158169a9d9ed85a4))
- **app:** GPUI app crate with diff patch and line-selection mapping ([9a661e6](https://github.com/pg56714/git-tabs/commit/9a661e62075cb4c36e898f1aad584840b7cc69bc))

### Fixes

- **app:** keyboard focus, warnings, log bar ellipsis, wording ([1137577](https://github.com/pg56714/git-tabs/commit/11375773a0690f202630e2b1c0bb3566183d6b18))
- **app:** shortcut bindings need static strings ([5ee6ff7](https://github.com/pg56714/git-tabs/commit/5ee6ff70a6c1cdcec7122e4fcab57cd48d96d9f0))
- **app:** conflict view parses; i18n polish ([b2bbfa7](https://github.com/pg56714/git-tabs/commit/b2bbfa793681dc28b155fe144dc201fc50e75663))

### Other

- run on main and pull requests only ([88b954f](https://github.com/pg56714/git-tabs/commit/88b954f0d8f6b3c160751675c582d8f10c6b659c))
- upload artifacts with actions/upload-artifact v7 (Node.js 24) ([4405f56](https://github.com/pg56714/git-tabs/commit/4405f56ec523b476dff0303e77bb866620a08228))
- build the Linux packages on Ubuntu 24.04 ([2b9e2d5](https://github.com/pg56714/git-tabs/commit/2b9e2d5ab94984ca452f871ffd15ad8c7037a72d))
- **app:** guard the translations (every string has English, holes match, nothing left untranslated) ([837692f](https://github.com/pg56714/git-tabs/commit/837692fec2e72362d4759aca3300aa928acb97f1))
- cargo-packager installers, CI workflow, release script for the Rust app; docs; cmd shortcuts on macOS; remove the Tauri/Vue code ([cafd59e](https://github.com/pg56714/git-tabs/commit/cafd59efd6d8efad418120c379416a099e00dd22))
- before i18n ([be56535](https://github.com/pg56714/git-tabs/commit/be56535aa5055b83ce076402d3ada130fabb3578))
- dialogs defaults, icons, first-run identities ([b6eb8ac](https://github.com/pg56714/git-tabs/commit/b6eb8ace22edcab127af0f57df6252f9f932ae76))
- add Apache-2.0 license and a download table ([3320ba9](https://github.com/pg56714/git-tabs/commit/3320ba99094923279c93df4b1261d39600a46359))

## v0.1.1 (2026-11-10)

### Fixes

- **ci:** get the draft release id from the create call so assets upload ([69585d6](https://github.com/pg56714/git-tabs/commit/69585d6ada4c80425cfc855dc3e5d22e2739eb24))

## v0.1.0 (2026-11-10)

### Features

- **history:** Fork-style layout and draggable pane dividers ([cbff5cb](https://github.com/pg56714/git-tabs/commit/cbff5cb3f30b9a4e45b3a4f29e476236aa0ff367))
- arrow-key review, instant staging and identity detection ([18aa5db](https://github.com/pg56714/git-tabs/commit/18aa5db092bd7b5eedb6719a82dffce32a762c34))
- add git-tabs desktop git client ([45e6986](https://github.com/pg56714/git-tabs/commit/45e69868212d2938126de6c1136d9ee7d7a18a2f))

### Fixes

- **ui:** keep buttons reachable in narrow panes and windows ([aeff971](https://github.com/pg56714/git-tabs/commit/aeff9713c7d1b551c85924bb5031cae7134f9cfb))
- scrollable tab row and fallback identity ([ee6e977](https://github.com/pg56714/git-tabs/commit/ee6e977dc16b204046bca6ae14e2648bb0496f62))
- **history:** keep commit messages visible and tighten the graph column ([6d88e11](https://github.com/pg56714/git-tabs/commit/6d88e1110f7fdcb95e758a7bd8191780276115e2))

### Performance

- **diff:** highlight in a web worker and stop status from rewriting the index ([bab2f35](https://github.com/pg56714/git-tabs/commit/bab2f3517489323130113a1c97cf8d015c4609f5))

### Other

- **readme:** warn that the app is unsigned and describe releasing ([bd209be](https://github.com/pg56714/git-tabs/commit/bd209bee7de4073333068bd32986af7dd24144b2))
- **release:** manual release workflow with automatic version and changelog ([6f6187e](https://github.com/pg56714/git-tabs/commit/6f6187e8919af2c0ade83017066c51c89821d7db))
- switch from npm to bun and remove the prototype ([70403e8](https://github.com/pg56714/git-tabs/commit/70403e8d832f71dd82e8a5fcfa680a5d44fe55b3))
- **ui:** center the commit graph, darken borders and indent the sidebar ([6e8723b](https://github.com/pg56714/git-tabs/commit/6e8723be6e722099b9001713314c34ee018ab37a))
- **readme:** center the title icon and add a Traditional Chinese version ([f0dba6f](https://github.com/pg56714/git-tabs/commit/f0dba6f0742d36e78f045338f9e6c65314b1c24a))
- **readme:** show the icon next to the title ([71316bc](https://github.com/pg56714/git-tabs/commit/71316bc1c40d11a81937ea8e15a8292df90606e8))

