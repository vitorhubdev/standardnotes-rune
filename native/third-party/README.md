# Third-party source and license inventory

Distribute this directory with the native application. The complete original
license texts, including nested third-party notices and embedded font licenses,
are in `THIRD_PARTY_NOTICES.txt`. The Markdown file is an index, not a replacement
for the complete text file.

`SOURCE_MANIFEST.json` records the versions, Cargo sources, registry archive
checksums, source Git revisions when published, target graph roles, declared
license expressions, and the SHA-256 of every collected license text. Texts are
deduplicated by their exact bytes. Byte offsets in that manifest allow each
original text to be extracted and verified from the aggregate file. Git is
instructed to preserve the bytes in this directory on Windows as well as Linux.

The inventory starts from `standardnotes-rune` and follows normal and build
dependency edges in the locked Cargo metadata for Linux x86_64 and Windows x86_64
MSVC. Dev-only edges are excluded. Build tools and proc macros are explicitly
marked `host-build`; this conservative graph inventory does not claim that every
listed package is linked into the final executable. Cargo metadata features can
be unified across workspace targets. Application code, system libraries, the
Rust toolchain, the Microsoft SDK/CRT, and binary file hashes must also be
identified by the release build's own provenance information.

## Verify the checked-in files

From `native`, with Python 3.11 or newer:

```sh
python scripts/generate-notices.py --verify
```

This checks artifact checksums, each complete original license text, and the
workspace manifests, lockfile, toolchain declaration, and generator that produced
the inventory. It does not contact a network, invoke Cargo, or compile anything.

## Regenerate after dependency changes

```sh
python scripts/generate-notices.py
```

The default uses `cargo metadata --locked --offline --filter-platform` for both
release targets. If required locked crate sources have not yet been downloaded,
use `--online` to allow Cargo to fetch them. No build or tool installation is
performed. Precomputed metadata is also accepted:

```sh
python scripts/generate-notices.py \
  --metadata x86_64-unknown-linux-gnu=/path/to/linux-metadata.json \
  --metadata x86_64-pc-windows-msvc=/path/to/windows-metadata.json
```

`UPSTREAM_LICENSES.json` and `licenses/upstream/` contain the original licenses
missing from several published crate archives. Each file was retrieved from the
exact upstream Git commit in that crate's `.cargo_vcs_info.json`, with its URL,
byte length and SHA-256 recorded. The generator applies such a file only when
both repository and source revision match; it never substitutes a generic SPDX
template for a missing original. If a new dependency has no recoverable license
file, generation fails until its original, revision-pinned license is supplied.

Rune's MIT license, Pierre's Apache-2.0 notice used by `rune-highlights`, and the
Inter / IBM Plex Mono OFL texts are also copied to named files in `licenses/` for
easy inspection. Both MIT and Apache-2.0 texts are retained for Rune Highlights'
`MIT AND Apache-2.0` expression. AccessKit's Chromium-derived source notice and
original copyright headers are included in the complete text file.

This inventory reproduces the upstream license expressions and texts. It does
not replace the application's source-availability obligations or select a
license alternative on behalf of a distributor.
