# 编辑器首版规格

> 本文记录当前代码契约。原生窗口截图、独立视觉审计与布局测量已覆盖内置预设和 1440/1024/760/390px 工作区；这不替代用户对完整操作流程的验收。原型证据与验收清单保留在 active 的 bevy-editor-shell 变更中。

## 1. 概念与数据所有权
- **PaneType**：World、Inspector、AssetsTree、AssetsGallery、Viewport、Camera 六种类型。
- **Pane**：有稳定布局 ID 的内容实例；持有类型、搜索字符串、收起项、编辑相机 orbit 和 center 状态。Pane 不拥有游戏实体数据。
- **Tab**：Pane 的标签入口。一个 Tab 对应一个 Pane。
- **TabGroup**：一个或多个 Tab 与一个 active Pane ID；同一时刻仅显示 active Pane。
- **DockNode**：Group 或 Split。Split 是包含 axis、ratio、a、b 的二叉布局节点；X 为左右分割，Y 为上下分割。规格中“Tab 包含多个 Tab”落实为 TabGroup，而非递归标签嵌套。
- **Layout**：名称与 DockNode 根树。
- **LayoutDocument**：version、active 布局下标和 layouts。
- **Header/Status**：每个窗口的固定 UI，不是 Pane/Tab，不参与分割、停靠或最大化。
- **Window**：每个窗口独立持有布局文档、菜单和焦点；所有窗口共享游戏世界与单选 Selection。

## 2. 布局行为
标签可切换、增加、关闭和改变 PaneType。关闭组中的最后一个标签会移除空组并提升兄弟；关闭整个工作区最后一个标签时补一个 World Pane。可通过 Pane 菜单增加左右/上下分割、最大化/恢复。

标签拖放目标的中心区域合并标签组；左右/上下边缘形成新的 Split。边缘判定为本地宽/高的前后 22%，先判左右，再判上下；其余是中心。四边新分割比例为 0.5。同组中心放置，以及单标签组向自身边缘放置均不改树。来源/目标不存在时拒绝修改；不支持跨 OS 窗口拖动。

分隔条拖动更新本地比例，限制到 0.08–0.92。分隔条与外边距均为 5 个逻辑像素。窗口宽度小于 900px 时，工作区展示当前焦点组，32px 的 PANES 切换栏提供所有组的入口；切换组退出显式最大化。该响应式展示不改写 DockNode、比例或 Layout JSON，恢复宽窗口后仍显示原停靠布局。手动把组分割到极窄宽度仍可能裁切内容，不强制 Pane 最小宽度。

| 预设 | 递归比例与内容 |
| --- | --- |
| Default | 左 World 18.5%；其余中 72%、右 28%；中上 Viewport 68.5%、下 Assets Gallery/Assets Tree；右上 Inspector 65.5%、下 Camera |
| Scene | 左 World 17.5%；其余中 77% 的 Viewport/Camera 标签组，右 Inspector |
| Assets | 左 Assets Tree/World 18.5%；其余中 72% 的 Gallery；右上 Inspector 55%、下 Viewport/Camera |

所有比例都作用于当前 Split 的可用长度，不是全窗口百分比。最大化只改变工作区显示，不销毁布局或世界。Header 的 Segmented Control 选择预设；+ 复制当前布局为 Custom。重置内置预设恢复其模板；重置 Custom 使用 Default 模板并保留名称。

## 3. 样式尺寸
尺寸均为逻辑像素，跟随窗口 DPI；Windows 200% 缩放对应两倍物理像素。

| 部分 | 尺寸 |
| --- | --- |
| Header / Status | 40 / 26px；宽度小于 640px 时 Header 分为菜单与 Layout 两行，总高 76px |
| Tab strip | 34px；active 顶边 2px |
| 标准工具栏 / Viewport / Camera 工具栏 | 36 / 37 / 31px |
| Pane footer | 无；仅保留窗口级 Status |
| Pane | 1px border、5px radius |
| Pane 内容区 | 上描边 1px，作为标签栏与内容的分隔线 |
| 字段 / 搜索 | 均为 26px 高、4px radius；左右内边距 8px、上下 5px、单行行高 14px，文字与占位文字垂直居中；图标按钮 24×24px |
| World 行 / Assets Tree 行 | 均为 24px；World 展开按钮和叶子占位均宽 16px，嵌套层级缩进 16px |
| Gallery | 最小列宽 103px、gap 12px、padding 12px；卡片 padding/gap 4px，缩略图宽高比 1.38 |
| 字体 / 图标 | 正文 11px、次级 9–10px；系统 Segoe UI；图标统一为 Lucide SVG，经 resvg 栅格化后使用原生 ImageNode 显示 |
| 类型菜单 | 宽 560px，Scene/Data/Assets 三列，各列宽 170px；窄于 580px 时改为纵向分类，菜单宽度限制在窗口内且窄屏最多 258px，超高时纵向滚动；宽屏行高 28px、窄屏 24px |

