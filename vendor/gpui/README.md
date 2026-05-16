# Vendored GPUI Closure

Source: zed/

Source revision: 3bd9d13b63fc5a5ffa39326597bc4fd91adc82d1

| Crate | Source | License |
| --- | --- | --- |
| collections | crates/collections | Apache-2.0 |
| derive_refineable | crates/refineable/derive_refineable | Apache-2.0 |
| gpui | crates/gpui | Apache-2.0 |
| gpui_linux | crates/gpui_linux | Apache-2.0 |
| gpui_macos | crates/gpui_macos | Apache-2.0 |
| gpui_macros | crates/gpui_macros | Apache-2.0 |
| gpui_platform | crates/gpui_platform | Apache-2.0 |
| gpui_shared_string | crates/gpui_shared_string | Apache-2.0 |
| gpui_util | crates/gpui_util | Apache-2.0 |
| gpui_web | crates/gpui_web | Apache-2.0 |
| gpui_wgpu | crates/gpui_wgpu | Apache-2.0 |
| gpui_windows | crates/gpui_windows | Apache-2.0 |
| http_client | crates/http_client | Apache-2.0 |
| http_client_tls | crates/http_client_tls | Apache-2.0 |
| media | crates/media | Apache-2.0 |
| perf | tooling/perf | Apache-2.0 |
| refineable | crates/refineable | Apache-2.0 |
| reqwest_client | crates/reqwest_client | Apache-2.0 |
| scheduler | crates/scheduler | Apache-2.0 |
| sum_tree | crates/sum_tree | Apache-2.0 |
| util | crates/util | Apache-2.0 |
| util_macros | crates/util_macros | Apache-2.0 |
| zlog | crates/zlog | GPL-3.0-or-later |
| ztracing | crates/ztracing | GPL-3.0-or-later |
| ztracing_macro | crates/ztracing_macro | GPL-3.0-or-later |

## Workspace Dependency Resolutions

- `windows`: umux previously used `0.62.2`; GPUI/Zed uses `0.61` with the full Zed feature set. The workspace dependency now follows Zed. `cargo check -p umux-win32` passed without adding a crate-local override; `webview2-com` still brings `windows 0.62.2` transitively where it needs it.
- GPUI-facing shared dependencies now follow the Zed workspace specs where they differed from umux: `anyhow = 1.0.86`, `serde = 1.0.221` with `derive` and `rc`, `serde_json = 1.0.144` with `preserve_order` and `raw_value`, `thiserror = 2.0.12`, and `tracing = 0.1.40`.
- `wgpu` follows Zed's fork and is pinned to the Zed-tested `v29` revision `a466bc382ea747f8e1ac810efdb6dcd49a514575` instead of floating on the branch.
- `reqwest_client` and `http_client_tls` are vendored support crates because GPUI dev-dependencies inherit them from the workspace even though the smoke app does not compile those dev targets.
