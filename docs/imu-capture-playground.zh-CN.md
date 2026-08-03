# VibeCon IMU 动作采样 Playground 设计

这份文档先设计采样体验，不实现采样功能。

目标不是让用户“录一堆传感器数字”，而是让用户在明确的动作指导下，稳定地产生一组可以复查、可以删除、可以重新采集的动作片段。

## 先确定一个原则

3D 模型的动画是**动作提示**，不是传感器的真值。

模型可以告诉用户“请向前倾斜约 30°”，但用户真正做出来的角度、速度和路径必须以 Joy-Con 的原始 IMU 序列为准。不能因为用户没有完全模仿动画，就把这次数据判定为错误；采样器应该保存原始数据和质量指标，让后续分析决定是否采用。

## Playground 的页面结构

建议从 Debug 页右上角打开“动作采样”，使用一个较宽的 modal，而不是再开一个全新的页面。

```text
┌──────────────────────────────────────────────────────────────┐
│ 动作采样                                      第 2 / 5 次     │
├──────────────────────┬───────────────────────────────────────┤
│ 动作示范              │ 采样状态                              │
│                      │                                      │
│   3D Joy-Con         │ ① 选择手柄：L / R / 双手柄             │
│   目标姿态与箭头       │ ② 选择动作：向前倾 / 左转 / ...        │
│                      │ ③ 保持静止                            │
│   [播放示范]           │ ④ [开始采样]                          │
│                      │ ⑤ 执行动作                            │
│                      │ ⑥ [结束采样]                          │
├──────────────────────┴───────────────────────────────────────┤
│ 最近一次质量摘要：时长、样本数、陀螺 RMS、加速度范围、警告      │
│ [重做本次] [保存并继续] [结束采样]                              │
└──────────────────────────────────────────────────────────────┘
```

默认不要同时显示两只手柄的复杂示范。先选择“左手柄 / 右手柄 / 双手柄”，双手柄动作再分别指定左右动作，避免用户不知道应该模仿哪一边。

## 状态机

采样器应该是一个显式状态机，避免“按钮看起来能点，但数据其实没有录到”的情况。

```text
idle
  → configuring
  → ready
  → countdown (3, 2, 1)
  → recording
  → settling (松手后等待 500ms)
  → review
       ├─ redo → ready
       ├─ save → next repetition / complete
       └─ cancel → idle
```

### 每个状态的行为

- `idle`：没有打开采样任务，不增加日志、不写文件。
- `configuring`：选择手柄和动作，预览 3D 示范。
- `ready`：要求手柄保持竖直并静止；显示静止噪声是否低于阈值。
- `countdown`：锁定配置，开始收集原始样本；倒计时结束才把片段标记为正式动作。
- `recording`：实时显示时长、样本数和三个轴的小型曲线；不显示复杂的十六进制日志。
- `settling`：结束后再收集一小段静止数据，用来估计动作是否真正结束。
- `review`：显示质量摘要，允许重做，不自动把坏片段混入数据集。
- `complete`：5 次都通过后，保存 manifest，并提供“继续采 5 次”而不是强迫用户离开。

“开始采样”和“结束采样”仍然由用户点击，但倒计时、最短/最长时长和 settling 阶段由程序控制。这样不同重复之间才有可比较的边界。

## 第一批预设动作

先做能用六轴 IMU 解释清楚的动作，不要一开始把“画圈”当成一个单独类别：

1. `neutral`：保持竖直静止（用于噪声和 bias）
2. `pitch_forward` / `pitch_backward`：绕本体 X 轴前后倾
3. `roll_left` / `roll_right`：绕本体 Y 轴左右侧倾
4. `yaw_left` / `yaw_right`：绕本体 Z 轴左右转向
5. `swing_forward_backward`：前后挥动，保留线性加速度特征
6. `swing_left_right`：左右挥动，保留线性加速度特征

前六类用于验证采样链路。真正用于快捷映射的动作，应该等采到一批数据后再决定，不能先凭直觉固定阈值。

## 3D 示范怎么做

第一版不需要在 Blender 里制作动画。Three.js 可以直接对现有旋转 pivot 做程序化动画：

```text
静止姿态
  → 目标姿态（例如 rotateX +30°）
  → 保持 600ms
  → 回到静止姿态
```

示范组件需要显示：

- 当前动作名称和目标轴
- 一个半透明箭头或轨迹弧线
- 目标角度只是“参考”，不作为通过条件
- [播放示范]、[循环播放]、[隐藏示范]

用户采样时，真实模型继续显示真实姿态；示范动画最好使用另一个轻量 ghost 模型或只显示箭头，不能把真实输入模型强行拉到目标姿态，避免混淆。

## 数据协议（先定下来再写代码）

