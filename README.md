# 灵动岛原型

使用 Rust、egui 和 eframe 构建的跨平台桌面悬浮胶囊，面向 macOS、Windows、Linux。界面参考岛仔的深色圆角胶囊与轻量信息栏样式；窗口不吸附屏幕边缘，可以拖放在桌面任意位置。

## 功能

- 采用岛仔同类的透明眼睛、嘴巴表情序列，自动播放眨眼、张望、害羞等动作；点击可切换，胶囊可自由拖动。
- 收起态默认显示表情和电池；鼠标停留约 0.18 秒后弹簧展开，移开后延迟收起。
- 展开后默认进入固定在第一页的通知中心；通知为空时显示空状态，通知较多时可分页查看并逐条清除。
- 性能监控、番茄钟和快递作为普通 Widget 页面提供，按 Registry 顺序切换。
- 显示系统电量和充电状态。
- Linux / Windows 播放音乐时，收起态和展开态歌词都按实际文字宽度单行显示，最大宽度为 4096 个逻辑像素；收起态音符紧贴歌词，音响图标随播放抖动、暂停静止。
- 监听 Chromium 系浏览器、Firefox 和 macOS Safari 的下载；可在设置中启用收起态进度，完成后展开并提供打开文件和显示所在位置的操作。
- 展开后查看 CPU、GPU、上下行网络速率。
- 设置面板直接显示在胶囊内；可配置收缩状态内容、免打扰时段，以及番茄钟专注/休息时长（各 1–180 分钟）。
- 快递 Widget 通过 OpenCLI 后台查询淘宝和京东，默认每 20 分钟刷新一次，可在「设置 → 快递」修改间隔。每个平台在本地保留上一次成功结果；首次查询建立基线，之后出现新快递、状态变化或新物流节点时进入通知中心。快递变更通知和取件通知可分别开启，待取件时优先发送取件提醒。
- 可选启用 Bilibili 私信和收到的赞通知；通过系统 OpenCLI 和现有 Chrome 登录态读取，不要求输入 Cookie。
- 顶部月亮按钮可手动常开免打扰；在计划时段中关闭时，只跳过当前时段，之后仍按计划启用。
- 右键胶囊打开退出菜单。
- CPU、网络和电池信息由跨平台 Rust 库采集。GPU 使用率按系统能力读取：macOS 使用 IOAccelerator，Linux 读取 DRM `gpu_busy_percent`，Windows 优先使用 NVIDIA `nvidia-smi`，并回退到 Windows GPU Engine 性能计数器；系统未提供数据时明确显示不可用。
- Noto Sans SC 与吉祥物表情序列均随程序打包，不依赖系统字体；字体和表情资源各自附带许可证。
- macOS 窗口加入所有桌面空间，并作为全屏辅助窗口显示在全屏应用上方。

下载信息从浏览器本地记录和临时文件读取，不会上传。浏览器锁定数据库时，程序会在系统临时目录创建短暂的只读快照，读取后删除。部分浏览器不会实时保存总大小或隐私浏览记录，此时显示已下载的数据量，完成通知取决于浏览器是否保存了下载记录。监听 Safari 的下载记录需要在 macOS「隐私与安全性 → 完全磁盘访问权限」中允许灵动岛访问。