间距层级：内部微间距 4px、控件间距 8px、面板内容边距 12px、树层级缩进及空状态边距 16px。Inspector 名称与实体 ID 相隔 4px；Transform 标题与轴字段相隔 4px，三组属性相隔 12px，三轴间距 4px。窄字段区域允许轴单元换行，数值不再被强行压成一两个字符。

颜色：背景 #141414、面板 #202020、字段 #181818、字段普通边框 #484848 / 焦点边框 #9c9c9c、hover #303030、线条 #363636、主文字 #e5e5e5、次文字 #9c9c9c、accent #d6d6d6。XYZ 为 #ec958b / #add296 / #88b9e4。示例项目为 Quiet Courtyard。顶部不显示品牌图标、编辑器品牌文字或项目名，File 等菜单从左侧开始。

编辑器 UI 的 Pane、实体/文件树、工具栏、状态与菜单图标统一使用 Lucide，不使用字体符号或 emoji 代替图标。图标随目标窗口 DPI、UiScale 和节点尺寸重新栅格化；相同图标及物理尺寸共享纹理，改色和普通同帧 UI 重建不重复栅格化。资源在编译时嵌入，不要求运行时携带 src/lucide_icons；该目录的 JSON 元数据不参与渲染。SVG 并非无限缩放的实时矢量，单个栅格化纹理边长上限 512px。系统鼠标光标仍由 OS 提供，普通文字及文字中的标点保持文本。

首版不复制原型所有装饰：Gallery 没有左侧分类导航；Tab 关闭按钮始终可见；Inspector 不仿造不可编辑的材质字段；不展示假 FPS、假保存状态或不可用播放按钮。

## 4. Header 与菜单
- File：保存 Layout JSON、加载 Layout JSON、新建编辑器窗口。
- Edit：删除所选模型；Group、Camera、Light 不支持删除，Status 明确提示。
- Add：Cube、Arc Pavilion、Ginkgo、Orbit Sculpture。
- Window：增加 Pane、新窗口、复制/重置 Layout、操作帮助。
- 每个新窗口都有同样的 Header。新窗口复制当前布局文档，此后其布局修改不影响其他窗口。
- 菜单为非模态弹层，不通过全屏透明控件拦截输入。点击其他菜单入口直接切换，点击当前入口收起；点击弹层外部或 Escape 关闭。外部按钮和输入框在同一次点击中正常执行动作或获得焦点，点击弹层标题/留白不关闭菜单。Ctrl+S 保存当前获得 OS 焦点窗口的布局。输入框获得焦点时 Delete 不删除世界物体。
- Header/Pane 菜单锚定触发控件并限制在窗口内；Pane 标签栏提供直接最大化/恢复按钮。
- 图标、搜索和字段提供悬停提示：首次等待约 500ms，提示出现后的短时间内切换其他控件立即显示。菜单打开、按住鼠标、标签拖动和字段编辑时不显示提示遮挡操作，仅保留当前错误字段的校验提示。提示优先显示在目标上方，空间不足时翻到下方并限制在窗口内；被提示控件保留时，状态或校验消息改变也同步更新当前提示。
- Help 显示独立的可关闭操作弹层。Status 显示完成与错误信息；长消息可横向滚动，也可悬停阅读。场景编辑是会话内数据，不声称已保存。