UI 日志和动作采样必须分开。采样模式不受 16ms UI 节流影响，并且 Rust 端应保存一个原生 `0x30` report 中的全部 IMU 子样本，而不是只发第一组给 Vue。

建议每次任务保存一个 `manifest.json`，每次重复保存一个 JSONL：

```json
{
  "schema_version": 1,
  "session_id": "2026-08-03T...-pitch-forward-left",
  "device": {
    "vendor_id": 1406,
    "product_id": 8198,
    "side": "left",
    "transport": "Bluetooth / HID"
  },
  "action": "pitch_forward",
  "repetitions_expected": 5,
  "source": "user_guided_playground",
  "samples": "repetition-02.jsonl"
}
```

JSONL 每行保留：

```json
{
  "timestamp_us": 123456789,
  "report_id": 48,
  "subsample": 0,
  "accel_raw": [3434, -1469, 1700],
  "gyro_raw": [-197, -98, -88],
  "buttons": [0, 0, 8],
  "raw_report_hex": "..."
}
```

保存 raw 值很重要：后面发现轴符号、工厂校准或单位换算有问题时，可以重新计算，不必让用户重新做动作。

## 质量门禁

每次重复结束后先计算摘要：

- 有效时长与样本数量
- 报告间隔的中位数和最大间隔
- 静止段 gyro RMS
- 加速度模长偏离 1g 的比例
- 是否发生传感器饱和或断流
- 动作段的最大角速度、角度范围和峰值数量

第一版只做提醒，不自动淘汰：

- 红色：没有有效样本、严重断流、时长不足
- 黄色：静止噪声偏大、动作幅度太小、加速度明显受挥动影响
- 绿色：可以保存

5 次是一个适合 MVP 的默认值，不是训练数据集的规模。后续若要训练动作分类器，应按用户/日期划分训练和验证集，不能把同一个人的 5 次重复随机打散到两边。

## 存储与撤销

默认写到：

```text
~/.vibecon/captures/<session-id>/
  manifest.json
  repetition-01.jsonl
  ...
  repetition-05.jsonl
```

界面必须提供“删除本次任务”而不是只清空列表。原始采样可能包含用户的动作习惯，默认只保存在本机，不上传。

## 调研结果：我们可以站在哪些肩膀上

### 1. Joy-Con 协议与传感器事实

[dekuNukem/Nintendo_Switch_Reverse_Engineering](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering) 记录了 Joy-Con 使用 LSM6DS3 六轴 MEMS，报告的硬件配置包括：加速度计 ±8g、陀螺仪 ±2000 dps；传感器轮询频率远高于控制器状态报告频率，状态报告约每 15ms 一次。

对 VibeCon 的直接影响：采样器必须保留 report 内的完整 IMU 样本和时间戳，不能把面向 UI 的节流频率误当成传感器采样率。

### 2. Joy-Con 专用姿态估计

