# Third-party notices

Full, unmodified license texts and copyright notices are in [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).
The source identities, locked archive checksums, exact Git revisions, target graph roles, and hashes of the individual license texts are in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json).

This inventory covers **415** unique packages and **311** distinct license texts for the two release targets.
It includes build tools and proc macros conservatively; it does not assert that all listed packages are linked into the executable.

## Embedded Rune assets

| Component | Original license | Pinned source |
| --- | --- | --- |
| Rune-MIT.txt | [Rune-MIT.txt](licenses/Rune-MIT.txt) | Rune `e970367bd7d2a51e2e7d58e2978ea32eac7780cb` / `LICENSE` |
| Inter-OFL.txt | [Inter-OFL.txt](licenses/Inter-OFL.txt) | Rune `e970367bd7d2a51e2e7d58e2978ea32eac7780cb` / `crates/rune/assets/Inter-LICENSE.txt` |
| IBMPlexMono-OFL.txt | [IBMPlexMono-OFL.txt](licenses/IBMPlexMono-OFL.txt) | Rune `e970367bd7d2a51e2e7d58e2978ea32eac7780cb` / `crates/rune/assets/IBMPlexMono-LICENSE.txt` |
| Rune-highlights-Apache.txt | [Rune-highlights-Apache.txt](licenses/Rune-highlights-Apache.txt) | Rune `e970367bd7d2a51e2e7d58e2978ea32eac7780cb` / `crates/rune-highlights/vendor/LICENSE.md` |

## Resolved package inventory

Normal = normal dependency path. Host = build dependency or proc-macro path. A package can have both roles.

