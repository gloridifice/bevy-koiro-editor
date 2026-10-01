<!-- doco:lifecycle v=1 created-at=2026-09-30T07:03:49Z completed-at=- archived-at=- -->
# 原生 Bevy 编辑器首版

## Purpose
变更前项目只有 Hello World。将 [HTML 原型](../../../../prototype/index.html) 的石墨灰编辑器原型转为可运行的 Bevy UI 编辑器，建立可扩展的 Pane、停靠布局和世界编辑边界，而非嵌入浏览器或 egui。

## Scope and acceptance
- Chrome DevTools MCP 检查原型，保留默认布局、六种 Pane、菜单、布局预设及窄窗口的截图和尺寸分析。
- 保留项目采用的最新预发布版本 Bevy 0.20.0-rc.2，使用 `bsn!`、原生 Button/TextInput/ViewportNode 和事件观察器。明确 RC 依赖与 BSN 资产文件尚未发布的限制。
- PaneType：Entity Component Inspector、World Inspector、Assets Tree、Assets Gallery、Viewport、Camera。纠正原始规格中的 Word/Gallary/Componet 拼写，不改变含义。
- Tab 持有一个 Pane；TabGroup 包含一个或多个 Tab；Split 递归组合 TabGroup。支持切换/增加/关闭/修改 Pane 类型、左右/上下分割、拖动标签停靠到中心或四边、分隔条调节、最大化。
- Default/Scene/Assets 分段布局选择、复制当前布局、重置、JSON 本地保存/载入。Header 永远位于布局外，每个新建编辑器窗口都有 Header；窗口共享世界与选择，布局独立。
- 编辑器与游戏使用真正独立的 World；世界树展示游戏 World 的实体层级。Inspector 基于 bevy_reflect 发现组件与参数并编辑支持的字段；不可反射/不可编辑内容明确只读。示例世界使用程序化低多边形庭院，不要求美术逐三角形复制 HTML。双 World 与反射改造是用户追加目标，尚待交付。
- Assets Tree 和 Gallery 仅展示本地 [assets 目录](../../../../assets)，不混入 Built-in 模型库；Tree 无缩略图，Gallery 展示图片预览和非图片文件类型回退。缺失目录、读取失败和无选择必须有明确状态。assets-only 调整尚待交付。
- Tab 栏及标签悬停使用 Pointer；按钮/可点击项、文本输入、标签拖动、横纵分隔条、数值拖动和视口操作使用对应系统光标；拖动样式优先于悬停，释放/取消/失焦后恢复，窗口之间不串状态。
- Viewport 与 Camera 都是实时离屏 3D 渲染而非截图；Viewport 支持选择、对象拖动、轨道/平移/缩放和 XYZ gizmo；Camera 不显示编辑 gizmo。当前只支持单选和世界空间移动，不伪装完整 DCC gizmo。
- `cargo check --all-targets`、布局/持久化边界测试、格式与静态检查通过；本机运行并截图确认布局和实际渲染。探索性 BSN/API 原型保留在 [examples 目录](../../../../examples)。

非目标：任意组件增删、场景文件保存、撤销/重做、播放模拟、导入模型转换、资产文件写入/重命名/删除、复杂旋转/缩放 gizmo、跨 OS 窗口拖动、完整生产级编辑器。不可用操作不展示为已实现功能。原型是视觉及交互参考，不是所有演示功能都必须在首版实现。

## Result
Pending — 首版代码、状态光标及原生输入焦点/提交/重建修复已交付，十四项边界测试通过；双 World、反射 Inspector 与 assets-only 的追加改造尚待交付，交互/视觉待用户验收。本变更保持 active；不自动 complete 或 archive。