## 5. 六种 Pane
所有搜索、名称和数值输入为原生单行 TextInput：点击获得焦点并显示浅色插入光标，支持文字、退格及插入位置移动，Tab/Shift+Tab 切换字段。Enter 与点击其他控件失焦后提交；先应用本帧原生文本编辑，再读取提交值，不丢最后一个字符。非法输入不写入原属性，重新聚焦错误字段并保留草稿、错误描边和提示；Escape 丢弃草稿并恢复已提交值。焦点字段使用浅色描边。

普通菜单、状态、属性、可见性及 gizmo 工具栏更新不重建 UI 外壳；属性值原地同步，不覆盖焦点字段或非法草稿。搜索/折叠、树结构、资产索引及 Inspector 选择变化只更新相关内容区，保留滚动容器、轨道和滑块。重复选择、无变化动作及分隔条释放不销毁控件。单窗口布局操作或响应式变化不重建其他窗口；同一响应式模式内的窗口缩放仅更新几何。只有 Layout 名称/下标、Pane/停靠结构、可见组或响应式模式变化才重建对应窗口的外壳。

必要的结构性 UI 刷新仍保留当前字段焦点、草稿、插入位置及文字选择；恢复按窗口、Layout、Pane、实体、字段和轴匹配，重复 Inspector 不串焦点。字段隐藏或移除时清除旧焦点。切换预设产生的搜索失焦提交写入来源 Layout，而不是新激活的 Layout。插入光标布局与输入链路经过 headless 自动化检查；实际 GUI 绘制仍由用户验收。

### World Inspector
只展示 EditorObject 标记的游戏根实体，包括 Group、相机姿态与灯光；UI、缩略图世界和子网格不进入树。按创建 key 排序，通过 ChildOf 组织层级。World 分组行作为区块标题：文件夹与直属非 Group 实体共享图标和文字列；嵌套 Group 及普通实体的后代仍按 16px 缩进，不修改实际 ECS 父子关系。树行左内边距为 4px，展开按钮及叶子占位宽 16px，展开按钮高 24px；命中区到图标间距 4px，图标到文字间距 8px。点击选择、组展开/收起；输入查询并 Enter/失焦应用过滤。选择共享给所有窗口的 Inspector/gizmo。收起 key 是示例世界的逻辑 key，不是通用场景 UUID。

### Entity Component Inspector
无选择时显示提示。Name 非空字符串、Visibility 开关、Transform position/rotation/scale 是实际 ECS 编辑。旋转显示 XYZ 欧拉角（度），存储为 Quaternion。Transform 标签位于三轴字段上方，旋转标签明确标记 °。无选择时显示带边距、可换行的空状态。字段 Enter 或失焦提交；NaN/Infinity/无法解析的字符串拒绝写入并显示错误；scale 必须 > 0。提交绝对数值直接赋值，不通过差量转换，避免两个有限大数之差溢出。

X/Y/Z 标签水平拖动：position/scale 为 0.01 单位/像素，rotation 为 0.5 度/像素；scale 拖动最低 0.001。其余组件仅显示类别与只读说明。没有任意组件增删或通用反射编辑。

### Assets Tree
扫描当前工作目录的 assets 文件夹。目录优先、名称不区分大小写排序，显示路径层级，没有缩略图。支持搜索与刷新；当前目录均展开，不提供目录收起控件。跳过符号链接与特殊文件，最多 2,000 条/12 层。目录不存在、读取失败、达到上限在刷新索引后通过窗口级 Status 明确提示。不会修改、移动或删除文件。

### Assets Gallery
内置模型展示真实离屏渲染缩略图，标记 BUILT-IN；点击实例化模型并选中。png/jpg/jpeg 本地文件通过 AssetServer 展示图片，其他文件使用文件类型线图标。支持过滤、刷新和纵向滚动；图片解码/加载错误由 Bevy AssetServer 记录，当前未实现错误缩略图替换。内置库与本地文件在同一网格，内置库不是本地目录伪装。本地文件名称与路径允许换行，避免长路径溢出卡片。

### Viewport
独立编辑相机，通过 ViewportNode 显示实时 RenderTarget Image。世界网格、选择包围标记与原生 TransformGizmoPlugin 手柄只出现在编辑视图。包围标记仍是示意形状，不是精确 mesh AABB；手柄位于所选根实体的世界原点，通过投影命中与拖动约束编辑实际 Transform，始终覆盖在场景上方，不作为世界 mesh 拾取目标。

