# 原生编辑器目标规格

## Scope and current contracts
变更前项目没有编辑器契约。本规格描述首版目标，视觉参考 [HTML 原型](../../../../../../prototype/index.html)。六种 Pane 的内容状态独立于世界实体数据，Header 不参与 Dock 布局。详细交付范围与排除项见 proposal。

## Requirements
### 概念与布局
PaneType 决定内容；Pane 是有稳定 ID 的实例。Tab = Pane 的标签入口。TabGroup 包含至少一个 Tab，只有 active Tab 内容显示。Layout.root 是 Group/Split 二叉树；X 是左右分割、Y 是上下分割。嵌套 Group 是递归布局而非在标签里面嵌套标签。最后一个标签不得让工作区消失。Default：左 World 18.5%，剩余区域中心 72%；中心上 Viewport 68.5%、下 Gallery/Tree；右上 Inspector 65.5%、右下 Camera。Scene：左 17.5%，剩余中 77%，Viewport/Camera 合组；Assets：左 18.5%，剩余中 72%，右 Inspector/Viewport 上下 55%。比例按各 Split 本地可用长度计算。5px 外边距与 5px 分隔条（Chrome 实测间隔为 5px）。
Header 40px、Status 26px、Tab strip 34px、常规 toolbar 36px、Viewport toolbar 37px、Camera toolbar 31px、Pane footer 26px，均为逻辑像素，适配系统 DPI。Pane border 1px/radius 5px；字段 25px/radius 3px；搜索 26px/radius 4px；树行 24px。正文 11px、次级 9–10px、实体名 13px；灰阶主题参见 evidence 测量。活动标签顶部 2px 浅灰条；选择树行左侧 2px 浅灰线；XYZ 分别 #ec958b/#add296/#88b9e4。

### Header 与窗口
每个窗口固定 Header，File/Edit/Add/Window 菜单与 Default/Scene/Assets segmented control。仅实现可执行菜单项；未实现播放不显示为功能按钮。新窗口共享世界/选择，拥有独立布局。支持保存、载入、复制布局、重置当前布局。错误与完成信息在 Status 显示，不谎报保存成功。

### 内容
World：展示独立游戏 World 的真实 ECS 层级、展开收起、过滤、单选，不靠 EditorObject 过滤冒充 World 隔离。Inspector：空选择提示；通过 bevy_reflect 发现实体真实组件与参数，支持的字段 Enter/失焦提交与数值拖动，修改写回游戏 World 并同步渲染。无效/非有限数字保留原值并显示错误。未注册反射或不支持编辑的字段明确只读；不要求任意组件增删。
Assets Tree：[assets](../../../../../../assets) 的目录及文件树，无缩略图、确定性目录优先排序、过滤、刷新、错误提示；不递归符号链接。Gallery：自适应卡片，103px 最小宽、11px gap、13px padding、缩略图宽高比 1.38、名称/类型；图片支持 png/jpg/jpeg，非图片使用文件类型图标。Tree 与 Gallery 只展示 assets/ 内容，不混入内置庭院模型库。
Viewport：独立编辑相机、网格、单选标记/XYZ 移动轴、拾取、对象拖动、右键 orbit/中键 pan/wheel zoom；编辑不更改游戏相机。Camera：绑定示例 Main Camera 姿态的实时预览，16:9 letterbox，无编辑 gizmo。

### 文本输入
搜索、名称及数值输入使用原生 TextInput 的焦点与键盘链路：点击聚焦并准备可见插入光标，接受文字/退格/方向键；Tab/Shift+Tab 在输入项间导航；控件为单行，Enter 提交而非插入换行。提交读取本帧原生文本编辑已经应用的值。点击另一输入框时提交旧字段，新的输入框继续保持焦点；UI 重建不丢当前草稿、插入位置或文字选择。匹配窗口、Layout、Pane、实体、字段和轴来恢复输入；已隐藏/移除字段不恢复过期焦点。预设切换导致搜索失焦时，提交写入来源 Layout，不能误写复用 Pane ID 的新 Layout。

