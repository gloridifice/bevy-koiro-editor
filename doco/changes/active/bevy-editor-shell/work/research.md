# 原型与 Bevy API 调研

调研日期：2026-09-30。视觉验收由用户负责；这里是实现依据与分析，不是验收结论。

## 1. Chrome DevTools MCP
当前 harness MCP 网关没有配置服务，因此通过 MCP SDK 的 stdio transport 启动 **chrome-devtools-mcp 1.10.1**，并非用源码推测截图。Chrome 独立 headless profile、初始 viewport 1440×900、WebGL SwiftShader；关闭遥测与 CrUX；workspace 限制为本项目。没有使用用户的 Chrome profile，也没有修改 HTML 文件。

调用路径：new_page(file URL) → evaluate_script 重置独立 profile 中原型状态 → take_screenshot → evaluate_script 读取 CSS tokens/DOMRect → take_snapshot 获取 UID → take_screenshot(uid) 截取各 Pane。为了取得各 Pane UID，只在独立页面 DOM 上临时添加 region/aria-label，不修改布局与样式。随后依次切换 Assets Tree、Pane type/File 菜单、Scene/Assets 预设；resize_page 到 1000×700。

截图在 [evidence](evidence/)：
- [默认全图](evidence/prototype-default.png)
- [Header](evidence/header.png)、[World](evidence/world.png)、[Viewport](evidence/viewport.png)、[Inspector](evidence/inspector.png)、[Gallery](evidence/assets-gallery.png)、[Camera](evidence/camera.png)
- [Assets Tree](evidence/prototype-assets-tree.png)
- [Pane 类型菜单](evidence/prototype-pane-menu.png)、[File 菜单](evidence/prototype-file-menu.png)
- [Scene](evidence/prototype-layout-scene.png)、[Assets](evidence/prototype-layout-assets.png)、[1000×700](evidence/prototype-1000x700.png)

## 2. Chrome 实测 DOMRect（1440×900）
| 区域 | x | y | width | height |
| --- | ---: | ---: | ---: | ---: |
| Header | 0 | 0 | 1440 | 40 |
| World | 5 | 45 | 263.625 | 824 |
| Viewport | 273.625 | 45 | 832.578125 | 561 |
| Gallery/Tree | 273.625 | 611 | 832.578125 | 257.984375 |
| Inspector | 1111.203125 | 45 | 323.78125 | 536.4375 |
| Camera | 1111.203125 | 586.4375 | 323.78125 | 282.546875 |
| Status | 0 | 874 | 1440 | 26 |

相邻 Pane 间隙是 **5px**，不是 6px。默认树比例：根 X .185；剩余 X .72；中心 Y .685；右侧 Y .655。需要在每个 Split 的长度扣除 5px gap 后乘 ratio，不能以全窗口百分比代替局部分割。

## 3. 样式与内容分析
- 石墨灰主题，不使用大面积彩色强调。bg #141414，panel #202020，field #181818，hover #303030，line #363636，ink #e5e5e5，muted #9c9c9c，accent #d6d6d6。
- Header 40、Status 26、Tab strip 34、普通 toolbar 36、Viewport 37、Camera 31、Pane footer 26。Pane 1px border/5px radius，active tab 顶部 2px accent，selected tree 左侧 2px accent。
- 树行 24px；实体标题 13px；组件标题 35px；属性标签约 58px；vec3 字段高度 25、间距 4、axis 标签 17px。XYZ 保留低饱和红绿蓝。正文 11、次级 9–10；HTML 字体列表首项 Inter，Windows 回退 Segoe UI。
- Gallery 是自适应 grid，最小列 103、gap 11、padding 13，缩略图比例 1.38。原型还有 102px 分类导航；首版先交付资产索引/图片与类型回退，不复制此分类侧栏。
- 类型菜单宽 430、行高 30，图标/名称/右对齐说明/选中符号同一行。选择类型改变 Pane 内容，不改变世界。
- HTML 的 World 使用虚拟对象数据，Assets Tree 使用虚拟文件；原生版必须对应真实 ECS 与本地只读索引，不搬运假数据。HTML 的播放、撤销、材质编辑和导出项目快照不是首版已实现功能。
- Camera 是独立视图，Viewport 有编辑 gizmo；不能以 Viewport 截图充当实时游戏相机预览。

