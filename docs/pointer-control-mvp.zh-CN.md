# Joy-Con 鼠标控制 MVP：实现、权限与验证

这份文档记录 VibeCon 当前的鼠标控制链路，以及开发时最容易误判成“协议没生效”的 macOS 权限问题。

## 两种模式

| 操作           | 摇杆模式             | 体感模式                           |
| -------------- | -------------------- | ---------------------------------- |
| 移动光标       | 左 / 右摇杆          | 旋转 Joy-Con                       |
| 左键与拖拽     | L / R                | L / R                              |
| 右键           | ZL / ZR              | ZL + L / ZR + R                    |
| 抬起并重新对准 | 不需要               | 按住 ZL / ZR；松开后以当前姿态继续 |
| 切换模式       | 长按 − / + 约 600 ms | 长按 − / + 约 600 ms               |

模式切换只改变 VibeCon 内部状态，因此不需要系统权限。移动、点击与拖拽通过 macOS Core Graphics `CGEvent` 输出，需要辅助功能权限。这解释了一个很迷惑但合理的现象：模式提示已经变化，光标却完全不动。

## 输入链路

```text
Joy-Con HID report
├─ 按键与摇杆解码
├─ Fusion 姿态估计
├─ 3D 实时预览
└─ Pointer Runtime
   ├─ 长按 − / + 切换模式
   ├─ 摇杆速度曲线，或相邻姿态角度差
   ├─ ZL / ZR clutch 与无跳变复位
   ├─ 亚像素累计与噪声阈值
   └─ macOS CGEvent 移动 / 点击 / 拖拽
```

调试页和映射页共享同一个手柄与 3D 预览。切到映射时只替换最下方的日志面板；原始日志停止增长，系统鼠标输出才会启用。

## 为什么 `tauri dev` 的权限会反复失效

普通 `cargo run` 生成的是 linker 临时签名二进制。它的 designated requirement 只有本次编译的 `cdhash`，Rust 热更新后 hash 会变化，macOS TCC 会把新二进制视作另一个程序。

项目的 macOS 开发配置使用 `scripts/tauri-dev-runner.sh`：

1. 先执行 `cargo build`；
2. 把二进制包装为 `target/debug/VibeCon Dev.app`；
3. 自动寻找钥匙串里的 `Apple Development` identity；
4. 用固定 identifier `io.coderserio.vibecon.dev` 签名整个 app bundle；
5. 通过 macOS Launch Services 启动 bundle，而不是直接执行 `Contents/MacOS/vibecon`。

因此系统设置里会显示明确的 `VibeCon Dev` 应用，而不是一个裸的 Unix 可执行文件。第一次仍需授权，但后续 Rust 重编译不应再因为 `cdhash` 改变而丢失权限。没有开发证书的贡献者仍可启动应用，只是 runner 会明确警告 ad-hoc 签名可能在重编译后重置权限。

## 如何验证

1. 运行 `pnpm tauri dev`。
2. 打开映射面板；权限状态会显示当前后端与需要授权的精确路径。
3. 点击“授予权限”，在系统设置中启用该 VibeCon 开发身份，然后完全退出并重新运行 `pnpm tauri dev`。
4. 点击“测试鼠标移动 +80 px”。VibeCon 会读取发送前后的系统光标坐标，只有观察到实际位移才会报告成功。`CGEvent.post()` 被调用但事件被系统丢弃时会明确报错，而不是产生“已经发送”的假阳性。
5. 启用“鼠标控制”，先测试摇杆模式，再长按 − 或 + 约 600 ms 测试体感模式。

Pointer Runtime 的移动、点击和拖拽也会传播 Core Graphics 后端错误。权限就绪但事件源创建或发送失败时，映射面板会显示具体错误；不会继续静默累计位移并在稍后突然跳动。

开发版还包含一个 debug-only 指针自检，但必须通过 Launch Services 运行，例如 `open -n "src-tauri/target/debug/VibeCon Dev.app" --args --pointer-self-test`。直接执行 bundle 内的 Mach-O 会让 TCC 将它视为命令行进程，无法代表应用实际获得的权限。自检不依赖 WebView、Joy-Con 或映射状态机。

若开发应用没有出现在辅助功能列表，可通过 `open -n "src-tauri/target/debug/VibeCon Dev.app" --args --request-accessibility` 发起请求。请求由 `VibeCon Dev.app` 自身发起，只负责向 TCC 注册正确身份并打开系统设置；权限开关仍必须由用户手动确认。

## 下一轮真机需要观察的内容

- 左右 Joy-Con 的摇杆上下方向是否都符合屏幕方向；
- 体感水平轴是否需要反向；
- 缓慢旋转时是否连续，静止时是否抖动；
- L / R 拖拽与 ZL / ZR 右键是否会卡住；
- 体感模式按住 ZL / ZR 后光标是否冻结，松开时是否无跳跃；
- 两只手柄同时连接时，任意一侧切换全局模式是否只触发一次。

当前没有实现滚动。先把移动、点击、拖拽、切换与复位验证稳定，再决定滚动手势，避免挤占单手握持时最容易按到的肩键。