### 光标反馈
可点击项为 Pointer，文本输入为 Text；Tab 栏与标签悬停 Pointer，拖动到有效同窗口目标为 Grabbing，无有效/跨窗口目标为 NoDrop；左右分隔条 ColResize，上下分隔条 RowResize；数值拖动 EwResize。Viewport 空白为 Default，可移动模型悬停 Grab，模型拖动与右键 orbit 为 Grabbing，中键 pan 为 Move；只读 Camera 预览为 Default。
拖动时覆盖悬停样式，文本选择拖动保持 Text。嵌套按钮/输入按最近控件决定光标；禁用/只读项不暗示可操作。仅物理鼠标及其所属 Viewport 的派生指针影响 OS 光标；触摸和无关离屏指针不得改变其他窗口。释放、PointerCancel、Escape、失焦或源节点销毁后不残留拖动光标；光标离开窗口时恢复默认。

## Boundaries, errors and compatibility
首版 Windows 桌面；小窗口可裁切过长标签但 Header/Dock 边界不重叠。不承诺 900px 以下完全复制 HTML 响应式细节。JSON version=1；拒绝未知版本、空布局、重复 ID、无效 active、NaN/Infinity/越界比例、深度超限/节点超限；导入错误不破坏当前状态。Entity ID 不存盘。文件 IO 不修改资产内容。Bevy 是 RC，不承诺未来版本源码兼容；`bsn!` 是编译时组合 API，`.bsn` 资产格式尚未发布。

## Acceptance scenarios
### S1 视觉与 API
实现依据由代理调研；最终 GUI 验收由用户负责（2026-09-30 明确要求）。
1440×900 打开 HTML，截图默认五个可见 Pane + Tree 切换 + 菜单 + Scene/Assets；1000×700 截图。保存 Chrome 实测 header/status/pane rect 及色彩/字体/间距。通过可编译示例验证 BSN 的 Children、组合和现代控件。
### S2 布局
数据模型通过自动化测试，以下鼠标交互留用户验收。
将 Gallery 拖到 World 中心后共享标签组，拖到右边形成左右分割；移动源最后一个 Tab 时提升兄弟，不丢失其他 Pane；同组中心 drop 无操作。关闭、修改类型、比例和 active 经 JSON roundtrip 保留。非法 JSON 不替换状态。
### S3 窗体与内容
Default 显示 World/Viewport/Gallery/Inspector/Camera，点击 Assets Tree 显示文件树。预设切换不销毁世界。打开新窗口拥有同样 Header；第二窗口切换布局不更改第一窗口。拖动 splitter 无世界变化；最大化保留 Header。
### S4 世界和资产
树选庭院物体，Inspector 显示实体真实 Name/Transform；输入 X/拖动对象后两个实时视图更新；关闭/重开 Pane 不改变实体。图像文件显示缩略图，非图像文件正常回退；无 assets 目录时明确提示。UI 实体不会出现在树中。

### S5 光标
自动化检查通过真实 Bevy picking 输入/命中边界验证光标组件的状态变更、子控件优先级、拖动覆盖、取消/释放/失焦/节点销毁恢复、离屏指针所属窗口与只读预览。实际 OS 光标外观及操作体验留用户验收，不进行代理 GUI 操作。

### S6 文本输入
使用实际编辑器输入构造器及 Bevy pointer focus、键盘分发、文本编辑和布局系统自动化验证：点击 → 焦点/插入光标布局 → 文字与退格；Tab/Shift+Tab；Enter/失焦提交；同帧文字与 Enter 不丢字符；UI 重建后保留焦点/草稿/插入位置；不同 Pane 的同名字段不串焦点；空搜索框的插入光标与提交、预设切换后搜索写入来源 Layout。实际 GUI 外观仍由用户验收。

## Current-document impact
交付后将可验证的当前行为同步至 [当前规格](../../../../../../doco/specs/editor.md)，记录 layout、editor、ui、scene、scene_input、icons、assets、cursor 模块边界与数据流，不把尚未实现的验收条件写成当前事实。
