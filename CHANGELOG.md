# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.4](https://github.com/SichangHe/tokio_gen_server/compare/v0.4.3...v0.4.4) - 2024-06-25

### Other
- *(trait bound)* simplify

## [0.4.3](https://github.com/SichangHe/tokio_gen_server/compare/v0.4.2...v0.4.3) - 2024-06-24

### Added
- feat!(cast/call): only take &self, like interior mutability

## [0.4.2](https://github.com/SichangHe/tokio_gen_server/compare/v0.4.1...v0.4.2) - 2024-06-24

### Added
- *(spawn)* convenience methods for JoinSet

## [0.4.1](https://github.com/SichangHe/tokio_gen_server/compare/v0.4.0...v0.4.1) - 2024-06-18

### Added
- *(relax)* allow `!Send` `Actor` to impl `ActorRunExt` (same for `Bctor`)

### Other
- shut up clippy for `#[test]` in doctest
- *(ci)* distinguish rust checks
- hydra first impressions
- dedup comment in test

## [0.4.0](https://github.com/SichangHe/tokio_gen_server/compare/v0.3.0...v0.4.0) - 2024-05-19

### Fixed
- fix doc generation;merge generation scripts
- fix bctor cancellation

### Other
- relax trait bound so that `Bctor` is object safe
- separate out object-safe `ActorExt`
- make traits ?Sized
- doc for `ActorExt`
- mostly doc-covered
- generation notice
- prevent bctor hang
- clean up bctor test
- compiling bctor but call stalls;doc generation
- half working blocking conversion script

## [0.3.0](https://github.com/SichangHe/tokio_gen_server/compare/v0.2.0...v0.3.0) - 2024-05-19

### Other
- standard test&release-plz CI
- separate out `Actor` doc and duplicate
- return receiver on failure
