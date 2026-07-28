<p align="center">
  <img src="./docs/images/logo.png" width="148" alt="VibeCon logo">
</p>

<h1 align="center">VibeCon</h1>

<p align="center">
  把闲置手柄变成一块可观察、可配置的 Vibe Coding 控制台。
</p>

<p align="center">
  <a href="#快速开始"><img src="https://img.shields.io/badge/platform-macOS%20%E4%BC%98%E5%85%88-111827?style=flat-square" alt="macOS 优先"></a>
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

## 当前能力

- 通过 Tauri/Rust 原生 HID 后端发现已配对的 **Joy-Con (L)** 和 **Joy-Con (R)**。
- 可同时选中左右两只手柄；日志按一个时间戳分组，并对齐显示 L/R 两行报文。
- 解码 Joy-Con 原生 `0x30` 与 macOS 紧凑 `0x3F` report，包括按钮 bitfield 和已观测到的八方向 HAT。
- 原创 CSS Joy-Con 可视化：摇杆实时同步、按住高亮、短按有残影提示。
- 日志策略可选：关键操作、旧版 75ms 快照、60/30/10 Hz 采样或每一条 report；也可以随时清空当前可见日志。
- 对抓到的 report 打标：摇杆点位或按键按下/抬起。数据只保存在 `~/.vibecon/annotations.jsonl`，再次命中时会回显标签。
- **实验性 macOS 映射：** 在 **Mappings** 页面显式开启后，用左摇杆大幅向右/向左推，触发下一个/上一个 `Cmd+Tab` 窗口；带冷却、需回中复位。开关配置会写入 `~/.vibecon/mapping-settings.json`，但 Debug 页面打开时会临时失效，避免干扰抓包。

### 已观测到的 Joy-Con 规律

当前 macOS 蓝牙 HID 路径中，紧凑 `0x3F` report 将 Joy-Con (L) 摇杆暴露为八方向 HAT：`0–7` 对应方向，`8` 是中立。按钮字节是 bitmask：应按字节使用位与/位或解析，不能把整条 HEX 当作一个可直接相加的数。

![macOS 下观测到的 Joy-Con HAT report](./docs/images/macos-joycon-hat-debug.png)

## 快速开始

### 1. 配对 Joy-Con

1. 将 Joy-Con 从 Switch 拆下并确保有电；
2. 长按滑轨上的小圆形 **Sync** 键，直到玩家指示灯依次闪动；
3. 打开 **系统设置 → 蓝牙**，连接 `Joy-Con (L)` 或 `Joy-Con (R)`；
4. 需要同时调试两侧时，左右各自配对。

如果列表里没有它：重新长按 Sync，并暂时让 Switch 远离或关机，避免它抢先回连。

### 2. 启动原生桌面应用

前置条件：已安装当前版本的 Node.js、pnpm 与 Rust toolchain。

```sh
cd /Users/carbon/Desktop/vibecon
pnpm install
pnpm tauri dev
```

点击 **Refresh controllers**，选中一个或两个 Joy-Con，然后摇动摇杆或按下按键。

> 不要用 `pnpm dev` 测手柄。它只启动浏览器里的 Vite UI，没有 Tauri Rust 后端，也没有本地 HID 权限。

## 开启 macOS 窗口切换权限

实验性映射通过 macOS `System Events` 发送 `Cmd+Tab`，因此必须授予系统权限。

1. 先构建一次 debug App（得到固定的 app bundle）：

   ```sh
   pnpm tauri build --debug
   ```

2. 打开 **系统设置 → 隐私与安全性 → 辅助功能**。
3. 点击 **+**，添加并开启：

   ```text
   /Users/carbon/Desktop/vibecon/src-tauri/target/debug/bundle/macos/VibeCon.app
   ```

4. 如果系统弹窗询问，请在 **隐私与安全性 → 自动化** 中允许 VibeCon 控制 **System Events**。
5. 重启 `pnpm tauri dev`，进入 **Mappings** 页面，再手动勾选开关。

映射现在会直接通过 macOS Quartz 发送快捷键，因此真正需要的是 **VibeCon 的辅助功能权限**；不再依赖额外的 `osascript` 自动化跳转。为了不干扰抓包，Debug 页面打开时映射会自动关闭。

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

手柄 report 与打标都只保留在本地，VibeCon 不发送遥测。现在唯一会自动执行的行为，是你明确开启的实验性 macOS 窗口切换；它不会执行 shell 命令，也不会自动批准 AI Agent 操作。

## License

等第一个通用控制器 profile 得到验证后，再确定开源许可证。
