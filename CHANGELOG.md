# Changelog

All notable PolyGlid changes are recorded here by the release workflow.

## [0.11.0](https://github.com/anonto42/polyglid/compare/v0.10.1...v0.11.0) (2026-08-07)


### Features

* add macos dmg release artifact ([7245482](https://github.com/anonto42/polyglid/commit/7245482ca7d025a9863aa0aa0b06dfaa100789ad))
* add website and affected CI automation ([127464d](https://github.com/anonto42/polyglid/commit/127464d1c456c85825cc4fadfd6f18214f7c20c6))
* add windows msi release artifact ([9e522e6](https://github.com/anonto42/polyglid/commit/9e522e66f9ae79889f3a826aeec84d3b9deb5adf))
* initialize cross-platform runtime directories ([6327ba7](https://github.com/anonto42/polyglid/commit/6327ba7478a9c056e542c5990de1104a414d0448))
* integrate runtime guidance into platform archives ([036d18d](https://github.com/anonto42/polyglid/commit/036d18d0c56d959c732a44ae2dc9b5a82df3e90c))
* integrate runtime setup into appimage release ([9dcbd52](https://github.com/anonto42/polyglid/commit/9dcbd52c9d16fa94023e8a93dfe18a50a5ea53fe))
* publish release update manifest ([1acf88f](https://github.com/anonto42/polyglid/commit/1acf88f88d0b1b1f493f041632ab20a98dc95432))
* **release:** package Linux AppImage alongside tar.gz ([473f67a](https://github.com/anonto42/polyglid/commit/473f67ae244f0ebf78703d6104b902bb490dae7e))
* report versioned first-run setup ([c1bac2a](https://github.com/anonto42/polyglid/commit/c1bac2a8b86cab3c9e24804292daf551193724d3))
* **site:** deploy website with stable CI tooling ([325e0f6](https://github.com/anonto42/polyglid/commit/325e0f6eab4eb1629857e2fb80148edd8f9c9119))
* surface setup status in desktop startup ([29d0b5b](https://github.com/anonto42/polyglid/commit/29d0b5b10378b0418bb449b9d83cd624aff592c8))
* **workbench:** add GPUI editor shell ([d88d327](https://github.com/anonto42/polyglid/commit/d88d327bcdfd854e94f26a649a49e1c630426384))


### Bug Fixes

* **ci:** gate release candidates on signing readiness ([48a4020](https://github.com/anonto42/polyglid/commit/48a4020ae3b146fecf85f86253dd3294e4941b08))
* **release:** make manifest publishing idempotent ([4b381c7](https://github.com/anonto42/polyglid/commit/4b381c70af3c4a587cfe569b431c5e85fe5c2b31))
* **release:** safely resolve release pull request ([c8d633e](https://github.com/anonto42/polyglid/commit/c8d633ed47f952c707dd5f92fb9016af8c3121a9))
* **release:** support virtual Cargo workspace versions ([831d89d](https://github.com/anonto42/polyglid/commit/831d89d7ae1dd1c71504ace1007a85603759a03a))


### Performance Improvements

* **ci:** streamline candidate and release delivery ([bd803ee](https://github.com/anonto42/polyglid/commit/bd803eec9a46346b3f0d8e8fc3e562629c65e79c))

## 0.10.1 - 2026-07-27

- Preserved the last release from the legacy development line as the release
  baseline for the new Projects-focused workspace.
