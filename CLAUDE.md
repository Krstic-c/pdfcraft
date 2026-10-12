# PdfCraft — instructions for agents

PdfCraft is a clean-room, open-source, Rust-native PDF application targeting Adobe Acrobat Pro parity. It runs natively on macOS, Windows, Linux and FreeBSD, and on the web via WASM. It is the sibling of `../photocraft` (a Photoshop-class editor) and follows the same conventions.

## Start every session here
1. Read `plan/STATUS.md`: the current milestone, the next unchecked task and any blockers. `ROADMAP.md` holds the milestone estimates and progress; update its table and log at the end of every session.
   - Read `ROADMAP.md` §Honest assessment and §Where we're lacking and where we're going before choosing work. They rank the gaps (own renderer, hardening, fidelity against Acrobat, editing existing content, Pro workflows, 1.0 polish); prefer them over new P2/P3 features, and keep both sections true when things change.
   - "Shipped" in `parity/` means "exists and tested", not "as good as Acrobat". Don't mark a feature shipped on a generic test, and say in its notes what is still missing.
2. Read that task in `plan/execution-plan.md` §3, the relevant section of `plan/architecture.md`, and the README of the crate you're touching.
3. Follow the **autonomous operation protocol** in `plan/execution-plan.md` §7 (orient → plan → implement + test → verify → record → commit). Don't stop to ask unless §7 lists the decision as the user's.

`plan/` is gitignored (local-only, like PhotoCraft). The machine-readable parity checklist lives in `parity/` (committed).

## Non-negotiables
- **Assets: read `AGENTS.md` §1 before adding or showing any icon, image, font or document.** No assets from Adobe products, ever. Only openly licensed or contributor-original assets are allowed, each with an entry in `ATTRIBUTION.toml`. `cargo xtask assets` enforces this. `AGENTS.md` overrides this file.
- **Fonts live in [storytold/craft-fonts](https://github.com/storytold/craft-fonts).** Never commit font files here (`AGENTS.md` §1.4; team members: [craftrules `standards/fonts.md`](https://github.com/storytold/craftrules/blob/main/standards/fonts.md), internal). It is the optional build input `CRAFT_FONTS_DIR`: `git clone https://github.com/storytold/craft-fonts ../craft-fonts && CRAFT_FONTS_DIR=../craft-fonts cargo test --workspace` embeds the Japanese fonts (UI fallback, Japanese text in edited PDFs) and runs their tests, which otherwise skip. Code using `pdfcraft_fonts::CRAFT_FONTS` must work when it is empty.
- **Clean-room.**
  - Never read, disassemble or copy anything inside the Acrobat bundle (names and listings only). **Never open `Contents/Resources/JavaScripts/`.**
  - Behaviour comes from public docs, specs (ISO 32000-2, the Arlington model) and black-box observation (`plan/acrobat/`).
  - Never copy GPL/AGPL code. MuPDF, Ghostscript, Poppler, veraPDF and DSS run only as external oracle processes.
  - See `plan/README.md` §Clean-room and `plan/adr/0001`.
- **Privacy.** When observing Acrobat, use synthetic fixtures only. Never capture the Home view, recent files or account info. Capture by window id (`plan/acrobat/tools/`). Never commit Acrobat outputs, corpus files or personal data.
- **Layering.** Nothing below L7 depends on egui/winit/eframe/rfd (`plan/architecture.md` §3). `cos`/`filters`/`crypt`/`arlington` stay standalone.
- **Fidelity.** The PDF object graph is the model. Preserve unknown data. Saves are incremental unless a full rewrite is required. Never silently drop or repair data without recording it.
- **Never panic.** Every PDF, script, MCP call, settings file and keystroke is untrusted input, and none of it may crash the app or lose the user's work. This outranks feature work. Return `Result` (the crate's error enum) and propagate with `?`, or fall back leniently and record it. In non-test code:
  - No `unwrap`/`expect`/`panic!`/`unreachable!`/`todo!` unless it is provably infallible, with a comment saying why.
  - No indexing or slicing with input-derived positions (use `get`, and slice strings only at char boundaries).
  - Use checked or saturating arithmetic on input-derived numbers, and guard against division by zero and NaN/inf casts.
  - Cap allocations sized by input, and bound recursion with depth limits or seen-sets.
  - Handle lock poisoning.
  - No `unsafe` (`unsafe_code = "forbid"`).
  - Every crash fix gets a synthetic regression test. See `AGENTS.md` §4 (and, for team members, the internal `craftrules/standards/never-crash.md`).
