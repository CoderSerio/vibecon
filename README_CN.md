# VibeCon

**把手柄变成可观察、可配置的 AI 编程控制台。**

VibeCon 的第一步很小，但刻意保持克制：先做一个原生桌面调试器，观察已经配对的 Nintendo Joy-Con 实际传来了什么输入，再决定怎样把它映射为窗口切换、Agent 中断或代码审阅操作。

项目目前在 macOS 上开发和验证；Tauri + Rust 的边界让未来的 Windows 原生版本成为可能，而不是把核心能力锁死在 Swift/macOS 中。

> 当前状态：早期原型，**只读**。它不会移动鼠标、注入按键、切换窗口、执行命令，或自动批准任何 Agent 操作。

[English README](./README.md)

## 当前已经实现的能力

在 Tauri 原生桌面窗口中可以：

- 发现已连接的 Nintendo HID 设备，包括 `Joy-Con (L)` 与 `Joy-Con (R)`；
- 选择一个控制器，持续读取最新的原始 HID 报文；
- 对 Joy-Con 原生 `0x30` 报文，以及 macOS 通用 HID 暴露的 `0x3f` 报文，解码摇杆数据和按钮字节；
- 用原创 CSS Joy-Con 可视化展示主摇杆实时移动，并高亮已经确认的方向键 HAT 状态；
- 保留最近 80 条报文，便于复制和排查。

即使设备使用了意料之外的报文模式，界面也会保留原始字节。这是有意为之：控制器映射必须基于我们真实观察到的输入，而不是先猜一套按键布局。

## macOS 快速开始

### 1. 配对 Joy-Con

1. 将 Joy-Con 从 Switch 拆下并确保有电；
2. 长按滑轨上的小圆形 **Sync** 键，直到玩家指示灯依次闪动；
3. 打开 **系统设置 → 蓝牙**，连接 `Joy-Con (L)` 或 `Joy-Con (R)`；
4. 需要调试两个手柄时，左右两侧分别配对。

如果蓝牙列表里找不到它：重新执行长按 Sync，并暂时让 Switch 远离或关机，避免 Joy-Con 自动回连 Switch。

### 2. 启动原生桌面壳

前置条件：已安装 Node.js、pnpm 与 Rust toolchain。

```sh
cd /Users/carbon/Desktop/vibecon
pnpm install
pnpm tauri dev
```

在打开的 VibeCon 窗口中点击 **Refresh controllers**，选择 Joy-Con，然后摇动摇杆或按下按键。下方的 **Raw input reports** 应开始刷新。

### 重要：不要用 `pnpm dev` 测硬件

`pnpm dev` 只会运行 Vite 浏览器预览，它适合看样式，但没有 Tauri 的 Rust 后端；因此没有 `invoke()`，也无法读取本地 HID 设备。

要调试 Joy-Con，请使用：

```sh
pnpm tauri dev
```

## 开发命令

```sh
pnpm build                    # 验证 TypeScript 与 Vite 构建
cd src-tauri && cargo check   # 验证 Rust/Tauri/HID 后端
```

目录结构：

```text
src/                    TypeScript 调试面板
src-tauri/src/lib.rs    Rust 命令与 Joy-Con HID 解码
src-tauri/              Tauri 配置、能力声明与桌面资源
README_CN.md            中文文档
```

## 为什么是 Tauri + Rust

- **优先服务 macOS 使用者：** 先在真实 Joy-Con 蓝牙/HID 行为上开发和验证；
- **不放弃 Windows 朋友：** 桌面界面和控制器核心不依赖 Swift/macOS API，但 Windows 仍要在 Windows 机器或 CI 上完成构建、测试；
- **先看见，再自动化：** 先把物理输入透明展示出来；之后才加入可审阅的明确映射，例如「摇杆左/右 → 前一个/后一个窗口」。

## 已规划，尚未实现

1. Joy-Con、DualSense、Xbox、8BitDo 的 profile 发现；
2. 控制器校准和可视化按键/轴映射；
3. 可选的平台动作适配层，例如前一个/后一个窗口；
4. 未来任何 Agent 批准动作，都必须有明确的手柄物理确认。

## 隐私与安全

当前 VibeCon 只读取本地手柄输入：不发送遥测，不模拟鼠标或键盘，不执行 shell 命令，也不自动批准 Agent 操作。

## License

等第一个真正可用的控制器 profile 验证完成后，再选择开源许可证。
