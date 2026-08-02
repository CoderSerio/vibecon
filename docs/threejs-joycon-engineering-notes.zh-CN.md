# 把 Joy-Con 放进 Three.js：一次边做边修的工程记录

这不是一篇“十分钟精通 Three.js”的教程。

它记录的是 VibeCon 的 3D Joy-Con 从一个能显示的模型，慢慢变成一个可以响应真实手柄输入的组件时，我们遇到了什么问题，又为什么选择现在这套做法。

如果几个月后我们忘了当时为什么这样写，希望只看这篇文档就能重新接上思路。

## 我们到底想做什么

目标听起来很简单：

> 现实里的 Joy-Con 摇杆动一下，画面里的 3D 摇杆也跟着动；按下按钮，模型上的那个按钮就亮起来或向下移动。

但这件事横跨了三个完全不同的世界：

1. **Joy-Con 输入协议**告诉我们“用户刚才做了什么”。
2. **Blender 模型**决定了“哪些东西可以单独移动”。
3. **Three.js 场景**负责把输入变成屏幕上的变化。

其中任何一层含糊，最后都会出现一种很熟悉的现象：看起来差不多能用，但按钮亮错位置、摇杆像贴纸一样移动，或者操作一会儿整个应用越来越卡。

## 先建立一个不容易混乱的脑内模型

当前的数据路径可以简化成这样：

```text
Joy-Con HID 报告
      ↓
Rust 解码按钮、摇杆和 IMU
      ↓
Vue 更新 stick / activeControls / imu
      ↓
ThreeJoyCon 根据变化请求绘制一帧
      ↓
修改 3D 节点的旋转、位置或材质
      ↓
WebGL 把场景画到 canvas
```

这里最重要的一点是：**输入数据和 3D 表现应该分开。**

Rust 和 Vue 只负责说“左摇杆现在向右 70%”或者“A 被按下”。至于模型该旋转多少度、发什么颜色的光，应该由 3D 组件决定。

这样以后即使换成别的模型、Babylon.js，甚至原生 Metal，手柄解码层也不用跟着推倒重来。

## 第一个错误：给模型前面套一个白圈

最早的 GLB 只有一个巨大的 mesh。外壳、方向键、摇杆和肩键全都在里面。

当时为了尽快验证输入位置，我们在按钮前方放了一个额外的 `RingGeometry`。按键发生时显示圆环，松开时隐藏。

它确实能快速回答一个问题：

> 我们猜的按键坐标大致对不对？

但它不是最终方案，因为它有几个明显问题：

- 圆环不是模型的一部分，手柄转动后很容易露馅。
- 圆环只是“盖在按钮前面”，并没有改变按钮本身。
- 摇杆移动看起来像光标移动，而不是一个实体在倾斜。
- 每个按钮都需要人工维护一个坐标和半径。

这就是典型的原型代码：它帮助我们确认了方向，但确认完以后应该及时退休。

## 为什么不能直接修改按钮材质

Three.js 当然支持修改材质。常见的词就是：

- `color`：材质的基础颜色。
- `emissive`：物体自己“发出来”的颜色，不完全依赖灯光。
- `emissiveIntensity`：发光强度。
- `MeshStandardMaterial`：Three.js 中常用的 PBR 材质。

理想代码大概是这样：

```ts
const button = scene.getObjectByName("Button_A");
button.material.emissive.set("#7de6c4");
```

问题是我们的模型里根本没有 `Button_A`。

Blender 检查显示，每只 Joy-Con 都只有一个连通网格和一个材质。更糟糕的是，它并不是“许多独立零件碰巧合并成一个对象”，而是所有顶点真的被焊接到了一起。按照 loose parts 分离，只能得到整只手柄，得不到单独按钮。

如果这时修改材质，整只 Joy-Con 会一起亮。

所以这不是 Three.js API 不够强，而是模型没有给运行时代码留下可以控制的边界。

## 临时过渡：在真实表面上做高亮

在 Blender 拆件完成前，我们使用了一个比白圈更合理的过渡方案：给 `MeshStandardMaterial` 注入一个表面高亮 mask。

它的大意是：