- **Rust only** in the product and build (`xtask`). No handwritten JS/TS.
- **Quality gates** before every commit: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, and the wasm check once `xtask ci` exists (M0).
- **Commits:** one task id per commit (e.g. `M1.4: xref stream reader`). Commit only green states. End messages with the attribution line required by the environment.

## Running and looking at the app
- `cargo run -p pdfcraft -- <file.pdf>` opens the desktop app.
- `cargo run -p pdfcraft-cli -- run --script steps.json --root DIR` drives the engine headlessly through the automation tools (`pdfcraft-cli tools` lists them). Use it, together with `page_render`, to check engine changes. `pdfcraft-cli mcp` is the opt-in MCP server (AGENTS.md §3).
- `cargo xtask fuzz --time 300` mutation-fuzzes open/render/edit/save in child processes. Findings land in `fuzz-out/findings/` (git-ignored; never commit corpus-derived files). Turn every real finding into a small synthetic regression test before fixing it.
- `cargo xtask parity [--partial]` reports Acrobat-parity progress from `parity/acrobat-features.toml`. Update the entry when a feature ships.
- `cargo xtask demo-pdf` builds `dist/demo/pdfcraft-showcase.pdf` (needs Chrome) for visual checks.
- For UI work, **look at the result**. Either launch `pdfcraft --control FILE doc.pdf` and use `pdfcraft-cli ui --control FILE screenshot --out x.png` (plus `inspect`, `click`, `key`, `type`, `command`, `set`), or take a headless shot with `cargo run -p pdfcraft-ui-egui --example shot`. Compare against `plan/acrobat/02-ui-ux.md`. Control-channel tests use kittest (`crates/ui-egui/tests/control.rs`).
- Parallel agents: use a separate `CARGO_TARGET_DIR` per agent and separate git worktrees.

## Current bootstrap debt (tracked in STATUS.md)
- `pdfcraft-render` renders through the `hayro` crate directly and inspects documents through `lopdf`. Both get replaced by `cos` / `model` / the DisplayList device (M1–M2, ADR-0004).

## 已知的坑（custom 分支，fork 自维护，细节见 CHANGELOG.md）
- **macOS 系统字体路径会变，别写死。** macOS 26 起 `PingFang.ttc` 不在 `/System/Library/Fonts`，而在 `/System/Library/AssetsV2/com_apple_MobileAsset_Font<N>/<hash>.asset/AssetData/`，哈希和 `N` 随系统版本变，`system_fonts.rs` 靠列目录去找。找不到时候选链会**静默**落到下一个字体，不报错，只能靠测试和截图发现。
- **字体集合（`.ttc`）别取"第一个含该字的 face"。** PingFang.ttc 的第 0 个是 HK 变体，STHeiti 的第 0 个是 TC（繁体）。要按 PostScript 名选：`PingFangSC-Regular`、`STHeitiSC-Light`。
- **字体行高要量，别信注释和经验。** PingFang SC 自然行高 1.40 em，Inter 是 1.21 em；egui 的行高取主字体（Inter），所以混排不会变高。改字体兜底后用 `cargo run -p pdfcraft-ui-egui --example shot -- out.png --language zh-hans` 看一眼，不要只看测试绿。
- **测试"ok"可能是被 skip。** `tests/fonts.rs` 里依赖 craft-fonts 或系统字体的用例，条件不满足时直接 `return`，只在 `--nocapture` 下打印 `skipping ...`；没有 `CRAFT_FONTS_DIR` 时 `chinese_ui_text_prefers_the_chinese_face` 等都是 skip。
- **本机（国内网络）构建环境。** `~/.cargo/bin` 不在非登录 shell 的 PATH 里，需 `export PATH="$HOME/.cargo/bin:$PATH"`。依赖用 rsproxy 镜像，不改全局配置、临时传参：`cargo --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' <cmd>`；rustup 装组件用 `RUSTUP_DIST_SERVER=https://rsproxy.cn RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup rustup component add rustfmt clippy`。首次全量编译约 7 分钟。
