<div align="center">

# Bevy Editor Prototype

</div>

<div align="center">

[English](../README.md) | 简体中文 | [日本語](ja-JP.md) | [Français](fr-FR.md)

</div>

一个实验性的桌面世界编辑器，使用 Bevy **0.20.0-rc.2**、BSN 和原生 UI 控件，不使用 egui 或 WebView。

这个原型用 Bevy UI 探索交互、布局、样式和游戏引擎编辑器的设计想法，不打算将其维护为可供实际使用的编辑器。

<p align="center">
  <img src="screenshot_0.png" alt="Koiro 桌面编辑器">
</p>

## 快速开始

### 安装

使用 Rust **1.96+** 从源码运行；克隆和构建步骤见[从源码构建](#从源码构建)。在项目根目录运行：

```sh
cargo run
```

### 更新

先提交本地修改或用 stash 保存，再在克隆的仓库中运行：

```sh
git pull --ff-only
cargo run
```

## 功能

- 可停靠面板、布局预设，以及共享同一个世界的多窗口。
- 世界层级树、名称与变换编辑，以及移动、旋转、缩放 gizmo。
- 资产浏览器、内置模型、图片预览和实时相机预览。

## 操作与限制

在视口中，右键拖动环绕，中键拖动平移，滚轮缩放。点击物体选中，拖动 gizmo 手柄编辑变换。Inspector 字段按 Enter 或失焦时提交，Escape 取消编辑。

**Ctrl+S 只将布局保存**到 `editor-layout.json`。场景修改仅在当前会话中有效。尚未实现场景保存、撤销/重做、播放模式或通用组件编辑。

## 从源码构建

需要 Git 和带 Cargo 的 Rust **1.96+**。Bevy 固定为 **0.20.0-rc.2**。

```sh
git clone https://github.com/gloridifice/bevy-koiro-editor.git
cd bevy-koiro-editor
cargo build --release --locked
```

在项目根目录运行 `cargo run --release`，以便编辑器找到 `assets/`。

## 许可证

项目未声明整体许可证。内置的 Lucide 图标保留其[上游许可证](../src/lucide_icons/LICENSE)。

## 开发

- [架构](../doco/architecture.md)
- [编辑器契约与限制](../doco/specs/editor.md)
- [维护与验证指南](../AGENTS.md)
- [BSN UI 示例](../examples/bsn_ui.rs)
