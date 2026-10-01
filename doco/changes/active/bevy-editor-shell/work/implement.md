# Implementation design

## 1. Baseline and goals
变更前 [main](../../../../../src/main.rs) 只有打印。Cargo 已选择 Bevy 0.20.0-rc.2；AGENTS/doco 初始化文件是用户已有未提交内容，保留。目标契约见 [editor spec](specs/editor.md)。调研：2026-09-30 crates.io 最新预发布 0.20.0-rc.2、稳定 0.19.1。使用已下载 RC 源码/官方 examples 作为 API 真值。

## 2. Overall approach
- `layout`：与 UI 无关的可序列化递归树、ID、修改/合并/分割/验证及预设。
- `editor`：插件、窗口工作区、选择/菜单/拖动状态、持久化、事件路由与快捷键。
- `ui`：BSN 可复用控件、Header/Status、递归布局渲染、六种 Pane、原型主题。按 dirty 重建 UI，不每帧重建。
- `scene`：被标记的游戏实体、程序化庭院、相机及 gizmo；Editor UI 实体不混入 World Tree。
- `scene_input`：离屏 mesh pointer 的选择/移动；`icons`：代码原生线图标，不依赖字体符号。
- `cursor`：控件声明光标语义；由 Bevy picking 的 hover/drag 状态统一计算每个编辑器窗口的 CursorIcon。拖动优先、子控件优先；Viewport 派生指针仅通过物理鼠标所操作的 surface 映射到所属窗口。释放/取消沿 picking 生命周期恢复，Escape/失焦清理会话。
- `assets`：只读目录索引，跳过符号链接、确定性排序、图片资源句柄，目录错误展示；图片加载错误暂由 AssetServer 日志报告。
ECS 是世界实体属性真值；布局只存 Pane 状态，绝不序列化运行期 Entity ID。各窗口持有独立布局文档，选择共享。

## 3. APIs and data model
`PaneType` 六种枚举。`Pane {id, kind, query, collapsed, orbit, center}`；`TabGroup {id, tabs, active}`；`DockNode::Split {id, axis, ratio, a, b}` 或 `Group`。`Layout {name, root}` 与 `LayoutDocument {version, active, layouts}`，v1 JSON 只与原生编辑器兼容，不接受 HTML schema。加载先完整验证，再原子替换；失败保留当前布局。ID 唯一，有限比例、非空组、有效 active、深度及节点数量限制。写临时文件后 rename；错误不报告成功。
EditorUiPlugin 管理 UI 生命周期与原生文本提交：控件 TabIndex / UI root 的 Bevy 焦点 TabGroup 接入点击和键盘；插入光标用浅色 TextCursorStyle。PostUpdate 在 EditableTextSystems 后提交字段，后续 Update 重建 UI；未完成失焦提交时不销毁旧控件。InputBinding 的窗口/Layout/Pane/实体/字段与 FieldAxis 共同识别逻辑输入，重建迁移整个 EditableText 编辑状态并恢复 InputFocus；缺失目标清除焦点。查询写入来源 Layout，避免预设复用 Pane ID 时误写新布局。此运行态绑定不改变 Layout JSON v1。
运行态持有 window Entity、UI camera、focused group、maximized、menu 和 dirty。UI root 使用 `UiTargetCamera`，UI camera 使用 `RenderTarget::Window`；每个可见 Viewport/Camera 使用独立 RenderTarget Image + ViewportNode。重建释放旧离屏资源，窗口关闭清理其 UI 和相机。数值输入 Enter/失焦提交；非法/非有限值不写入 ECS，并保留输入草稿以便修正。渲染资源从 UI 状态分离，世界不会因布局切换销毁。

## 4. Algorithms and rules
标签中心拖入目标组，四边拖入新 Split；同组中心为无操作；移除空组时将兄弟提升，最后一个标签关闭后补 World。先验证目标与来源再修改，失败保持原树。分隔条沿轴调整比例并 clamp .08–.92；拖动期间更新几何，不重建导致丢失捕获。最大化仅过滤工作区，不影响 Header。菜单点击外部或 Escape 关闭。树选择更新全局选择；Viewport 拾取到子网格时向上寻找可编辑根实体。移动将屏幕增量变换为相机平面世界增量；Camera preview 不接编辑交互。

## 5. Fixed decisions and discretion
固定：原生 Bevy UI、精确锁定 RC、BSN 内联而非未发布 `.bsn` loader；不引入第三方 docking/egui；Split/Group/Pane 分离；JSON 只保存布局；首版单选/世界空间移动。局部控件抽取、程序化模型形状、图标实现可调整。图标采用代码原生符号，不生成位图。字体优先系统 Segoe UI（可回退 Bevy 默认字体）。没有阻塞性的开放 API 选择；通过 [BSN UI 示例](../../../../../examples/bsn_ui.rs) 验证组合场景。

## 6. 用户追加方向（待实现）
原单 World、固定 Inspector 和 Built-in Gallery 的边界已被用户新要求覆盖。追加任务：真实编辑器/游戏双 World 与渲染桥接、反射组件发现/编辑、assets-only 面板。具体双 World 桥接方案仍在调查，不能将 marker 过滤称为双 World；完成前当前架构仍记录现有事实。光标反馈独立交付，不依赖尚未完成的双 World 改造。

## 7. Verification and documentation impact
布局测试守护跨组移动/空组提升/重复 ID/不合法数据/保存载入不丢状态等可观察不变量，不测试内部函数调用形状。`cargo check --all-targets`、`cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`doco check bevy-editor-shell`。Chrome MCP 截图及 Bevy 桌面 cutty 截图单独记录；未实际手动/自动运行的交互不标已验证。更新 [当前架构](../../../../../doco/architecture.md) 和已实现部分的 [当前规格](../../../../../doco/specs/editor.md)，未完成契约留 work。不自动完成变更。用户已明确自行验收交互与视觉；代理负责编译、边界测试、规格记录，不再执行 GUI 操作验收。
