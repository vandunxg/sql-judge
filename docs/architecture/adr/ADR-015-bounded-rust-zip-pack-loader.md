# ADR-015: Bounded Rust ZIP Pack Loader

- Status: Accepted
- Date: 2026-10-12
- Requirements: REQ-009, REQ-010
- Acceptance: AC-008, AC-009
- Security: SEC-006

## Context

Pack import must accept the existing `.sqljudge.zip` artifact without relying
on Python or a platform `unzip` executable. Pack contents are untrusted and
must be bounded before structural or PostgreSQL runtime validation.

## Decision

Use the Rust `zip` crate in `problem-pack` with default features disabled and
only the deflate decoder enabled. `PackData::from_zip` will stream each entry
into bounded memory and reuse the existing path, duplicate, file-count, and
total-byte validation rules before returning `PackData`. ZIP imports also cap
the cumulative compressed payload at 8 MiB before entry allocation. Directory
imports use the same relative path limits and additionally count recursive
filesystem entries before descending.

The loader will reject absolute paths, traversal, symlink and other
non-regular entries, filesystem hardlinks, duplicate or case-colliding names,
unsupported compression, oversized entries, and archives whose
compressed/uncompressed ratio exceeds the defensive limit. ZIP has no portable
inode/link-count representation, so regular ZIP entries are treated as
independent files; duplicate file contents are not treated as hardlinks.
Directory hardlink detection reads the link count from `nlink()` on Unix. On
Windows, `MetadataExt::number_of_links()` is nightly-only, so the loader opens
the file and reads `nNumberOfLinks` through `GetFileInformationByHandle` from
`windows-sys` (Windows-only target dependency, already in both lockfiles via
other crates).
It also rejects relative paths over 512 bytes, paths deeper than 16 components,
archives or directory trees over 4,096 total entries, packs over 2,000 regular
files, and ZIP compressed payloads over 8 MiB. These limits apply before
untrusted directory recursion or ZIP entry allocation proceeds.
The existing `validate()` and transactional runtime validation remain the
authoritative manifest and SQL checks.

## Alternatives Rejected

- Calling the existing Python validator: not guaranteed on packaged clients.
- Calling a system `unzip`: platform-dependent and difficult to constrain.
- Directory-only import: incompatible with the existing `.sqljudge.zip`
  distribution artifact.

## Consequences

- One portable implementation handles desktop pack artifacts.
- The crate adds a reviewed dependency and lockfile entries.
- ZIP-specific negative tests are required before exposing import through Tauri.
- SEC-006 evidence includes deep/long directory paths, filesystem hardlinks,
  excessive directory entries, link-like ZIP metadata, excessive ZIP entries,
  cumulative compressed bytes, and deep ZIP paths in addition to the prior
  traversal, symlink, duplicate, and compression-bomb tests.
