---
created_on: 2026-10-14 18:19
last_modified: 2026-10-14 18:19
status: current
---

# DJ collection interchange

Read `../AGENTS.md` and the full applicable skills before edits. This independent Rust library has no CLI entrypoint or published artifacts.

## Commands

- `just check`: package-scoped format verification, strict Clippy, all-target tests with at least 90% line coverage, doctests, and strict API documentation.
- `just test`: behavioral tests and API doctests.
- `just build`: all targets.
- `just fmt`: format only this package; never use `cargo fmt --all` across independent path dependencies.
- `just doc`: generate local API documentation.

Tests require `ffmpeg` on PATH for real MP3/ID3 fixtures and the sibling `engine-library` checkout for its attributed beat-data fixture. Keep evidence and temporary files under `.tmp`.

## Contracts

- Use `engine-library` native databases, records, schema, and performance codecs directly. Add shared database capabilities there rather than duplicating a SQLite facade here.
- Keep portable times in seconds, slots one-based, beats zero-based, and absent keys and main cues distinct from zero. Preserve local track identity independently of source database identity.
- Require explicit Engine duration units. The interchange options default to seconds as the inspected djtools source does; the shared Engine music services default to milliseconds. Never infer units by magnitude.
- Preserve unchanged nullable native metadata, original timestamp spelling, cue alpha, original grids, unknown binary fields, and opaque caches. Reject changes that cannot be represented safely instead of fabricating native flags or reanalysis.
- Stage the complete Engine database pair before publication. Refuse active SQLite sidecars, preserve unrelated files, and retain rollback failures and backup recovery paths. Never describe two file renames as one crash-atomic transaction.
- Preserve Serato crate membership on per-file audio errors and return explicit warnings. Keep extraction read-only; use native ID3 decoding for text encodings and generate binary/audio fixtures rather than copying externally licensed recordings.
- Keep native XML URLs and DateModified correct, remove nested references when removing tracks, and reject unsupported meters. Use primary format references linked from README when modifying codecs.
- Any runtime behavior change requires a corresponding behavioral test change and at least 90% line coverage; `scripts/` is excluded. Develop red/green and temporarily disable the implementation to prove the tests fail, then restore it exactly and rerun checks.
- Keep standing rules here and progress evidence in `.tmp`; update existing rules in place rather than appending task history.
- Never publish releases, tags, packages, or production deployments automatically without explicit user authorization. Never test against live user collections without explicit authorization for the writes.

The inspected Go source is `../.tmp/sources/djtools`, module `github.com/nateranda/djtools`, fetched from the Alex Gorbatchev fork. Retain Nate Randa's MIT attribution. The Serato layout reference documentation has its own CC BY-SA license; do not copy its audio assets or relicense its documentation as package source.
