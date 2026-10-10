# CHANGELOG（custom 分支）

（此前的历史未记录，当前状态见 upstream storytold/pdfcraft 及本仓库 commit 历史）

## 2026-10-10

- **fix(fonts): 系统字体兜底改为每个脚本独立槛位**（`3b08f3f`）
  - 起因：issue #728，Windows/macOS 在不内嵌 craft-fonts 的构建下，切换到简体中文界面会显示豆腐块。根因是 `system_fonts.rs` 的兜底机制只有一个槛位，用阿拉伯字母探测，而阿拉伯字体通常不含中文字形。
  - 影响范围：`crates/ui-egui/src/system_fonts.rs`、`theme.rs`、`tests/fonts.rs`。把单槛位机制改成按探测字符分槛位（每种脚本一个），Windows 加了 `msyh.ttc`，macOS 加了中文候选，阿拉伯文兜底保留不变。
  - 验证：本机 macOS 实测通过（Chinese UI 正常显示，无豆腐块），workspace 测试全绿。

- **fix(fonts): macOS 中文兜底从 Hiragino Sans GB 换成 PingFang/STHeiti**（`3f71a41`）
  - 起因：上面那次修复引入了新 bug——Hiragino Sans GB 的行高（1.50 em）比 Inter（1.21 em）大很多，导致中英文混排时英文字母（"PDF"、"Discord" 等）明显下沉，这是 epaint 的行高/基线计算机制导致的（混合字体时用最大行高，再按 valign 居中补偿）。
  - 影响范围：`crates/ui-egui/src/system_fonts.rs`。候选链改为 `PingFang.ttc`（旧路径）→ `STHeiti Light.ttc` → `STHeiti Medium.ttc`，这两个字体的行高天然跟 Inter 接近，不需要额外的 FontTweak 补丁。新增 `MAX_CHINESE_BYTES`（128MB）专门给这几个较大的中文字体集合用，不动全局的 32MB 限制。参考了 upstream 已有的 PR #738 的思路（同样用 PingFang/STHeiti，同样单独开大小上限），但 #738 没有处理行高下沉问题。
  - 验证：本机 macOS 实测对比截图（Hiragino 版本 vs PingFang/STHeiti 版本 vs 官方 release vs PR #738），确认下沉问题消失，workspace 测试全绿。

- **其他**：本地打包了 macOS dmg（ad-hoc 签名，未公证）和 Windows x86/x64/arm64 安装包（GitHub Actions 云端构建），发布在 fork 的 [v0.5.0-custom release](https://github.com/Krstic-c/pdfcraft/releases/tag/v0.5.0-custom)。fork 分支清理为只保留 `main` + `custom` 两个分支。