1. 告诉 shader 按钮中心在模型表面的哪个位置。
2. 计算当前像素离按钮中心有多远。
3. 距离足够近，就向 `emissive` 中加入一点颜色。

这个方案不会增加一个盖在模型前面的圆圈，亮起来的是模型自己的表面。

但它仍然只是过渡方案：它可以让某块表面发光，却不能让焊死的按钮真正向下移动。

当前实现位于 [`ThreeJoyCon.vue`](../src/components/ThreeJoyCon.vue)。已经从 Blender 拆出来的部件会跳过表面 mask，直接控制真实 mesh；还没拆的按钮暂时继续使用 mask。

## 真正的解决办法：回到 Blender 拆件

我们没有直接在唯一的模型上开刀，而是先建立了两层资产：

```text
work/baseline/
  VibeCon_FullJoyCons_clean_uv_fixed_packed.blend

work/development/
  VibeCon_FullJoyCons_interactive_dev.blend
```

`baseline` 是目前确认可用的基准：

- 左右 UV 正确。
- Clean 材质正确。
- 八张实际使用的贴图已经 Pack 进 `.blend`。
- 移动文件后不会因为相对路径变化而丢贴图。

`development` 每次都可以从 baseline 重新复制。实验切坏了，不修补残局，直接重新复制一份再来。

这是 3D 资产开发中很朴素但很有用的习惯：**可恢复性比一次切对更重要。**

### 焊死了以后怎么切

虽然网格连在一起，按钮仍然比外壳更凸。

我们分析了每个区域的面中心，发现：

- 外壳正面大致处在 `X ≈ 0.086–0.098`。
- 摇杆顶部可以达到 `X ≈ 0.160`。
- 方向键和 ABXY 的凸面也比外壳略高。

因此可以结合两个条件选面：

1. 这个面是否落在摇杆或按钮的平面半径内。
2. 这个面是否比外壳正面更凸。

第一个 MVP 只拆左摇杆。它最终分离出 216 个面，并设置自己的几何中心作为 pivot。Workbench 检查图位于本地模型工作目录：

```text
/Users/carbon/Desktop/nintendo-switch-with-detachable-joycons/work/development/
  VibeCon_interactive_left_stick_mvp_workbench.png
```

图中绿色部分就是独立摇杆。它不会随 Git 仓库发布；模型许可证确认前，GLB 和相关截图都不应当进入 Release。

负责这套流程的脚本都在 [`tools/blender`](../tools/blender)：

- `prepare_full_switch_joycons.py`：从 Switch 模型中提取左右 Joy-Con。
- `clean_joycon_color_textures.py`：生成更接近新品的颜色贴图。
- `pack_joycon_baseline.py`：制作自包含 baseline。
- `split_interactive_parts_mvp.py`：目前只拆左摇杆。
- `export_interactive_joycons.py`：导出包含独立节点的开发 GLB。

## 为什么左手柄曾经像丢了材质

源 FBX 在每个 Joy-Con 上都保留了三套 UV：

- `LJUV`
- `RJUV`
- `SUV`

分离网格以后，Blender 默认让左右两边都使用 `RJUV`。因此左手柄虽然加载了 `L_Colour.png`，却用右手柄的坐标去读取它，看起来就像材质丢失或严重发灰。

修复规则很简单：

```text
Joy-Con (L) → LJUV
Joy-Con (R) → RJUV
```

这次问题提醒我们：**贴图文件存在，不等于模型正在正确读取贴图。** 检查材质时要同时确认 image、材质节点和 active UV。

## “越操作越卡”不一定是模型面数

每只 Joy-Con 只有约 3400 个顶点，左右加起来不到 7000。对桌面 GPU 来说，这个数量很小。

真正昂贵的东西主要有这些：

### 1. 两套 2048 贴图

Base Color、Normal、Roughness、Metallic 在 GLB 中经过压缩，所以文件只有十几 MB；进入 GPU 后会展开。左右手柄加上 mipmap，实际显存占用会比文件大小大很多。

后续更值得做的是把调试预览贴图降到 1024，并把适合合并的通道打包，而不是急着把 3400 个顶点减成 2000 个。