快递查询沿用 [OpenCLI 快递插件](https://github.com/kevinhuang001/opencli-kuaidi) 的订单范围。请在连接了 OpenCLI Browser Bridge 的 Chrome 中登录淘宝和京东，并安装插件：

```sh
opencli plugin install github:kevinhuang001/opencli-kuaidi
```

如果某个平台查询失败，Widget 会提示并保留该平台上次的结果。平台不再返回的订单会从列表移除；只有查询到明确的签收状态时才提示签收。

天气页显示当前温度、体感温度、日出日落、湿度、风、降水概率和紫外线。逐小时折线图从所选城市的下一整点开始，显示未来五小时的气温和天气图标。未来七天的最高温和最低温曲线也在天气页中。收起态显示天气图标与当前温度。地点、更新间隔和风险提醒可在「设置 → 天气」调整。更新间隔为 1–180 分钟，默认 10 分钟。天气图标使用 MIT 许可的静态 SVG。[Bas Milius 的 Meteocons](https://github.com/basmilius/meteocons)

天气数据由 [Open-Meteo](https://open-meteo.com/) 提供。极端天气提醒使用预测值与阈值比较。阈值设计参考 [schlpbch/open-meteo-mcp-rust 的 `get_weather_alerts`](https://github.com/schlpbch/open-meteo-mcp-rust/blob/f0b84f5ab954de3c6a2a79bc83a9def53c3b43c5/src/tools/alerts.rs)（Apache-2.0）。Peninsula 使用逐小时和逐日预报生成提示，参考项目提供了参数和默认阈值。

展开态固定按钮左侧的仪表盘按钮会打开一个独立窗口，按屏幕空间排列所有 Widget。展开 Widget 的最小宽度为屏幕宽度的 25%，最小高度为屏幕高度的 160/900。较小的内容会在展开区域内居中。

Windows 和 Linux 提供音乐扩展。检测到正在播放的媒体会话时显示音乐 Widget 和 Compact 歌词。暂停后保留播放控制，停止会话或退出播放器后隐藏。Widget 展示歌曲、进度、上一首、播放/暂停、下一首及歌词。带时间轴的歌词会同步到 Compact，纯文本歌词可以在 Widget 内滚动阅读。

- Windows 读取系统媒体会话。Linux 读取支持 MPRIS 的播放器，播放本地文件时优先读取同名 `.lrc`。
- 在线查词时会将歌名和艺人提交给 [LRCLIB](https://lrclib.net/)、网易云音乐或 QQ 音乐。音乐界面与歌词实现参考 [岛仔（Daozai）](https://github.com/Renly1994/xiaotang-daozai)，保留其 MIT 许可。

扩展可通过 `CompactView.visibility` 声明当前是否显示，宿主同时应用用户的 Compact 开关。番茄钟首次开始后才出现，暂停时保留，重置后隐藏。

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

在 Windows 上构建安装包（需要 NSIS，`choco install nsis`）：

```powershell
cargo build --locked --release
makensis /DAPP_VERSION=0.1.0 scripts/installer.nsi
```

`Windows installer` workflow 会在推送到 `main` / `codex/weather-integration` 或打 `v*` 标签时，用 sccache 和 Rust 缓存构建 release，上传 `Peninsula-<版本>-windows-x64-setup.exe` 与便携版 zip。安装包按用户安装到 `%LOCALAPPDATA%\Programs\Peninsula`，不需要管理员权限。

CI 会在 macOS、Windows 和 Linux 上运行测试并构建 release。

## 扩展架构

- `src/extension/` 定义扩展 ID、版本化描述、生命周期 Hook、Widget/Compact 视图数据、通知视图和操作契约；Hook 状态持久化在用户数据目录。
- `src/core/` 管理扩展调度、操作分发、通知待处理状态和提醒展示队列。提醒关闭或超时不会清除通知；确认、扩展操作和逐条/全部清理会将其标记为已处理。
- `src/builtins/` 注册 Download、Performance、Pomodoro、Bilibili 通知及表情和时钟 Compact。性能 Widget 与 Compact 共用一个采样器快照；番茄钟使用 Core Deadline 调度，并保存专注/休息时长。
- `src/core/external/opencli.rs` 负责检测并执行系统 OpenCLI；`integrations/opencli/peninsula-bilibili/` 提供私信与收到的赞只读命令。
- `src/app/` 管理胶囊外壳、设置面板、页面切换和窗口行为。

通知使用 `(source_extension_id, notification_id)` 作为稳定标识。通知扩展分别提供提醒和通知中心条目的视图数据；Core 将这些数据交给统一的渲染器并回传用户操作。Rust 扩展仍在应用进程内运行；OpenCLI 插件只是由系统 OpenCLI 执行的集成资源，不是 Peninsula 的动态插件加载器或公开 SDK。

### 声明式 Widget

Widget 注册后默认为可见。扩展可通过 `RuntimeExtension::widget_visibility` 返回 `Hidden`，宿主会将它从轮播和仪表盘同时移除。运行时状态变化时，扩展返回 Widget ID 让宿主刷新缓存。Compact 由组件自己的 `CompactView::visibility` 和用户的 Compact 显示设置共同决定。

扩展在 `src/ui/declarative.rs` 提供的数据类型上组合视图；展开页、收起态扩展组件和通知界面都由 `src/ui/renderer/` 绘制。扩展可以返回不同的数据和操作 ID，窗口大小、位置、切页和操作路由由宿主决定。视图在数据失效或到达刷新期限时重建；计时器暂停时不做定时重建。

例如，性能监控用 `Node::Grid(Grid::new(Columns::AutoFit { min_item_width: 65.0, max_columns: 4 }), cards)` 排列指标；横向空间不足会减少列数，较高内容可滚动。固定网格可为各列、各行选择不同的轨道类型，并通过 `ViewNode::placement` 声明行列及跨距。树中的尺寸用 egui points；`Percent(0.5)` 表示父容器可用尺寸的一半，`Fill(2.0)` 表示分配剩余空间时的权重。描述符给出默认尺寸，启用 `resizable` 的 Widget 可用运行时视图请求新尺寸。设置 `fit_width` 或 `fit_height` 时会按内容测量对应尺寸，宿主将展开宽高限制在当前屏幕的 75%。图片和精灵引用宿主注册的纹理名，如 `lucide/clock`。

表格可使用 `Node::Table(Table { columns, rows })`。列定义包含标题和相对宽度，表头与各行共享列宽。单元格接受普通视图节点，行高随内容调整。

前景色可继承或覆盖，背景默认透明。语义色随主题变化，RGBA 保持声明的颜色。可以在 Core 的 `preferences.conf` 里设置 `widget_theme=dark`、`light` 或 `system`，以及 `widget_accent=#25579c`。

### 扩展设置

在 `ExtensionDescriptor::with_settings` 声明配置版本、字段类型、默认值和约束。Core 验证配置并原子写入用户配置目录的 `extension-settings.json`；改动后通过 `on_settings_changed` 给扩展传递验证过的快照。需要改变旧值结构时，可在 `migrate_settings` 中按旧版本转换。标记为 `Restart` 的值保存后在下次启动生效。时长以毫秒表示，颜色是四个 sRGBA 通道。

番茄钟时长和 Bilibili 的轮询间隔通过 Schema 保存。旧版 `preferences.conf` 中的 OpenCLI 路径会迁移至扩展配置，默认自动检测。性能监控的 `show_download`、`show_upload` 和下载胶囊的 `show_rate` 是额外的布尔配置，未新增设置页面。启用状态、顺序和免打扰仍由 Core 管理；登录信息沿用现有认证方式。

## Linux 音乐显示演示

[观看 Linux release 实机录屏](docs/media/music-lyrics-linux.mp4)。视频使用本地 MPRIS 测试播放器和原创 LRC 歌词，展示长短歌词切换、收起 / 展开和播放 / 暂停；不播放音频。

可在同一个桌面 D-Bus 会话中复现（需要 Python 3 和 `dbus-next`）：

```sh
python3 -m venv .venv-demo
.venv-demo/bin/pip install dbus-next
.venv-demo/bin/python scripts/demo-music.py &
cargo run --locked --release
```

演示会在第 25 秒暂停音响动画，第 29 秒恢复；也可以通过音乐页面的播放 / 暂停按钮控制。歌词文件为 `scripts/fixtures/music-demo.lrc`。
