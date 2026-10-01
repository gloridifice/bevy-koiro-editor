# Koiro · Bevy World Editor

参照 `prototype/index.html` 实现的原生 Bevy UI 编辑器首版，使用 Bevy **0.20.0-rc.2**、BSN 与原生 UI 控件，不使用 egui/WebView。

## 运行
需要 Rust **1.96+**。在项目根目录运行：

```sh
cargo run
```

BSN 控件验证原型：

```sh
cargo run --example bsn_ui
```

Default / Scene / Assets 为布局分段选择；Tab 的下拉箭头修改 Pane 类型，`+` 增加 Pane，标签栏直接提供最大化/恢复，`···` 提供分割等操作。宽度小于 900px 时通过 PANES 切换聚焦工作区，小于 640px 时顶栏分为两行；不改变已保存的停靠比例。File 菜单及 Ctrl+S 保存布局到 `editor-layout.json`；只保存布局，不保存场景修改。

文本框点击聚焦，显示浅色插入光标；Tab/Shift+Tab 切换字段。Inspector 字段 Enter 或失焦提交，非法输入保留草稿并显示错误，Escape 恢复已提交值；UI 刷新保留当前输入、焦点和内容区滚动位置。拖动 XYZ 标签调整数值，内容超高时使用原生滚动条。Viewport 支持右键 orbit、中键 pan、滚轮缩放和点击选择物体；拖动 Mesh 不移动物体，只通过 gizmo 手柄操作 Transform。工具栏可切换原生 gizmo 的移动/旋转/缩放及 World/Local 轴，Escape 取消手柄拖动。手柄显示在最后操作的 Viewport 中，不进入 Camera 预览。Window → New Editor Window 创建独立布局、共享世界的新窗口。

Tab 栏与 gizmo 手柄悬停使用 Pointer，手柄拖动使用 Grabbing；按钮、输入、分隔条、数值拖动和视口操作已接入对应系统光标；拖动期间保持操作样式，释放/取消/失焦后恢复。真正双 World、反射 Inspector 和 assets-only 面板的追加改造仍待交付。

UI 保留密集的桌面编辑器风格：4/8/12px 的内部/控件/面板间距、16px 层级缩进（World 分组与直属实体同列）、24px 图标按钮和 26px 输入框；输入框左右内边距 8px、文字垂直居中，普通边框保持低对比度。悬停图标、搜索或字段查看提示；Help 提供操作与保存范围说明弹层。

## 原生 UI 测量

```sh
cargo run --example ui_audit
# 快照：系统临时目录/koiro-ui-audit.json；PNG 必须匹配窗口、尺寸和 DPI
cutty --pid <进程号>
python scripts/ui_audit.py <快照.json> <全尺寸截图.png> --output <临时目录中的输出前缀>
```

Python 工具需要 Pillow。报告实际节点边界、padding/gap、裁切、碰撞、对比度、桌面鼠标目标尺寸和像素重心，并输出标记图。弹层叠放和滚动裁切需结合截图判断；像素对称不作为编辑器布局门槛。

## 图标
所有编辑器 UI 图标使用 `src/lucide_icons` 中的 Lucide SVG，编译时嵌入实际使用的图标；无需将资源复制到 `assets` 或随可执行文件携带 `src`。`resvg` 按目标窗口 DPI 与 UI 尺寸栅格化，通过原生 `ImageNode` 显示、着色并共享纹理缓存。该目录中的 JSON 为上游元数据，不参与运行时渲染；许可见 [`src/lucide_icons/LICENSE`](src/lucide_icons/LICENSE)。

## 规格与验收
- [当前详细规格](doco/specs/editor.md)
- [当前架构](doco/architecture.md)
- [doco 变更](doco/changes/active/bevy-editor-shell/proposal.md)
- [原型分析与截图](doco/changes/active/bevy-editor-shell/work/research.md)
- [交付检查与用户验收项](doco/changes/active/bevy-editor-shell/work/tasks.md)

**交互和视觉待用户验收。** 当前是首版：没有播放模拟、撤销、通用反射组件增删、框选/多选 gizmo 或场景文件保存。不要将会话内场景修改当作已持久化。

```sh
cargo test
cargo check --all-targets
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```