[QiuYuCode/joycon_ros2_driver](https://github.com/QiuYuCode/joycon_ros2_driver) 的 `orientation.py` 使用四元数积分陀螺仪，并只在加速度模长位于 `0.8g–1.2g` 时加入重力反馈；同时提供由加速度初始化 roll/pitch 和单独重置 yaw 的流程。

这与我们的 Playground 设计高度一致：开始动作前先静止、用静止段做 bias、动态挥动时不要无条件信任加速度计。该仓库没有声明可直接复用的项目许可证，暂时只借鉴思路，不复制代码。

[matiaspalmac/everything-imu](https://github.com/matiaspalmac/everything-imu) 是 MIT 项目，包含 Joy-Con 轴重映射、工厂校准系数、Madgwick/VQF 等融合实现。它明确对右 Joy-Con 做 Y/Z 符号修正，提醒我们左右手柄不能只靠镜像 UI 推断轴方向。可以在许可证允许的范围内进一步研究其实现。

### 3. Joy-Con 数据采集与可视化先例

[macitry/joycon_imu](https://github.com/macitry/joycon_imu) 展示了直接读取 Joy-Con IMU、保存/绘制原始轴、积分角度和 Kalman/互补滤波结果的实践。它没有声明许可证，因此只作为采样界面和曲线布局的参考，不直接复制代码。

### 4. 通用惯性手势识别研究

- [Gesture recognition with inertial sensors and optimized DTW prototypes](https://doi.org/10.1109/icsmc.2010.5641703)：说明在小规模、动作边界明确的场景里，DTW 是一个值得先于深度模型尝试的基线。
- [PCA & HMM Based Arm Gesture Recognition Using IMU](https://doi.org/10.4108/icst.bodynets.2013.253667)：把连续 IMU 序列切成阶段并用 HMM 建模，适合参考我们的“倒计时—动作—settling”边界设计。
- [Echo State Networks and Long Short-Term Memory for Continuous Gesture Recognition](https://doi.org/10.1007/s12559-020-09754-0) 及其[复现实验仓库](https://github.com/swtietz/UHH-IMU-gestures-comparison)：数据由 5 位参与者完成，比较 ESN 与 LSTM。它提醒我们必须按参与者划分验证集，不能把同一人的重复样本随机泄漏到训练和验证两边。

这些研究不是 Joy-Con 专属数据集，不能直接拿来训练 VibeCon 的动作分类器；它们能直接借鉴的是采样边界、重复设计、归一化、DTW 基线和跨用户验证方法。

## 推荐实现顺序

1. 先实现本 Playground 的状态机和假数据回放，不连接真实设备。
2. 再让 Rust 采样器写入完整原始序列，Vue 只订阅摘要。
3. 使用 `neutral`、`pitch_forward`、`yaw_left` 三类动作各采 5 次，检查数据是否可解释。
4. 用曲线和 DTW 做离线比较，确认动作间确实存在稳定差异。
5. 最后才考虑动作分类、快捷键触发或导出训练数据。

一句话总结：**先把“怎样采到可信数据”做成一个可复查的实验流程，再讨论模型能不能识别动作。**

## 库的选择：采用、参考，还是暂时不引入

调研后不建议把一个“大而全”的追踪器项目直接嵌进 VibeCon。我们现在已经有 Tauri、`hidapi`、按钮映射和震动输出；直接替换 HID 生命周期会让调试范围突然扩大。

### 最值得参考：`everything-imu`

[matiaspalmac/everything-imu](https://github.com/matiaspalmac/everything-imu) 是 MIT 项目，最有价值的不是 UI，而是它拆开的几个工程边界：

- Joy-Con 右手柄轴符号修正
- 工厂校准与 raw → SI 单位转换
- Madgwick / VQF 等姿态融合
- 独立的设备层、融合层和追踪输出层

它的 `device-joycon` 依赖 `hidapi`、`btleplug`、异步运行时和一组内部 crate。对 VibeCon 来说，优先借鉴 `axis_remap`、`calibration`、融合测试夹具；暂时不要引入整个 crate graph。

### 协议事实来源：`dekuNukem`

[Nintendo Switch Reverse Engineering](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering) 不是可依赖库，但它是我们确认 Joy-Con 报告布局、LSM6DS3、传感器频率和校准存储位置的主要资料。协议事实应引用它，运行时代码仍由我们自己维护。

### 姿态算法参考：`joycon_ros2_driver`

[QiuYuCode/joycon_ros2_driver](https://github.com/QiuYuCode/joycon_ros2_driver) 的姿态模块很适合对照：四元数积分、加速度模长为 `0.8g–1.2g` 时才做重力修正、静止标定和 yaw 复位。这套思路与我们的采样 Playground 直接相容，但仓库没有明确声明可直接复用的许可证，所以只参考算法结构，不复制实现。

### 可以考虑直接依赖：`joycon-rs`

[KaiseiYokoyama/joycon-rs](https://github.com/KaiseiYokoyama/joycon-rs) 是 Apache-2.0，目标平台包含 macOS 和 Windows，提供 Joy-Con 管理、输入报告、灯光与震动等更完整的 Rust 封装。

它目前仍标注为开发中，且会接管设备发现和报告读取。短期继续使用现有 `hidapi` 更稳；如果之后要补 SPI 工厂校准、设备热插拔和更完整的输出协议，再做一个独立分支评估迁移，不要在 Playground 开发中途替换底层。

### 只作为采样界面参考：`macitry/joycon_imu`

[macitry/joycon_imu](https://github.com/macitry/joycon_imu) 展示了原始轴曲线、积分角度、Kalman 与互补滤波。但它没有声明许可证，不能直接复制代码；我们只借鉴“同时画 raw / angle / filtered 三组曲线”的调试体验。

### 数据集和识别论文不能直接替代我们的采样

[UHH IMU gestures](https://github.com/swtietz/UHH-IMU-gestures-comparison) 和其对应的 [ESN/LSTM 论文](https://doi.org/10.1007/s12559-020-09754-0) 很适合参考跨用户验证与连续动作切片，但它不是 Joy-Con 数据，也不能直接解决左右手柄轴向和握持姿态问题。

## 最终决策

```text
现在：hidapi + 自己的报告层 + 自己的 Playground
借鉴：everything-imu 的校准/轴修正，ROS driver 的四元数融合
事实：dekuNukem 的协议资料
以后评估：Apache-2.0 joycon-rs，前提是需要完整设备生命周期
不直接复制：无许可证的 Joy-Con 项目代码
```
