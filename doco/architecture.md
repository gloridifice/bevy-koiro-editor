# 当前架构

## 状态
项目已从 Hello World 变为原生 Bevy UI 编辑器首版。Bevy 精确锁定 0.20.0-rc.2，Rust 最低 1.96。代码与静态/数据边界检查已交付；原生窗口截图与独立视觉审计覆盖内置预设、窄窗口和内容间距，完整交互验收仍由用户执行，变更保持 active。

## 模块与职责
| 模块 | 已实现边界 |
| --- | --- |
| [main](../src/main.rs) | DefaultPlugins、主窗口 1440×900、以当前工作目录的 assets 为 AssetPlugin 根目录、EditorPlugin |
| [layout](../src/layout.rs) | 与 ECS 无关的 Pane/Group/Split/Layout 数据、三套预设、停靠/关闭/增加/裁剪与 JSON 验证；仅依赖 serde |
| [editor](../src/editor.rs) | 插件编排、各窗口工作区、共享选择、菜单动作、停靠拖动、滚动、显式布局保存；EditorUiPlugin 管理快捷键、原生文本提交与保留焦点的 UI 生命周期 |
| [ui](../src/ui.rs) | BSN 基础控件、响应式 Header/工作区、递归布局几何、六种 Pane、锚定菜单、悬停提示、原生滚动条与停靠预览 |
| [icons](../src/icons.rs) | 编译时嵌入 src/lucide_icons 中使用的 Lucide SVG；resvg 按原生 UI 的物理尺寸生成白色 alpha 纹理，通过 ImageNode 着色和缓存复用 |
| [cursor](../src/cursor.rs) | 控件光标语义、物理鼠标及其 Viewport 派生指针路由、拖动优先的每窗口系统光标与取消/失焦清理 |
| [scene](../src/scene.rs) | 程序化庭院、可编辑根实体标记、内置库缩略图、离屏视图、游戏相机姿态同步、网格与选择标记 |
| [transform gizmo](../src/transform_gizmo.rs) | 原生 TransformGizmoPlugin 的设置、状态与手柄渲染；离屏、多窗口鼠标输入适配、父变换转换、拖动取消及 overlay 相机路由 |
| [assets](../src/assets.rs) | assets 的只读、目录优先索引及图片句柄；跳过符号链接、限制扫描规模 |