| Package | Version | Declared license expression | Linux | Windows |
| --- | --- | --- | --- | --- |
| accesskit | 0.25.2 | MIT OR Apache-2.0 | Normal | Normal |
| accesskit_atspi_common | 0.21.1 | MIT OR Apache-2.0 | Normal | — |
| accesskit_consumer | 0.39.2 | MIT OR Apache-2.0 | Normal | Normal |
| accesskit_unix | 0.24.1 | MIT OR Apache-2.0 | Normal | — |
| accesskit_windows | 0.35.2 | MIT OR Apache-2.0 | — | Normal |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | Normal | Normal |
| aead | 0.5.2 | MIT OR Apache-2.0 | Normal | Normal |
| aead | 0.6.1 | MIT OR Apache-2.0 | Normal | Normal |
| ahash | 0.8.12 | MIT OR Apache-2.0 | Normal | — |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | Normal | Normal |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | Normal | Normal |
| arboard | 3.7.0 | MIT OR Apache-2.0 | Normal | Normal |
| argon2 | 0.5.3 | MIT OR Apache-2.0 | Normal | Normal |
| argon2 | 0.6.0 | MIT OR Apache-2.0 | Normal | Normal |
| arrayref | 0.3.9 | BSD-2-Clause | Normal | Normal |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | Normal | Normal |
| as-raw-xcb-connection | 1.0.1 | MIT OR Apache-2.0 | Normal | — |
| ash | 0.38.0+1.3.281 | MIT OR Apache-2.0 | Normal | Normal |
| async-broadcast | 0.7.2 | MIT OR Apache-2.0 | Normal | — |
| async-channel | 2.5.0 | Apache-2.0 OR MIT | Normal | — |
| async-executor | 1.14.1 | Apache-2.0 OR MIT | Normal | — |
| async-io | 2.6.0 | Apache-2.0 OR MIT | Normal | — |
| async-lock | 3.5.0 | Apache-2.0 OR MIT | Normal | — |
| async-process | 2.5.0 | Apache-2.0 OR MIT | Normal | — |
| async-recursion | 1.2.0 | MIT OR Apache-2.0 | Host | — |
| async-signal | 0.2.14 | Apache-2.0 OR MIT | Normal | — |
| async-task | 4.7.1 | Apache-2.0 OR MIT | Normal | — |
| async-trait | 0.1.92 | MIT OR Apache-2.0 | Host | — |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | Normal | Normal |
| atspi-common | 0.13.0 | Apache-2.0 OR MIT | Normal | — |
| atspi-proxies | 0.13.0 | Apache-2.0 OR MIT | Normal | — |
| atspi | 0.29.0 | Apache-2.0 OR MIT | Normal | — |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | Host | Host |
| base64 | 0.22.1 | MIT OR Apache-2.0 | Normal | Normal |
| base64 | 0.23.1 | MIT OR Apache-2.0 | Normal | Normal |
| base64ct | 1.8.3 | Apache-2.0 OR MIT | Normal | Normal |
| bit-set | 0.10.0 | Apache-2.0 OR MIT | Normal | Normal |
| bit-vec | 0.9.1 | Apache-2.0 OR MIT | Normal | Normal |
| bitflags | 1.3.2 | MIT/Apache-2.0 | Normal | Normal |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | Host, Normal | Host, Normal |
| blake2 | 0.10.6 | MIT OR Apache-2.0 | Normal | Normal |
| blake2 | 0.11.0 | MIT OR Apache-2.0 | Normal | Normal |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | Normal | Normal |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | Normal | Normal |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | Normal | Normal |
| blocking | 1.7.0 | Apache-2.0 OR MIT | Normal | — |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | Normal | Normal |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT | Host | Host |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | Normal | Normal |
| bytes | 1.12.1 | MIT | Normal | Normal |
| calloop-wayland-source | 0.3.0 | MIT | Normal | — |
| calloop | 0.13.0 | MIT | Normal | — |
| cc | 1.11.1 | MIT OR Apache-2.0 | Host | Host |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | Normal | Host, Normal |
| cfg_aliases | 0.2.2 | MIT | Host | Host |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | Normal | Normal |
| chacha20 | 0.9.1 | Apache-2.0 OR MIT | Normal | Normal |
| chacha20poly1305 | 0.10.1 | Apache-2.0 OR MIT | Normal | Normal |
| chacha20poly1305 | 0.11.0 | Apache-2.0 OR MIT | Normal | Normal |
| chrono | 0.4.45 | MIT OR Apache-2.0 | Normal | Normal |
| cipher | 0.4.4 | MIT OR Apache-2.0 | Normal | Normal |
| cipher | 0.5.2 | MIT OR Apache-2.0 | Normal | Normal |
| clipboard-win | 5.4.3 | BSL-1.0 | — | Normal |
| cmov | 0.5.4 | Apache-2.0 OR MIT | Normal | Normal |
| codespan-reporting | 0.13.1 | Apache-2.0 | Normal | Normal |
| color_quant | 1.1.0 | MIT | Normal | Normal |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT | Normal | — |
| constant_time_eq | 0.4.2 | CC0-1.0 OR MIT-0 OR Apache-2.0 | Normal | Normal |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | Normal | Normal |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | Normal | Normal |
| crc32fast | 1.5.4 | MIT OR Apache-2.0 | Normal | Normal |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | Normal | — |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | Normal | Normal |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | Normal | Normal |
| ctutils | 0.4.8 | Apache-2.0 OR MIT | Normal | Normal |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib | Normal | Normal |
| data-url | 0.3.2 | MIT OR Apache-2.0 | Normal | Normal |
| digest | 0.10.7 | MIT OR Apache-2.0 | Normal | Normal |
| digest | 0.11.3 | MIT OR Apache-2.0 | Normal | Normal |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | Host | Host |
| dlib | 0.5.3 | MIT | Normal | — |
| document-features | 0.2.12 | MIT OR Apache-2.0 | Host | Host |
| downcast-rs | 1.2.1 | MIT/Apache-2.0 | Normal | — |
| dpi | 0.1.2 | Apache-2.0 AND MIT | Normal | Normal |
| endi | 1.1.1 | MIT | Host, Normal | — |
| enumflags2 | 0.7.12 | MIT OR Apache-2.0 | Host, Normal | — |
| enumflags2_derive | 0.7.12 | MIT OR Apache-2.0 | Host | — |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | Host, Normal | Host, Normal |
| errno | 0.3.14 | MIT OR Apache-2.0 | Normal | — |
| error-code | 3.4.0 | BSL-1.0 | — | Normal |
| euclid | 0.22.14 | MIT OR Apache-2.0 | Normal | Normal |
| event-listener-strategy | 0.5.4 | Apache-2.0 OR MIT | Normal | — |
| event-listener | 5.4.2 | Apache-2.0 OR MIT | Normal | — |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | Host, Normal | Normal |
| fax | 0.2.7 | MIT | Normal | Normal |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | Normal | Normal |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 | Host | Host |
| fixedbitset | 0.5.9 | MIT OR Apache-2.0 | Normal | — |
| flate2 | 1.1.11 | MIT OR Apache-2.0 | Normal | Normal |
| float-cmp | 0.9.0 | MIT | Normal | Normal |
| foldhash | 0.1.5 | Zlib | Normal | — |
| foldhash | 0.2.0 | Zlib | Host, Normal | Host, Normal |
| font-types | 0.12.7 | MIT OR Apache-2.0 | Normal | Normal |
| fontique | 0.12.0 | Apache-2.0 OR MIT | Normal | Normal |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | Normal | Normal |
| fs2 | 0.4.3 | MIT/Apache-2.0 | Normal | Normal |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 | Normal | Normal |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | Normal | Normal |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | Normal | Normal |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT | Normal | — |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | Host | Host |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 | Normal | Normal |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | Normal | Normal |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | Normal | Normal |
| generic-array | 0.14.7 | MIT | Normal | Normal |
| gethostname | 1.1.0 | Apache-2.0 | Normal | — |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | Normal | Normal |
| getrandom | 0.3.4 | MIT OR Apache-2.0 | Normal | — |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | Normal | Host, Normal |
| gif | 0.14.2 | MIT OR Apache-2.0 | Normal | Normal |
| gpu-allocator | 0.28.0 | MIT OR Apache-2.0 | Normal | Normal |
| half | 2.7.1 | MIT OR Apache-2.0 | Normal | Normal |
| harfrust | 0.12.0 | MIT | Normal | Normal |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 | Normal | — |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | Normal | Normal |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | Host, Normal | Host, Normal |
| hex | 0.4.3 | MIT OR Apache-2.0 | Normal | Normal |
| http-body-util | 0.1.5 | MIT | Normal | Normal |
| http-body | 1.1.0 | MIT | Normal | Normal |
| http | 1.5.0 | MIT OR Apache-2.0 | Normal | Normal |
| httparse | 1.10.1 | MIT OR Apache-2.0 | Normal | Normal |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | Normal | Normal |
| hyper-rustls | 0.27.10 | Apache-2.0 OR ISC OR MIT | Normal | Normal |
| hyper-util | 0.1.21 | MIT | Normal | Normal |
| hyper | 1.12.0 | MIT | Normal | Normal |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 | Normal | — |
| icu_collections | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_locale_core | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_locale_fallback | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_locale_fallback_data | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_normalizer | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_normalizer_data | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_properties | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_properties_data | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_provider | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_segmenter | 2.4.0 | Unicode-3.0 | Normal | Normal |
| icu_segmenter_data | 2.4.0 | Unicode-3.0 | Normal | Normal |
| idna | 1.1.0 | MIT OR Apache-2.0 | Normal | Normal |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | Normal | Normal |
| image-webp | 0.2.4 | MIT OR Apache-2.0 | Normal | Normal |
| image | 0.25.10 | MIT OR Apache-2.0 | Normal | Normal |
| imagesize | 0.13.0 | MIT | Normal | Normal |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | Host, Normal | Host, Normal |
| inout | 0.1.4 | MIT OR Apache-2.0 | Normal | Normal |
| inout | 0.2.6 | MIT OR Apache-2.0 | Normal | Normal |
| ipnet | 2.12.2 | MIT OR Apache-2.0 | Normal | Normal |
| itoa | 1.0.19 | MIT OR Apache-2.0 | Normal | Normal |
| jobserver | 0.1.35 | MIT OR Apache-2.0 | Host | Host |
| kurbo | 0.11.3 | Apache-2.0 OR MIT | Normal | Normal |
| libc | 0.2.191 | MIT OR Apache-2.0 | Host, Normal | Normal |
| libloading | 0.8.9 | ISC | Normal | Normal |
| libm | 0.2.16 | MIT | Normal | Normal |
| linebender_resource_handle | 0.1.1 | Apache-2.0 OR MIT | Normal | Normal |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Normal | — |
| linux-raw-sys | 0.4.15 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Normal | — |
| litemap | 0.8.3 | Unicode-3.0 | Normal | Normal |
| litrs | 1.0.1 | MIT OR Apache-2.0 | Host | Host |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | Normal | Normal |
| log | 0.4.35 | MIT OR Apache-2.0 | Normal | Normal |
| lru-slab | 0.1.3 | MIT OR Apache-2.0 OR Zlib | Normal | Normal |
| memchr | 2.8.3 | Unlicense OR MIT | Host, Normal | Normal |
| memmap2 | 0.9.12 | MIT OR Apache-2.0 | Normal | Normal |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | Normal | Normal |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | Normal | Normal |
| mio | 1.2.4 | MIT | Normal | Normal |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | Normal | Normal |
| naga-types | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| naga | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| nom | 8.0.0 | MIT | Normal | — |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | Normal | Normal |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | Normal | Normal |
| opaque-debug | 0.3.1 | MIT OR Apache-2.0 | Normal | Normal |
| ordered-float | 5.5.1 | MIT | Normal | Normal |
| ordered-stream | 0.2.0 | MIT OR Apache-2.0 | Normal | — |
| os_pipe | 1.2.3 | MIT | Normal | — |
| parking | 2.2.1 | Apache-2.0 OR MIT | Normal | — |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | Normal | Normal |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | Normal | Normal |
| parlance | 0.1.1 | Apache-2.0 OR MIT | Normal | Normal |
| parley | 0.12.0 | Apache-2.0 OR MIT | Normal | Normal |
| parley_data | 0.12.0 | Apache-2.0 OR MIT | Normal | Normal |
| parley_emoji | 0.12.0 | MIT | Normal | Normal |
| parley_engine | 0.12.0 | Apache-2.0 OR MIT | Normal | Normal |
| password-hash | 0.5.0 | MIT OR Apache-2.0 | Normal | Normal |
| password-hash | 0.6.1 | MIT OR Apache-2.0 | Normal | Normal |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | Normal | Normal |
| petgraph | 0.8.3 | MIT OR Apache-2.0 | Normal | — |
| phc | 0.6.1 | Apache-2.0 OR MIT | Normal | Normal |
| phf | 0.14.0 | MIT | Normal | — |
| phf_generator | 0.14.0 | MIT | Host | — |
| phf_macros | 0.14.0 | MIT | Host | — |
| phf_shared | 0.14.0 | MIT | Host, Normal | — |
| pico-args | 0.5.0 | MIT | Normal | Normal |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | Normal | Normal |
| piper | 0.2.5 | MIT OR Apache-2.0 | Normal | — |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | Host | — |
| png | 0.17.16 | MIT OR Apache-2.0 | Normal | Normal |
| png | 0.18.1 | MIT OR Apache-2.0 | Normal | Normal |
| polling | 3.11.0 | Apache-2.0 OR MIT | Normal | — |
| pollster | 1.0.1 | Apache-2.0/MIT | Normal | Normal |
| poly1305 | 0.8.0 | Apache-2.0 OR MIT | Normal | Normal |
| poly1305 | 0.9.1 | Apache-2.0 OR MIT | Normal | Normal |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT | Normal | Normal |
| potential_utf | 0.1.6 | Unicode-3.0 | Normal | Normal |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | Normal | Normal |
| presser | 0.3.1 | MIT OR Apache-2.0 | Normal | Normal |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | Host | — |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | Host, Normal | Host |
| profiling | 1.0.18 | MIT OR Apache-2.0 | Normal | Normal |
| pulldown-cmark | 0.13.6 | MIT | Normal | Normal |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | Normal | Normal |
| quick-error | 2.0.1 | MIT/Apache-2.0 | Normal | Normal |
| quick-xml | 0.41.0 | MIT | Host | — |
| quinn-proto | 0.11.19 | MIT OR Apache-2.0 | Normal | Normal |
| quinn-udp | 0.5.16 | MIT OR Apache-2.0 | Normal | Normal |
| quinn | 0.11.12 | MIT OR Apache-2.0 | Normal | Normal |
| quote | 1.0.47 | MIT OR Apache-2.0 | Host, Normal | Host |
| rand | 0.10.3 | MIT OR Apache-2.0 | Normal | Normal |
| rand | 0.8.8 | MIT OR Apache-2.0 | Normal | Normal |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | Normal | Normal |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | Normal | Host, Normal |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | Normal | Normal |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 | Normal | Normal |
| range-alloc | 0.1.5 | MIT OR Apache-2.0 | — | Normal |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | Normal | Normal |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 | Normal | Normal |
| renderdoc-sys | 1.1.0 | MIT OR Apache-2.0 | Normal | Normal |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | Normal | Normal |
| resvg | 0.45.1 | Apache-2.0 OR MIT | Normal | Normal |
| rfd | 0.17.2 | MIT | — | Normal |
| rgb | 0.8.53 | MIT | Normal | Normal |
| ring | 0.17.14 | Apache-2.0 AND ISC | Normal | Normal |
| roxmltree | 0.20.0 | MIT OR Apache-2.0 | Normal | Normal |
| rune-highlights | 0.0.1 | MIT AND Apache-2.0 | Normal | Normal |
| rune-kit | 0.0.1 | MIT | Normal | Normal |
| rune-motion | 0.1.0 | MIT | Normal | Normal |
| rune-staging | 0.0.1 | MIT | Normal | Normal |
| rune-taffy | 0.0.1 | MIT | Normal | Normal |
| rune | 0.0.1 | MIT | Normal | Normal |
| rustc-hash | 1.1.0 | Apache-2.0/MIT | Normal | Normal |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | Normal | Normal |
| rustix | 0.38.44 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Normal | — |
| rustix | 1.1.6 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Normal | — |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | Normal | Normal |
| rustls-webpki | 0.103.16 | ISC | Normal | Normal |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | Normal | Normal |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | Normal | Normal |
| scoped-tls | 1.0.1 | MIT/Apache-2.0 | Normal | — |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | Normal | Normal |
| sctk-adwaita | 0.10.1 | MIT | Normal | — |
| serde | 1.0.229 | MIT OR Apache-2.0 | Host, Normal | Normal |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | Host, Normal | Normal |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | Host | Host |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | Normal | Normal |
| serde_repr | 0.1.21 | MIT OR Apache-2.0 | Host | — |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 | Normal | Normal |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | Normal | Normal |
| shlex | 2.0.1 | MIT OR Apache-2.0 | Host | Host |
| signal-hook-registry | 1.4.8 | MIT OR Apache-2.0 | Normal | — |
| simd-adler32 | 0.3.10 | MIT | Normal | Normal |
| simplecss | 0.2.2 | Apache-2.0 OR MIT | Normal | Normal |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | Host, Normal | Normal |
| skrifa | 0.44.0 | MIT OR Apache-2.0 | Normal | Normal |
| slab | 0.4.12 | MIT | Normal | Normal |
| slotmap | 1.1.1 | Zlib | Normal | Normal |
| smallvec | 1.16.5 | MIT OR Apache-2.0 | Normal | Normal |
| smithay-client-toolkit | 0.19.2 | MIT | Normal | — |
| smol_str | 0.2.2 | MIT OR Apache-2.0 | Normal | Normal |
| sn-rune-core | 0.1.0-alpha.1 | AGPL-3.0-only | Normal | Normal |
| sn-rune-protocol | 0.1.0-alpha.1 | AGPL-3.0-only | Normal | Normal |
| socket2 | 0.6.5 | MIT OR Apache-2.0 | Normal | Normal |
| spirv | 0.4.0+sdk-1.4.341.0 | Apache-2.0 | Normal | Normal |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | Normal | Normal |
| standardnotes-rune | 0.1.0-alpha.1 | AGPL-3.0-only | Normal | Normal |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | Normal | Normal |
| strict-num | 0.1.1 | MIT | Normal | Normal |
| subtle | 2.6.1 | BSD-3-Clause | Normal | Normal |
| svgtypes | 0.15.3 | Apache-2.0 OR MIT | Normal | Normal |
| swash | 0.2.10 | Apache-2.0 OR MIT | Normal | Normal |
| syn | 2.0.119 | MIT OR Apache-2.0 | Host | Host |
| syn | 3.0.10 | MIT OR Apache-2.0 | Host, Normal | Host |
| sync_wrapper | 1.0.2 | Apache-2.0 | Normal | Normal |
| synstructure | 0.14.0 | MIT | Host | Host |
| taffy | 0.15.1 | MIT | Normal | Normal |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | Normal | Normal |
| termcolor | 1.4.1 | Unlicense OR MIT | Normal | Normal |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | Host | — |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | Host | Host |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | Normal | — |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | Normal | Normal |
| tiff | 0.11.3 | MIT | Normal | Normal |
| tiny-skia-path | 0.11.4 | BSD-3-Clause | Normal | Normal |
| tiny-skia | 0.11.4 | BSD-3-Clause | Normal | Normal |
| tinystr | 0.8.5 | Unicode-3.0 | Normal | Normal |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | Normal | Normal |
| tokio-macros | 2.7.2 | MIT | Host | Host |
| tokio-rustls | 0.26.6 | MIT OR Apache-2.0 | Normal | Normal |
| tokio | 1.54.1 | MIT | Normal | Normal |
| toml_datetime | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | Host | — |
| toml_edit | 0.25.19+spec-1.1.0 | MIT OR Apache-2.0 | Host | — |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | Host | — |
| tower-http | 0.6.11 | MIT | Normal | Normal |
| tower-layer | 0.3.3 | MIT | Normal | Normal |
| tower-service | 0.3.3 | MIT | Normal | Normal |
| tower | 0.5.3 | MIT | Normal | Normal |
| tracing-attributes | 0.1.31 | MIT | Host | Host |
| tracing-core | 0.1.36 | MIT | Normal | Normal |
| tracing | 0.1.44 | MIT | Normal | Normal |
| tree_magic_mini | 3.2.2 | MIT | Normal | — |
| try-lock | 0.2.5 | MIT | Normal | Normal |
| typenum | 1.20.1 | MIT OR Apache-2.0 | Normal | Normal |
| unicase | 2.10.0 | MIT OR Apache-2.0 | Normal | Normal |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | Host, Normal | Host, Normal |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | Normal | Normal |
| unicode-width | 0.2.3 | MIT OR Apache-2.0 | Normal | Normal |
| universal-hash | 0.5.1 | MIT OR Apache-2.0 | Normal | Normal |
| universal-hash | 0.6.1 | MIT OR Apache-2.0 | Normal | Normal |
| untrusted | 0.9.0 | ISC | Normal | Normal |
| url | 2.5.8 | MIT OR Apache-2.0 | Normal | Normal |
| usvg | 0.45.1 | Apache-2.0 OR MIT | Normal | Normal |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT | Normal | Normal |
| utf_types | 1.0.0 | Apache-2.0 OR MIT | Normal | Normal |
| uuid | 1.29.2 | Apache-2.0 OR MIT | Normal | Normal |
| version_check | 0.9.5 | MIT/Apache-2.0 | Host | Host |
| want | 0.3.2 | MIT | Normal | Normal |
| wasmparser | 0.245.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Host | Host |
| wayland-backend | 0.3.17 | MIT | Normal | — |
| wayland-client | 0.31.15 | MIT | Normal | — |
| wayland-csd-frame | 0.3.0 | MIT | Normal | — |
| wayland-cursor | 0.31.14 | MIT | Normal | — |
| wayland-protocols-plasma | 0.3.12 | MIT | Normal | — |
| wayland-protocols-wlr | 0.3.12 | MIT | Normal | — |
| wayland-protocols | 0.32.13 | MIT | Normal | — |
| wayland-scanner | 0.31.11 | MIT | Host | — |
| wayland-sys | 0.31.11 | MIT | Normal | — |
| webpki-roots | 1.0.11 | CDLA-Permissive-2.0 | Normal | Normal |
| weezl | 0.1.12 | MIT OR Apache-2.0 | Normal | Normal |
| wgpu-core-deps-windows-linux-android | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| wgpu-core | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| wgpu-hal | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| wgpu-naga-bridge | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| wgpu-types | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| wgpu | 30.0.1 | MIT OR Apache-2.0 | Normal | Normal |
| winapi-util | 0.1.11 | Unlicense OR MIT | — | Normal |
| winapi | 0.3.9 | MIT/Apache-2.0 | — | Normal |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | — | Normal |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | — | Normal |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | — | Normal |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | — | Host |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | — | Host |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | — | Normal |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | — | Normal |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | — | Normal |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | — | Normal |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | — | Normal |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | — | Normal |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | — | Normal |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | — | Normal |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | — | Normal |
| windows | 0.62.2 | MIT OR Apache-2.0 | — | Normal |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | — | Normal |
| winit | 0.30.13 | Apache-2.0 | Normal | Normal |
| winnow | 1.0.5 | MIT | Host, Normal | — |
| wl-clipboard-rs | 0.9.5 | MIT OR Apache-2.0 | Normal | — |
| writeable | 0.6.4 | Unicode-3.0 | Normal | Normal |
| x11-dl | 2.21.0 | MIT | Normal | — |
| x11rb-protocol | 0.13.2 | MIT OR Apache-2.0 | Normal | — |
| x11rb | 0.13.2 | MIT OR Apache-2.0 | Normal | — |
| xcursor | 0.3.11 | MIT | Normal | — |
| xkbcommon-dl | 0.4.2 | MIT | Normal | — |
| xkbcommon | 0.9.0 | MIT | Normal | — |
| xkeysym | 0.2.1 | MIT OR Apache-2.0 OR Zlib | Normal | — |
| xmlwriter | 0.1.0 | MIT | Normal | Normal |
| yazi | 0.2.2 | Apache-2.0 OR MIT | Normal | Normal |
| yeslogic-fontconfig-sys | 6.0.1 | MIT | Normal | — |
| yoke-derive | 0.8.4 | Unicode-3.0 | Host | Host |
| yoke | 0.8.4 | Unicode-3.0 | Normal | Normal |
| zbus-lockstep-macros | 0.5.2 | MIT | Host | — |
| zbus-lockstep | 0.5.2 | MIT | Host, Normal | — |
| zbus | 5.19.0 | MIT | Normal | — |
| zbus | 6.2.1 | MIT | Normal | — |
| zbus_macros | 5.19.0 | MIT | Host | — |
| zbus_macros | 6.2.1 | MIT | Host | — |
| zbus_names | 4.3.5 | MIT | Host, Normal | — |
| zbus_utils | 6.1.0 | MIT | Host, Normal | — |
| zbus_xml | 5.2.1 | MIT | Host, Normal | — |
| zcheapstr | 1.2.0 | MIT | Host, Normal | — |
| zeno | 0.3.3 | Apache-2.0 OR MIT | Normal | Normal |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | Host | Host |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | Normal | Normal |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | Host | Host |
| zerofrom | 0.1.8 | Unicode-3.0 | Normal | Normal |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | Normal | Normal |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT | Host | Host |
| zerotrie | 0.2.5 | Unicode-3.0 | Normal | Normal |
| zerovec-derive | 0.11.7 | Unicode-3.0 | Host | Host |
| zerovec | 0.11.9 | Unicode-3.0 | Normal | Normal |
| zlib-rs | 0.6.8 | Zlib | Normal | Normal |
| zmij | 1.0.23 | MIT | Normal | Normal |
| zruntime | 0.2.2 | MIT | Normal | — |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | Normal | Normal |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib | Normal | Normal |
| zvariant | 5.15.1 | MIT | Host, Normal | — |
| zvariant_derive | 5.15.1 | MIT | Host | — |
| zvariant_utils | 4.2.0 | MIT | Host, Normal | — |

## Scope

System libraries and the Windows SDK/CRT are not Cargo packages and are not represented as crate licenses in this inventory. Binary build provenance must record their platform/toolchain versions separately.
Source code for the application remains under its repository license. The individual upstream terms above remain applicable to each dependency or embedded asset.
