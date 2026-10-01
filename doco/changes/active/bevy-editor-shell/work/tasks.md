# Execution tasks

- [x] 1.1 分区检查 HTML 并调研 Bevy UI/BSN
  - Acceptance: [视觉与 API](specs/editor.md#s1-视觉与-api) 的截图、实际尺寸、版本与源码依据齐全；见 [research](research.md)。
  - Verification: 通过 stdio 调用 chrome-devtools-mcp 1.10.1；1440×900 分区截图/菜单/预设、1000×700 截图及 DOMRect；BSN 原型已编译。
- [x] 2.1 实现布局模型、修改与 JSON 边界
  - Dependencies: 1.1
  - Acceptance: [布局](specs/editor.md#s2-布局) 的数据模型与变更 API 已实现，模型/持久化边界测试通过；实际鼠标验收留 3.2。
  - Verification: 布局测试覆盖跨组/四边移动、空组提升、同组与失败 drop 不变、关闭最后 Tab、JSON roundtrip、重复 ID/非法文档；真实文件测试覆盖覆盖保存、临时文件清理、失败不破坏原文件。
- [x] 2.2 实现原生 UI、六类 Pane 与多窗口 Header
  - Dependencies: 2.1
  - Acceptance: [窗体与内容](specs/editor.md#s3-窗体与内容) 的代码与窗口状态边界已交付；GUI/视觉验收留 3.2。
  - Verification: cargo check --all-targets 通过；真实 ECS 测试证明新窗口共享选择、独立布局；examples/bsn_ui 可编译。
- [x] 2.3 实现世界联动、实时渲染与资产索引
  - Dependencies: 2.2
  - Acceptance: [世界和资产](specs/editor.md#s4-世界和资产) 的代码已交付；操控体验留 3.2。
  - Verification: 本机运行产生默认/资产布局截图，包括实际庭院渲染、单独预览、本地 checker.png 和 notes.txt 回退。Inspector Enter/失焦、非法数字及有限大数溢出回归测试通过。
- [x] 2.4 实现状态光标与每窗口路由
  - Dependencies: 2.3
  - Acceptance: [光标](specs/editor.md#s5-光标) 的状态、拖动覆盖与生命周期自动化检查通过；实际 OS 外观留用户验收。
  - Verification: 3 个光标测试经真实 PointerInput/PointerHits 管线验证悬停、嵌套/禁用优先、释放/取消/Escape/失焦/销毁/离窗、有效/无效目标、文本选择与数值拖动、Viewport/Camera、触摸与窗口路由；1 个实际 PointerDragEnd observer 回归验证跨窗口拒绝旧目标且同窗口仍可停靠。
- [ ] 2.5 实现独立游戏 World 与渲染/拾取桥接
  - Dependencies: 2.3
  - Acceptance: [世界和资产](specs/editor.md#s4-世界和资产) 的追加双 World 契约；先确定桥接方案并更新设计。
- [ ] 2.6 实现 bevy_reflect 世界树/组件参数编辑
  - Dependencies: 2.5
  - Acceptance: 真实组件发现、支持字段编辑写回游戏 World、不可编辑内容只读及数值安全边界。
- [ ] 2.7 移除资产面板 Built-in 内容
  - Dependencies: 2.3
  - Acceptance: Tree/Gallery 仅展示 assets/，保留图片预览/非图片回退/搜索/刷新/错误状态。
- [x] 2.8 修复原生文本输入并调整 Tab 栏光标
  - Dependencies: 2.2, 2.4
  - Acceptance: [文本输入](specs/editor.md#s6-文本输入) 的实际点击/键盘/插入光标布局与重建生命周期自动化检查通过；Tab 栏悬停为 Pointer。
  - Verification: 3 个 headless 原生 UI 测试使用实际编辑器构造器/EditorUiPlugin 与 Bevy pointer focus、键盘分发、文本编辑、插入光标布局系统；覆盖点击、文字/退格/Home、Tab/Shift+Tab、空搜索框、同帧 Enter、失焦、草稿/插入位置/焦点保留、非法数值、重复 Inspector、隐藏字段和预设复用 Pane ID 的来源布局提交。原光标测试已按 Pointer 新契约更新并通过。
- [ ] 3.1 运行总体检查并同步当前文档
  - Dependencies: 2.4, 2.5, 2.6, 2.7, 2.8
  - Acceptance: 编译、边界测试、格式、clippy、git diff --check 与 doco check 实际通过；当前架构/规格/README 完整且不宣称 GUI 验收通过。
  - Verification: 首版检查曾通过；追加范围完成后重新验证。
- [ ] 3.2 用户进行交互与视觉验收
  - Dependencies: 3.1
  - Acceptance: 用户确认下列 checklist；未确认前不 complete/archive。
  - Owner: 用户（2026-09-30 明确要求自行验收）

## 用户验收清单
1. 在项目根目录 cargo run；对照原型检查 Default/Scene/Assets 的 Header、比例、灰阶、文字、字段与六种 Pane。
2. 切换/关闭/增加 Tab；修改 PaneType；左右/上下分割；调 splitter；最大化与恢复后 Header 保留。
3. Gallery Tab 拖到 World 中心合组；拖四边拆分；源最后一个 Tab 移走时正确提升兄弟；Escape 取消/点击外部关闭菜单；无悬挂停靠遮罩。
4. World 选择同步 Inspector；点击文本框出现插入光标，可输入/退格/移动插入位置；Tab/Shift+Tab 切换字段，Enter 与失焦提交后仍能继续输入；非法数字不写入；拖 XYZ 调整。
5. Viewport 选择与移动模型，RMB orbit/MMB pan/滚轮 zoom；游戏相机预览不跟随编辑相机，Camera 中没有 gizmo；切换 Pane 不丢世界修改。
6. Assets Tree 为本地文件树且无缩略图；Gallery 图片与其他文件回退；搜索/刷新/滚动，窄 Pane 和小窗口裁切是否可接受。
7. Window 新窗口有独立 Header；第二窗口切换布局不影响第一窗口，选择与世界修改共享；关闭窗口不影响剩余窗口。
8. Ctrl+S/File Save 保存布局，File Load 还原；场景修改不声称保存；损坏/未知版本 JSON 显示错误且不破坏当前布局。
9. 检查 Tab 栏悬停 Pointer，以及按钮/输入/横纵 splitter/数值拖动/Viewport 的光标；拖出原控件仍保持拖动样式，释放/Escape/失焦后恢复；Camera、只读文件和其他窗口不显示错误操作光标。
10. 追加双 World/反射/资产调整交付后，确认世界树列出游戏 World 全部实体层级、Inspector 真实组件/参数可编辑、Tree/Gallery 没有 Built-in 内容。

## Verification
Blocked: none
- cargo check --all-targets：已通过。
- cargo test：14 passed，另含 binary/doc-test 空测试套件；无失败。
- cargo build：最新二进制构建通过（本次测试进程已退出，没有继续 GUI 验收）。
- cargo fmt --check / git diff --check：通过。
- doco check bevy-editor-shell：本次机械检查通过（6/11）；预期提示未完成 2.5/2.6/2.7/3.1/3.2，不代表语义/GUI 验收通过。
- cargo clippy --all-targets -- -D warnings：已通过。
- 有限大数绝对输入回归：修复前 inspector 测试实际得到 -inf（失败），改为绝对赋值后通过。
- 开发截图只是渲染 smoke evidence；最新 Tab/menu/scroll 路由调整后不再进行桌面操作，GUI 验收明确留给用户。
- 光标回归探针：暂时停用光标更新，3 个测试分别在 Default ≠ Pointer/ColResize/Grab 处实际失败；恢复后通过。去掉跨窗口 release 保护，observer 回归实际修改布局并失败；恢复后通过。探针均已撤回，未保留测试开关。
- 文本输入回归：修复前实际点击后 InputFocus 为 None；补焦点后发现同帧 Enter 提交得到 Cube 而非 CubeX、UI 重建后焦点引用已销毁节点。来源布局漏绑定时查询仍为 Cube 而非 NCube；省略 Pane 身份会把焦点从 Pane 12 串到 10，取消失焦提交延后保护会丢掉 YCubeX 草稿。修复/恢复后通过；未保留探针开关。
- 光标与文本输入交付仅做编译/headless 自动化检查，没有操作 GUI；检查的是插入光标布局数据，实际 OS 光标/插入光标绘制与体验由用户验收。
- 2.5/2.6/2.7 尚未实现，3.1 待追加范围完成后重验；变更保持 active、Result Pending，不自动 complete/archive。