### 2. 两个 WebGL renderer

当前左右手柄分别有自己的 canvas 和 WebGL context。这样组件简单，但灯光、shadow map、上下文和资源管理都会重复。

成熟版本应该考虑一个 renderer、一个 scene、一个 camera，同时放入左右手柄。

### 3. 输入不断制造无意义更新

Joy-Con 输入最高以约 60 Hz 进入前端。之前即使按键状态完全相同，我们仍会创建一个新的 `activeControls` 数组，让 Vue 认为状态发生了变化。

现在相同状态会直接返回，不再带着左右 3D 组件重复更新。

### 4. 日志越来越多

日志有 160 条上限，所以不是无限泄漏。但每次插入都需要复制数组、重新分组，并让 Vue 比较越来越多的 DOM 行。

这解释了为什么它在达到上限前会表现得“越操作越慢”。未来可以使用固定 ring buffer 和虚拟列表。

## 一帧结束时，Three.js 到底清理了什么

`renderer.render(scene, camera)` 会更新屏幕上的颜色和深度缓冲，但不会删除 scene 里的对象。

这通常是正确的：桌子、灯和手柄不会因为一帧画完就消失，下一帧还要继续用。

真正危险的情况是：

- 每一帧都 `scene.add(new Mesh(...))`，却从不 remove。
- 每次切换页面都重新加载 GLB，却不 dispose 老资源。
- 材质重新编译时，把旧 shader 引用一直保存在数组中。

我们没有在每帧增加 mesh，但之前在页面卸载时只调用了 `renderer.dispose()`。这不足以自动释放 GLTF 中的 geometry、material 和 texture。

现在卸载流程会显式完成：

```text
停止待绘制的 requestAnimationFrame
断开 ResizeObserver
遍历模型
释放 texture
释放 material
释放 geometry
清理 WebGL render list
释放 renderer 和 context
移除 canvas
清空 JS 引用
```

同时，每帧高亮计算会复用预先创建的 `Vector3` 和 `Vector4`，不再反复 `clone()` 并等待垃圾回收。

## 为什么改成事件驱动绘制

最早的两个 canvas 无论画面是否变化，都会一直以 60 FPS 重绘。

但调试页的大部分时间里，手柄可能静止。此时重复绘制同一个画面没有意义。

现在的规则是：

- 摇杆值变化，申请一帧。
- 按键状态变化，申请一帧。
- 开启姿态追随且 IMU 变化，申请一帧。
- 窗口尺寸变化，申请一帧。
- 用户点击复位，申请一帧。

如果同一轮事件中有多处状态一起变化，`requestAnimationFrame` 会把它们合并成一次绘制。

## 当前完成到哪里

已经实现：

- Clean 蓝红材质。
- 左右 Joy-Con 正确 UV。
- 基准与开发 Blender 文件分离。
- 左摇杆成为真实独立 mesh。
- 左摇杆围绕自身 pivot 跟随输入倾斜。
- 摇杆按下时修改自身 emissive。
- 其他按钮使用表面 shader 过渡。
- 事件驱动绘制。
- Three.js 资源显式释放。

还没有完成：

- 右摇杆拆件。
- 方向键和 ABXY 拆件。
- L、ZL、R、ZR、SL、SR 拆件。
- 一个 canvas 同时渲染左右 Joy-Con。
- 1024 调试贴图与通道打包。
- 日志虚拟列表。
- 完整的模型许可证确认。

## 下一次继续时，建议怎么走

不要一次拆完所有按钮。继续沿用已经验证过的节奏：

1. 拆右摇杆，复用左摇杆规则。
2. 在 Blender Workbench 中检查选面有没有吃到外壳。
3. 导出 interactive GLB，在 Three.js 中验证 pivot 和方向。
4. 再选择一个最简单的正面按钮，例如 A。
5. 验证真实按压位移和 emissive。
6. 确认规则稳定后，才批量处理 ABXY 和方向键。

一句话总结这次经验：

> Three.js 能控制什么，首先取决于 Blender 留给它什么；性能问题也不要凭感觉先减面，要先分清是输入、DOM、JavaScript、贴图、WebGL，还是生命周期出了问题。
