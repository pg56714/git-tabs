# Changelog

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

