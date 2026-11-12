# User Instruction Memory

This file records user instructions, preferences, and teachings for reference in future interactions.

## Format

### User Instruction Entry
User instruction entries should follow this format:

[User Instruction Summary]
- Date: [YYYY-MM-DD]
- Context: [Mentioned scenario or time]
- Instructions:
  - [Content of user teaching or instruction, described line by line]

### Project Knowledge Entry
Entries discovered by the Agent during task execution should follow this format:

[Project Knowledge Summary]
- Date: [YYYY-MM-DD]
- Context: Discovered by Agent while performing [specific task description]
- Category: [Operations & Deployment|Build Methods|Testing Methods|Troubleshooting & Debugging|Workflow & Collaboration|Environment Configuration]
- Instructions:
  - [Specific knowledge points, described line by line]

## Deduplication Strategy
- Before adding a new entry, check for similar or identical instructions.
- If a duplicate is found, skip the new entry or merge it with the existing one.
- When merging, update the context or date information.
- This helps avoid redundant entries and keeps the memory file tidy.

## Entries

[Host loop]
- Date: 2026-11-12
- Context: Discovered by Agent while wiring the ARM IPK recipe
- Category: Build Methods
- Instructions:
  - Daily loop on this x86_64 Linux host is `make sim` / `make check`. `make check` also runs `ci/test_mkipk.py`.
  - Official webOS NDK has no linux-x86_64 asset. `make setup-env` and `make ipk` require macOS or linux-aarch64 (GitHub `ubuntu-24.04-arm`).
  - webosbrew native-toolchain pin is `ci/dependencies.json` entry `webos-ndk` (`webos-d7ed7ee.6`). Default SDK path is `~/webos-ndk/arm-webos-linux-gnueabi_sdk-buildroot`.
  - ARM target is `armv7-unknown-linux-gnueabi` with cortex-a9 / `-neon,-soft-float`. Device binary is C `src/main.c` + `src/starfish.c` linked to `libharbor_app.a`.
  - Packager is `ci/mkipk.py` (common `ar`, no GNU trailing slash). Do not use `ares-package`.
  - Workflow: `.github/workflows/build-ipk.yml`. S912 / QEMU are not builders.
