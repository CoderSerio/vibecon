<p align="center">
  <img src="./docs/images/logo.png" width="148" alt="VibeCon logo">
</p>

<h1 align="center">VibeCon</h1>

<p align="center">
  把闲置手柄变成一块可观察、可配置的 Vibe Coding 控制台。
</p>

<p align="center">
  <a href="#安装-macos-预览版"><img src="https://img.shields.io/badge/platform-macOS%20%E4%BC%98%E5%85%88-111827?style=flat-square" alt="macOS 优先"></a>
  <a href="#当前能力"><img src="https://img.shields.io/badge/status-%E5%AE%9E%E9%AA%8C%E6%80%A7-F97316?style=flat-square" alt="实验性"></a>
  <a href="#windows"><img src="https://img.shields.io/badge/Windows-%E8%AE%A1%E5%88%92%E4%B8%AD-2563EB?style=flat-square" alt="Windows 计划中"></a>
  <a href="./README.md"><img src="https://img.shields.io/badge/README-English-0AB9E6?style=flat-square" alt="English README"></a>
</p>

<p align="center"><a href="./README.md">English README</a></p>

<p align="center">
  <img src="./docs/images/vibecon-debug-lab.png" width="860" alt="VibeCon Joy-Con 调试界面">
</p>

> **早期原型。** 目前在 macOS 上开发、验证；Tauri + Rust 的结构为跨平台而设计，但 Windows 输入与窗口切换尚未实机验证。

## 安装 macOS 预览版

