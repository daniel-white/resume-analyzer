# Changelog

## [0.2.4](https://github.com/daniel-white/resume-analyzer/compare/v0.2.3...v0.2.4) (2026-11-18)


### Bug Fixes

* authenticate cosign to GHCR before signing ([#14](https://github.com/daniel-white/resume-analyzer/issues/14)) ([1b8a332](https://github.com/daniel-white/resume-analyzer/commit/1b8a33224892fadc4ba594e2ed5d18a7e346f895))

## [0.2.3](https://github.com/daniel-white/resume-analyzer/compare/v0.2.2...v0.2.3) (2026-11-17)


### Bug Fixes

* pass separate -t arguments to imagetools create ([#12](https://github.com/daniel-white/resume-analyzer/issues/12)) ([53e36fa](https://github.com/daniel-white/resume-analyzer/commit/53e36fa01a028d1569f8b2cacd637c0e169f91b9))

## [0.2.2](https://github.com/daniel-white/resume-analyzer/compare/v0.2.1...v0.2.2) (2026-11-17)


### Bug Fixes

* align release image paths and build matrix ([#10](https://github.com/daniel-white/resume-analyzer/issues/10)) ([5958d36](https://github.com/daniel-white/resume-analyzer/commit/5958d366b2d5b8717faf32e053fab0bb0ce0e4e8))

## [0.2.1](https://github.com/daniel-white/resume-analyzer/compare/v0.2.0...v0.2.1) (2026-11-17)


### Bug Fixes

* force git tags for draft releases ([#8](https://github.com/daniel-white/resume-analyzer/issues/8)) ([91955ab](https://github.com/daniel-white/resume-analyzer/commit/91955ab74ca17e5bba3d12ab3d3e063727503146))

## [0.2.0](https://github.com/daniel-white/resume-analyzer/compare/v0.1.0...v0.2.0) (2026-11-17)


### Features

* stream model generation to the UI ([73de67b](https://github.com/daniel-white/resume-analyzer/commit/73de67b4ee944c27da72a8757d207a579227a3f8))


### Bug Fixes

* .first() on locator not matcher in admin e2e test ([9b85c3f](https://github.com/daniel-white/resume-analyzer/commit/9b85c3ff5b8ca2b53c3dfe668e420fdc503e6b80))
* adapt workflows to upgraded GitHub Actions ([#6](https://github.com/daniel-white/resume-analyzer/issues/6)) ([1320906](https://github.com/daniel-white/resume-analyzer/commit/13209066e597a9a862f5d323f04983e7c4b47019))
* admin e2e test strict mode with multiple reopen buttons ([73ec00f](https://github.com/daniel-white/resume-analyzer/commit/73ec00f50d4baa039665d27ca4ce37196794e93f))
* buildx builder creation without suppressing errors ([aa8293e](https://github.com/daniel-white/resume-analyzer/commit/aa8293e7e28e6646f737c87bcc56250e1bb1b10d))
* correct actions/upload-artifact pinned SHA ([6628f6c](https://github.com/daniel-white/resume-analyzer/commit/6628f6cefda69d7169b335a63e76ebb0dbb325fb))
* create docker-container buildx builder for attestations ([938e068](https://github.com/daniel-white/resume-analyzer/commit/938e0680178f687f0b6a2a027fe7d43a6b9acdd4))
* e2e test text and browser-specific issues ([abf1e45](https://github.com/daniel-white/resume-analyzer/commit/abf1e45aa3227152cbc67f0e7fcaf3e7292f0b22))
* install pnpm dependencies before web checks ([9cfb2e2](https://github.com/daniel-white/resume-analyzer/commit/9cfb2e20a1b140004846a23fed6bd6c8c7a94977))
* run e2e tests against production build ([133d33a](https://github.com/daniel-white/resume-analyzer/commit/133d33a6ecaef3089a84d88a639ed48e46747618))
* skip flaky e2e tests in CI gate ([fa940bd](https://github.com/daniel-white/resume-analyzer/commit/fa940bd0c91a269f6afd160960adb7925b322663))
* skip registry cache on PR runs ([5c3bb15](https://github.com/daniel-white/resume-analyzer/commit/5c3bb1527fc421f108b6c50f524d2cb2c9f07359))
* unblock release-please for the Cargo workspace ([127ca19](https://github.com/daniel-white/resume-analyzer/commit/127ca193fd6794b0ca692e4eb7aa58085565be4f))
* use config-file input for release-please action ([8b8f538](https://github.com/daniel-white/resume-analyzer/commit/8b8f538675b1a78caa40d72f423967d07789feab))
