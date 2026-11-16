# 灵动岛原型

使用 Rust、egui 和 eframe 构建的跨平台桌面悬浮胶囊，面向 macOS、Windows、Linux。界面参考岛仔的深色圆角胶囊与轻量信息栏样式；窗口不吸附屏幕边缘，可以拖放在桌面任意位置。

## 功能

- 采用岛仔同类的透明眼睛、嘴巴表情序列，自动播放眨眼、张望、害羞等动作；点击可切换，胶囊可自由拖动。
- 收起态只保留表情、电池和提示箭头；鼠标停留约 0.18 秒后弹簧展开，移开后延迟收起，展开态显示性能卡片。
- 显示系统电量和充电状态。
- 展开后查看 CPU、GPU、上下行网络速率。
- 右键胶囊打开退出菜单。
- CPU、网络和电池信息由跨平台 Rust 库采集。GPU 使用率按系统能力读取：macOS 使用 IOAccelerator，Linux 读取 DRM `gpu_busy_percent`，Windows 优先使用 NVIDIA `nvidia-smi`，并回退到 Windows GPU Engine 性能计数器；系统未提供数据时明确显示不可用。
- Noto Sans SC 与吉祥物表情序列均随程序打包，不依赖系统字体；字体和表情资源各自附带许可证。
- macOS 窗口加入所有桌面空间，并作为全屏辅助窗口显示在全屏应用上方。

## 开发

需要 Rust stable。Linux 构建还需 X11/Wayland 与 OpenGL 开发库。

```sh
cargo test --locked
cargo run
```

在 macOS 上构建并安装到 `~/Applications/灵动岛.app`：

```sh
./scripts/install-macos.sh
```

CI 会在 macOS、Windows 和 Linux 上运行测试并构建 release。