Viewport 与 Camera 的可见视图在菜单、选择和字段提交等增量刷新时不被替换；在必要的结构性 UI 外壳刷新时也保留节点、拾取指针、相机及目标纹理，不重置纹理尺寸或渲染状态。复用身份包含窗口、Layout、Pane ID 和视图类型，不跨窗口或预设串状态。视图不再可见、关闭、切换 Layout 或改变类型时释放对应资源；布局重置/加载仍应用文档中的相机姿态。

左键点击网格，沿父链解析可编辑根；拖动网格不会修改 Transform，Viewport 的物体移动/旋转/缩放只由 gizmo 手柄驱动。右键 orbit、中键 pan、wheel zoom；距离限制 2–80，pitch 限制 0.05–1.5 radians。每个 Pane 的 orbit/center 在拖动过程中同步布局文档并随布局保存；相机拖动结束不重建 UI 外壳，不重置其他 Pane 的滚动条、滑块几何或滚动位置。编辑相机操作不修改游戏相机姿态。工具栏提供 Move / Rotate / Scale 和 World / Local 轴切换；模式和轴空间在窗口间共享，仅在本次会话生效，不写入 Layout JSON。缩放始终使用局部轴。移动轴、屏幕平面移动环、旋转环、局部缩放轴和中心均匀缩放均可拖动；中心均匀缩放按水平鼠标位移调整。手柄输入只由操作该 Viewport 的物理鼠标驱动，适配 Pane 偏移、DPI、纹理尺寸及非主窗口；拖动期间锁定来源视图和轴，不切换模式。手柄悬停/拖动及释放不触发其下方模型的选择。父变换用于世界移动/旋转到局部 Transform 的转换；奇异父变换不写入，无限值/NaN 拒绝写入，缩放最小 0.01。Escape 恢复拖动前 Transform；释放完成操作，PointerCancel、失焦、来源消失或选择改变结束会话，保留已应用的值。结束后刷新 Inspector。

原生手柄仅显示在最后操作的 Viewport 中；未操作前取一个可见 Viewport。原生 overlay 跟随该视图的 RenderTarget、Projection 和姿态，并使用独立层 2，避免进入 Camera 预览或内置资产缩略图。关闭/隐藏来源后切换到其他可见 Viewport；无 Viewport 或无选择时停用 overlay。仍只支持单选，不包含框选、多选手柄或撤销。

### Camera
实时显示示例 Main Camera 姿态对应的世界，16:9 letterbox。与 Viewport 使用独立相机和纹理，不读取其截图。排除 gizmo 渲染层；不处理物体选择/拖动或编辑 orbit。首版只支持示例 Main Camera，尚无任意游戏相机选择器。

World、Inspector、Assets Tree 和 Gallery 使用原生 Scrollbar/ScrollbarThumb：只在内容超出可视高度时显示，可拖动或点击轨道翻页。增量刷新保留滚动条及滑块实体，只随真实内容/几何变化更新尺寸与位置。必要的结构性 UI 外壳刷新按窗口、Layout 和 Pane 恢复可见内容区滚动位置；不将滚动位置写入 Layout JSON。

## 6. 光标反馈
通过各编辑器窗口的 Bevy CursorIcon 使用系统光标，具体外观由 OS 决定。

| 状态 | SystemCursorIcon |
| --- | --- |
| 按钮、菜单、可点击树行 | Pointer |
| 文本输入、文本选择拖动 | Text |
| Tab 栏及标签悬停 / 拖到有效同窗口目标 / 无效或跨窗口目标 | Pointer / Grabbing / NoDrop |
| 左右 / 上下分隔条悬停及拖动 | ColResize / RowResize |
| 数值标签悬停及拖动 | EwResize |
| Viewport 空白 / 可选择模型悬停 | Default / Pointer |
| gizmo 手柄悬停 / 拖动 | Pointer / Grabbing |
| 右键 orbit / 中键 pan | Grabbing / Move |
| Camera 预览、静态文件、禁用控件 | Default |

最近子控件优先于祖先；Tab 栏空白、标签及其关闭/下拉按钮均为 Pointer。拖动期间覆盖新悬停控件的光标，直到释放、PointerCancel、Escape、失焦或源节点销毁；窗口外恢复 Default。失焦后再聚焦不会恢复旧拖动。标签在另一窗口释放不使用之前的停靠目标。