1. 从 [GitHub Releases](https://github.com/CoderSerio/vibecon/releases) 下载适用于 Apple Silicon Mac 的 `.dmg`。
2. 打开镜像，把 `VibeCon.app` 拖入「应用程序」。
3. 当前预览版使用 ad-hoc 签名，但**还没有 Apple 公证**。macOS 可能提示「Apple 无法验证」。拖入应用程序后，在 Terminal 执行一次：

   ```sh
   xattr -dr com.apple.quarantine "/Applications/VibeCon.app"
   ```

4. 从「应用程序」打开 `VibeCon`。只有启用实验性的窗口切换映射时，才需要再授予辅助功能权限。

这条命令只应对从本仓库下载的 Release 使用。Developer ID 签名与 Apple Notarization 会在之后的公开版本中补上。

## 它是什么？

VibeCon 从一个很朴素的想法开始：与其买一个昂贵又封闭的「Vibe Coding 键盘」，不如把已有的 Joy-Con 或其他手柄，变成一块真正可理解、可检查的物理控制面板。

它不会一上来就自动化。第一步是把输入看清楚：原始 HID 报文、摇杆、按键高亮、采样频率和本地打标；之后再让你主动开启一项足够小、可以审阅的映射。

<p align="center">
  <a href="./docs/media/vibecon-window-switch-demo.mp4">
    <img src="./docs/media/vibecon-window-switch-demo.gif" width="720" alt="Joy-Con 通过 VibeCon 切换窗口">
  </a><br>
  <sub>Joy-Con 实机切换窗口演示。点击查看原始视频。</sub>
</p>

## 当前能力

- 通过 Tauri/Rust 原生 HID 后端发现已配对的 **Joy-Con (L)** 和 **Joy-Con (R)**。
- 可同时选中左右两只手柄；日志按一个时间戳分组，并对齐显示 L/R 两行报文。
- 解码 Joy-Con 原生 `0x30` 与 macOS 紧凑 `0x3F` report，包括按钮 bitfield 和已观测到的八方向 HAT。
- 原创 CSS Joy-Con 可视化：摇杆实时同步、按住高亮、短按有残影提示。
- 日志策略可选：关键操作、旧版 75ms 快照、60/30/10 Hz 采样或每一条 report；也可以随时清空当前可见日志。
- 对抓到的 report 打标：摇杆点位或按键按下/抬起。数据只保存在 `~/.vibecon/annotations.jsonl`，再次命中时会回显标签。
- **基于预设的 macOS 映射：** 可选择 **Code**、**Codex Cowork**、**Inspect Only**，或需要显式开启的 **Keyboard Focus** 实验。前两个预设可用左摇杆向右/向左切换下一个/上一个 `Cmd+Tab` 窗口，并用 Joy-Con (L) 的方向上或 Joy-Con (R) 的 X 聚焦 Codex。Keyboard Focus 会向前台应用发送 Tab / Shift+Tab / Space。每个预设和绑定都需要显式开启，保存在 `~/.vibecon/mappings.json`，并在 Debug 页面临时失效，避免干扰抓包。
- **实验性 Joy-Con 输出：** Mappings 页面提供一个手动、短促的 **测试已选 Joy-Con 震动** 脉冲。它不会被绑定或任务事件自动触发；任何 HID 写入失败都会显示出来且不会重试。

### 已观测到的 Joy-Con 规律

当前 macOS 蓝牙 HID 路径中，紧凑 `0x3F` report 将 Joy-Con (L) 摇杆暴露为八方向 HAT：`0–7` 对应方向，`8` 是中立。按钮字节是 bitmask：应按字节使用位与/位或解析，不能把整条 HEX 当作一个可直接相加的数。

![macOS 下观测到的 Joy-Con HAT report](./docs/images/macos-joycon-hat-debug.png)

## 从源码运行

前置条件：已安装当前版本的 Node.js、pnpm 与 Rust toolchain。

```sh
cd /Users/carbon/Desktop/vibecon
pnpm install
pnpm tauri dev
```

先在 **系统设置 → 蓝牙** 配对 Joy-Con。在 VibeCon 中点击 **Refresh controllers**，选中一个或两个 Joy-Con，然后摇动摇杆或按下按键。

> 不要用 `pnpm dev` 测手柄。它只启动浏览器里的 Vite UI，没有 Tauri Rust 后端，也没有本地 HID 权限。

## 配置 macOS 映射

1. 在 **Debug** 页面选中要使用的手柄。需要两个 Codex 聚焦按键时，左右两侧都选中。
2. 打开 **Mappings**，选择预设，然后启用总开关及所需绑定。**Code** 提供窗口导航；**Codex Cowork** 增加两个聚焦 Codex 的按键；**Inspect Only** 则刻意不向 macOS 发送操作；**Keyboard Focus** 向前台应用发送 Tab / Shift+Tab / Space。该页面会暂停 Debug 的可视化与日志，但 HID reader 仍会保持运行。
3. 窗口切换需要给 VibeCon 授予 **辅助功能** 权限；如果缺失，Mappings 页面可以直接打开对应的系统设置页面。

窗口切换直接通过 macOS Quartz 发送快捷键，因此只需要 **辅助功能** 权限；聚焦 Codex 使用系统应用启动器，不需要辅助功能权限。

### 用 Agent 编辑预设

映射是位于 `~/.vibecon/mappings.json` 的可读 JSON。点击 **Copy Agent Prompt** 可以复制 schema 与安全边界给编码 Agent。VibeCon 只接受已知 Joy-Con 控件，以及 `window_previous`、`window_next`、`focus_codex`、`focus_next`、`focus_previous`、`activate_focused` 这些安全动作；它不会从映射文件执行任意 shell 命令。点击 **Reset defaults** 可恢复四个内置预设。

Keyboard Focus 刻意只做键盘层的焦点导航，不读取或控制完整的可访问性树：它适用于前台应用支持标准 Tab 导航的场景，并受该应用自身焦点规则限制。

## 开发

```sh
pnpm build                    # TypeScript + Vite
cd src-tauri && cargo check   # Rust/Tauri/HID 后端
```

```text
src/App.vue                   Vue 调试与映射界面
src/components/JoyCon.vue     实时 CSS 手柄可视化
src-tauri/src/lib.rs          HID 流、Joy-Con 解码和原生命令
docs/images/                  Logo 与 README 截图
```

## Windows

桌面 UI 与控制器核心没有绑定 Swift 或 macOS API；但当前窗口切换只实现了 macOS，Windows 的 HID 行为也还需要真机测试。Windows 是明确目标，不是当前兼容性承诺。

## 接下来

- Joy-Con、DualSense、Xbox、8BitDo 的 profile 与校准；
- 更多明确、可审阅、按平台申请权限的映射；
- 空间 / 动作输入探索；
- 可导出、可共享的控制器 profile。

## 隐私

手柄 report 与打标都只保留在本地，VibeCon 不发送遥测。它只会执行你明确开启的窗口切换和 Codex 聚焦；不会执行 shell 命令，也不会自动批准 AI Agent 操作。

## License

[MIT](./LICENSE) © 2026 CoderSerio。
