# 灵动岛原型

使用 Rust、egui 和 eframe 构建的跨平台桌面悬浮胶囊，面向 macOS、Windows、Linux。界面参考岛仔的深色圆角胶囊与轻量信息栏样式；窗口不吸附屏幕边缘，可以拖放在桌面任意位置。

## 功能

- 采用岛仔同类的透明眼睛、嘴巴表情序列，自动播放眨眼、张望、害羞等动作；点击可切换，胶囊可自由拖动。
- 收起态默认显示表情和电池；鼠标停留约 0.18 秒后弹簧展开，移开后延迟收起。
- 展开后默认进入固定在第一页的通知中心；通知为空时显示空状态，通知较多时可分页查看并逐条清除。
- 性能监控和番茄钟作为普通 Widget 页面提供，按 Registry 顺序切换。
- 显示系统电量和充电状态。
- 监听 Chromium 系浏览器、Firefox 和 macOS Safari 的下载；可在设置中启用收起态进度，完成后展开并提供打开文件和显示所在位置的操作。
- 展开后查看 CPU、GPU、上下行网络速率。
- 设置面板直接显示在胶囊内，左右两栏并留有更宽松的间距；可配置收缩状态内容，并按星期和起止时间设置免打扰时段。
- 顶部月亮按钮可手动常开免打扰；在计划时段中关闭时，只跳过当前时段，之后仍按计划启用。
- 右键胶囊打开退出菜单。
- CPU、网络和电池信息由跨平台 Rust 库采集。GPU 使用率按系统能力读取：macOS 使用 IOAccelerator，Linux 读取 DRM `gpu_busy_percent`，Windows 优先使用 NVIDIA `nvidia-smi`，并回退到 Windows GPU Engine 性能计数器；系统未提供数据时明确显示不可用。
- Noto Sans SC 与吉祥物表情序列均随程序打包，不依赖系统字体；字体和表情资源各自附带许可证。
- macOS 窗口加入所有桌面空间，并作为全屏辅助窗口显示在全屏应用上方。

下载信息从浏览器本地记录和临时文件读取，不会上传。浏览器锁定数据库时，程序会在系统临时目录创建短暂的只读快照，读取后删除。部分浏览器不会实时保存总大小或隐私浏览记录，此时显示已下载的数据量，完成通知取决于浏览器是否保存了下载记录。监听 Safari 的下载记录需要在 macOS「隐私与安全性 → 完全磁盘访问权限」中允许灵动岛访问。

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

## 扩展架构

- `src/extension/` 定义扩展 ID、描述信息、Widget/Compact 视图数据、通知视图和操作契约。
- `src/core/` 管理扩展调度、操作分发、通知待处理状态和提醒展示队列。提醒关闭或超时不会清除通知；确认、扩展操作和逐条/全部清理会将其标记为已处理。
- `src/builtins/` 注册 Download、Performance、Pomodoro 及表情和时钟 Compact。性能 Widget 与 Compact 共用一个采样器快照；番茄钟使用 Core Deadline 调度。
- `src/app/` 只负责 egui 绘制、胶囊交互、页面切换和窗口行为。

通知使用 `(source_extension_id, notification_id)` 作为稳定标识。通知扩展分别提供提醒和通知中心条目的视图数据；App 绘制这些数据并回传用户操作。当前架构只支持进程内置扩展，本阶段不包含外部插件加载器或公开 SDK。