## 数据与渲染流
- 游戏数据存在同一个 Bevy ECS World 中；只有带 EditorObject 的根实体进入世界树。模型的子网格由 ChildOf 归属根实体，拾取沿父链寻找根实体。
- Selection 是共享 Resource；每个 Window 的 EditorWindow Component 持有独立 LayoutDocument、菜单、焦点组和最大化状态。新窗口复制布局文档，不复制世界。
- UI Action/Pointer → editor、scene 的选择 observer 或 transform_gizmo → 布局、选择或 ECS 属性更新。Viewport 不注册 mesh 拖动移动逻辑，空间操作只由 gizmo 手柄驱动。文本控件通过 TabIndex 获取点击焦点，UI root 声明 Bevy 焦点 TabGroup（不是停靠模型中的 Group）；原生 TextInput 接收键盘，TextCursorStyle 用浅色插入光标。
- EditorUiPlugin 在 Update 处理快捷键，在 PostUpdate 的 EditableTextSystems 之后提交 Enter/失焦字段，避免丢失同帧最后一个字符。布局、选择和已提交字段改变时标记 UiDirty，作为 Update 中的合并刷新通知而非全局重建命令。刷新比较每窗口的布局结构、Layout 名称/下标、可见组及响应式模式；只有这些外壳事实变化时重建对应窗口，不影响其他窗口。菜单单独创建/销毁弹层，状态、焦点组边框、树选择高亮、名称、属性值、可见性图标及 gizmo 工具栏原地同步。搜索/展开、树结构、资产索引或 Inspector 选择改变时只替换相关滚动区的内容子节点，保留其 ScrollPosition、轨道和原生滑块。未变化的动作不销毁控件。失焦提交未完成时延后重建；按窗口、Layout、Pane、实体、字段和轴恢复当前 EditableText 编辑状态及焦点，包含草稿、插入位置、选择和排队编辑。字段不再可见时清除旧焦点。非法提交重新聚焦字段，保留错误草稿及 FieldError，UI 恢复时一并恢复；Escape 还原 committed 值并清除焦点；原生 TextInput 在 PreUpdate 先清焦点时，由 LastFocus 识别被取消的字段。常规外壳刷新按窗口、Layout 和 PaneScroll 身份保留内容区 ScrollPosition。搜索提交写入来源 Layout，避免预设间复用 Pane ID 导致串状态。拖动分隔条只更新几何，避免销毁被捕获节点。
- 控件声明 CursorRole；PostUpdate 读取 Bevy PointerInteraction/PointerState，按最近控件与拖动优先级更新窗口 CursorIcon，早于 winit 的 Last 更新。派生 mesh pointer 只有在物理鼠标操作对应 Viewport 时影响该窗口；触摸/其他离屏指针不改变 OS 光标。Escape、失焦和失效源清理 picking 捕获与编辑会话，释放/PointerCancel 沿原 picking 生命周期恢复。标签在其他窗口释放时拒绝旧停靠目标。
- EditorUiPlugin 注册 LucideIconsPlugin。所有编辑器 UI 图标使用显式 Icon 类型，包括 Pane、树行、工具栏、状态和菜单；普通标签不会解析实体名、布局名或文件路径中的符号。SVG 编译时嵌入，不依赖运行时 src 目录，也不进入用户 assets 索引。PostUpdate 在 UiSystems::Layout 后读取 ComputedNode 的物理尺寸（含目标窗口 DPI 和 UiScale），按图标及尺寸共享 Image；颜色只修改 ImageNode::color，不触发重绘。解析树按图标缓存，未被当前节点使用的纹理尺寸释放缓存句柄；同帧 UI 外壳重建复用已有纹理。单个纹理边长上限 512px，SVG 保持宽高比、居中绘制；透明抗锯齿覆盖率转为白色 straight-alpha 像素，避免暗边。
- Header 和 Status 是 UI root 的固定兄弟节点，不属于 DockNode。宽度小于 640px 时 Header 改为 76px 的双行；小于 900px 时额外显示 32px 的 PANES 组切换栏，工作区只显示焦点组，不改写 DockNode 或保存比例。render、geometry、菜单与拖放通过同一组 workspace_top/workspace_size/visible_group 函数计算工作区坐标。窗口宽度在同一响应式模式内变化只更新几何和菜单尺寸；跨越 Header/工作区/状态等响应式模式时只重建该窗口并保留焦点。窗口高度和分隔条拖动由几何更新处理，分隔条释放不触发额外刷新。UI root 通过 UiTargetCamera 绑定该窗口的 Camera2d；Camera2d 通过 RenderTarget::Window 指定窗口。菜单是高层非模态节点，没有全窗口拾取遮罩；外部控件动作自行关闭或切换菜单，其余外部 PointerClick 冒泡到窗口 UI root 后关闭，避免 PointerPress 时重建销毁尚待释放的控件。弹层标题/留白阻止点击向 root 冒泡。PostUpdate 在原生 UI 布局后把菜单锚定到实际触发控件并限制在窗口内；窄 Pane 类型菜单从三列改为纵向分类。TooltipState 管理鼠标悬停约 500ms 的首显及短暂的同组即时提示，提示忽略拾取，优先放在目标上方，空间不足才翻到下方；菜单、鼠标操作及字段编辑期间抑制提示，仅允许当前错误字段的校验提示。原生 Scrollbar/ScrollbarThumb 管理内容区轨道、拖动与翻页，轨道仅在确有纵向溢出时显示。
- 每个可见 Viewport/Camera 有自己的 Camera3d、目标 Image 和 ViewportNode。Viewport 可见世界层 0 + 网格/选择标记层 1；Camera 只见层 0，姿态跟随 MainGameCamera 实体。原生 TransformGizmoPlugin 注册后禁用其只读取 PrimaryWindow 的默认输入，编辑器用物理鼠标的实际 Viewport 命中与原生投影/约束工具适配输入，不改写窗口光标坐标。共享选择同步 TransformGizmoFocus，最后使用的 Viewport（初始取一个可见 Viewport）独占 TransformGizmoCamera。原生 overlay 的 RenderTarget/Projection 跟随该视图，color 不清屏；手柄从原生层 15 移到编辑专用层 2，避免与库缩略图层 10–18 冲突。Camera 和缩略图不渲染手柄。
- 内置资产有九张 160×116 离屏缩略图，使用独立 RenderLayers；本地图片通过 AssetServer 读取。当前缩略图相机持续渲染，尚未做冻结/按需调度优化。
- 必要的结构性 UI 外壳刷新按窗口、Layout 下标、Pane ID 和视图类型复用可见 Viewport/Camera 的节点、派生 pointer、相机与目标 Image；重建前暂时解除节点父子关系，刷新后重新挂接，避免普通 UI 交互重置离屏渲染资源。仅未被新 UI 使用的视图（隐藏、关闭、切换 Layout 或改变类型）释放相机与纹理；关闭窗口清理其 UiOwned 实体和视图纹理。orbit/pan 在拖动期间同步布局文档；相机拖动结束不触发 UI 外壳重建，避免无关 Pane 的滚动条/滑块重新布局而闪烁。重置/加载布局可更新复用相机的姿态。模型实体不会随布局切换销毁。

## 持久化与限制
工作目录下 editor-layout.json 仅保存当前窗口的布局文档；临时文件写入完成后 rename 替换。多个窗口保存同一文件，最后一次显式保存生效。不保存场景、Entity ID、运行期相机/纹理或选择；没有隐式自动保存。启动时保存布局无效则使用默认布局并显示错误。

BSN 是编译时场景组合，不采用尚未发布的 .bsn 资产格式。不使用 egui、WebView 或第三方 docking 库。当前支持示例世界的单选、Name/Transform/Visibility 与移动/旋转/缩放手柄；真正双 World、反射 Inspector 与 assets-only 面板是用户追加目标，尚未实现，见 active 变更。播放、撤销、框选和多选 gizmo 仍未实现。详细当前契约见 [编辑器规格](specs/editor.md)。原生测量工具 [ui_audit](../examples/ui_audit.rs) 在临时目录输出 ComputedNode 快照，[ui_audit.py](../scripts/ui_audit.py) 对匹配的全尺寸 cutty 截图计算边界、颜色、容器间距与独立像素重心，并生成标记图；不使用浏览器 DOM 近似原生布局。