## 4. 版本事实
[crates.io API](https://crates.io/api/v1/crates/bevy) 实际返回：最新预发布 **0.20.0-rc.2**（2026-09-28 发布），最新稳定 **0.19.1**（2026-08-13）。[Bevy 官方 news](https://bevy.org/news/) 最新稳定发布文章为 Bevy 0.19。项目原来已经使用 RC，本次保留并精确锁定，不把 RC 说成稳定版。RC 的 Cargo 元数据要求 Rust 1.96，使用本机 rustc 1.96.0 编译。

## 5. 已核对的现代 API
源码来自 cargo fetch 的 0.20.0-rc.2，不套用旧版本教程：
- [BSN example](https://docs.rs/crate/bevy/0.20.0-rc.2/source/examples/scene/bsn.rs)：bsn!/bsn_list!、Children 用 -- 分隔、@函数组合、on 观察器。Commands::spawn_scene 排队；World::spawn_scene 返回 Result<EntityWorldMut, SpawnSceneError>。
- [bevy_scene 文档](https://docs.rs/bevy_scene/0.20.0-rc.2/bevy_scene/) 明确 **.bsn 文件格式尚未发布**。使用内联宏，不设计不存在的 loader。
- Node 自身包含 border_radius；使用 Node/Text/TextFont/ImageNode 等 Component，不使用旧 NodeBundle/Style/TextBundle。
- TextFont.font_size 是 FontSize，字体可经系统发现解析 FontSource::Family；BSN 的字段使用 FontSourceTemplate::Family("Segoe UI")。直接将已构造的 TextFont 值插入 BSN 会触发模板限制，已通过编译原型确认。
- [原生 Button](https://docs.rs/bevy_ui_widgets/0.20.0-rc.2/bevy_ui_widgets/struct.Button.html) 是无样式行为控件，触发 Activate；Hovered/Pressed 负责状态。拖动 Tab 用普通 Node + PointerClick/Drag 处理，避免 Button 释放时的激活销毁捕获节点。
- [TextInput](https://docs.rs/bevy_ui_widgets/0.20.0-rc.2/bevy_ui_widgets/struct.TextInput.html) 配合 EditableText、TextCursorStyle、InputFocus；已用真实 ECS 数值提交路径测试 Enter/失焦与非法输入。
- [ViewportNode](https://docs.rs/bevy_ui/0.20.0-rc.2/bevy_ui/widget/struct.ViewportNode.html) 绑定 Camera entity，自动根据 ComputedNode 调整目标 Image 尺寸，并将指针映射到离屏视图。必须显式安装 MeshPickingPlugin。
- RenderTarget 是独立 Component，窗口 UI 使用 RenderTarget::Window + UiTargetCamera；离屏 Camera3d 使用 RenderTarget::Image。
- PointerClick/PointerDrag 等是直接事件类型，On<PointerDrag> 中位置来自 event.pointer.position，而不是旧 pointer_location 字段。
- gizmo 的盒子 API 为 cube，DirectionalLight 使用 shadow_maps_enabled；编辑 gizmo 单独 RenderLayers::layer(1)，Camera preview 只看世界层 0。

## 6. 工程原型与证据边界
[BSN UI 原型](../../../../../examples/bsn_ui.rs) 可用 cargo run --example bsn_ui 运行，验证内联 Children、可复用 Scene 函数、Button 和 pointer observer 的组合。

[原生默认布局截图](evidence/bevy-default.png) 与 [资产布局截图](evidence/bevy-assets.png) 来自 Windows 的 cutty，已实际读取检查，能够证明本机渲染出 UI、庭院、实时视图以及 checker.png 图片/notes.txt 类型回退。运行环境：NVIDIA RTX 5070 Ti Laptop GPU，Vulkan，Windows 200% DPI。

这些是开发过程截图，不是最终视觉验收；随后代码仍调整了菜单/Tab 事件处理和滚轮父链路由。2026-09-30 用户明确表示交互与视觉由其验收，此后停止桌面操作验证。数据层测试通过不代表标签拖动、焦点与 mesh 拾取已经得到 GUI 验收。具体用户待验收项见 tasks。