仅物理鼠标及其所操作 Viewport 的派生指针影响 OS 光标；触摸、无关离屏 pointer 或只读 Camera 不能覆盖另一窗口光标。实际 OS 外观和交互感受仍由用户验收。

## 7. Layout JSON v1
工作目录文件 editor-layout.json 保存发起保存动作的窗口的整个 LayoutDocument，不保存世界实体、选择、运行期 Entity ID、相机实体或纹理。无自动保存。多个窗口共用文件，最后一次显式保存覆盖之前内容。

载入先完整解析和验证，成功才替换当前窗口文档；失败保留原布局并显示错误。启动载入无效时回退默认布局，显示原因。保存先验证，写入 editor-layout.json.tmp 后 rename；IO 失败不显示成功。

边界：version 必须 1；1–20 个 Layout，active 下标有效，名称非空且最多 100 bytes；每棵树所有 node/pane ID 唯一且在 1..=1,000,000,000；Group 1–30 个 Pane 且 active 有效；总 node/pane 不超过 150、深度不超过 18；ratio 有限且 0.08–0.92；query 最多 200 bytes、collapsed 最多 500 个 key；orbit/center 有限，orbit pitch/distance 满足上述范围；输入 JSON 最大 1 MiB。未知版本与 HTML 原型 schema 不兼容。

最小文档形状：
```json
{
  "version": 1,
  "active": 0,
  "layouts": [{
    "name": "Default",
    "root": {
      "node": "Group",
      "id": 1,
      "tabs": [{"id": 2, "kind": "World"}],
      "active": 2
    }
  }]
}
```
query/collapsed/orbit/center 可省略，使用默认值。Split 形状为 node="Split"、id、axis="X" 或 "Y"、ratio、a、b。

## 8. 验证状态与非目标
编译、格式、clippy 与布局/持久化/多窗口状态/Inspector 数值/光标/pointer/原生文本输入及重建生命周期边界测试通过；图标增加了实际 SVG 栅格化、白色透明抗锯齿、原生多窗口 DPI/UiScale/尺寸变化及纹理复用测试，包含视口节点、相机和纹理复用，以及隐藏/关闭视图与窗口关闭时的资源清理。原生 gizmo 的 CPU 集成测试覆盖偏移与 200% DPI 的非主窗口输入、带父变换的移动/旋转/局部轴与均匀缩放、Escape 回滚、手柄 Pointer 光标、底层 mesh 输入隔离、PointerCancel/失焦结束及 overlay 与库缩略图隔离；原生窗口截图确认手柄与工具栏渲染。增量刷新回归测试覆盖菜单/状态/属性/gizmo 工具栏、搜索/折叠/资源索引、相机/分隔条/数值拖动、共享 Inspector、多窗口隔离、响应式断点和动态状态提示；检查控件与滚动条保留、实际值更新及必要的结构重建。测试不等价于 GUI 验收，标签拖放、输入焦点、滚动、Viewport 拾取/移动和多窗口视觉等仍交用户确认。响应式布局经真实原生 UI 布局/命中测试检查控件可达性和停靠文档不变；输入校验测试覆盖失焦错误草稿保留与 Escape 取消。

视觉复验工具：`cargo run --example ui_audit` 每秒在系统临时目录写入 `koiro-ui-audit.json`，记录原生 ComputedNode 的实际边界、padding/gap、文字颜色和滚动位置。配合全尺寸 cutty 截图及 `python scripts/ui_audit.py <json> <png> --output <临时目录中的输出前缀>`（需要 Pillow）输出带标记截图与测量结果。碰撞检查区分弹层与底层；对比度、裁切、24px 桌面鼠标按钮下限和像素重心分别报告。重心与间距规律只作信号，不以对称布局替换编辑器的任务结构。

无场景保存、撤销/重做、播放模拟、跨窗口拖放、模型导入转换、资产文件写操作或生产级扩展 API。真正双 World、反射组件编辑与 assets-only 面板的追加改造尚待交付，当前实现边界如上；目标记录在 active 变更。Bevy 是 RC；升级后必须重新验证 API。BSN 使用内联宏，不提供未发布的 .bsn 文件 loader。
