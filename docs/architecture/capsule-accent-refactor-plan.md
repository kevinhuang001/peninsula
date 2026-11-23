# Capsule 命名与强调色体系重构计划

> 状态：本方案已进入代码实现，具体通过情况以本 PR 的 GitHub CI 和 Review 为准。
>
> 基线：`main` @ `c73180cf8b63d79574ca41d36ae1575cbf01f818`（2026-11-23 检查）
>
> 原则：统一命名、单一主题真相源、规约与业务颜色解耦、Core 负责主题解析、扩展只描述声明式 UI。不引入面向旧名称的长期兼容层。

## 1. 目标与边界

1. 将产品概念 **Compact 全面改称 Capsule**：Rust 类型、Trait 方法、enum 分支、模块与文件、变量、测试、活动文档、演示脚本、持久化字段与序列化契约都纳入核查。`Capsule` 指收起时的胶囊内容/贡献项；已存在的外壳 `app::capsule` 保留。
2. 中文界面规范：`Widget` 显示为 **组件**；`Capsule` 显示为 **胶囊**。英语界面/代码继续使用 `Widget` / `Capsule`，不可把 Widget 的 Rust API 改成 Component。
3. 将强调色从“一个 RGB 基色 + 各处手写浅深计算/固定蓝色”改为 **统一、多色阶、深浅主题适配、可校验对比度**的主题 token 系统。预设颜色只是输入基色，组件不能匹配具体色名来决定 UI。
4. 声明式 Widget/Capsule/Prompt 共用颜色规约：扩展以语义色或色阶 token 声明外观；Core 统一解析颜色。官方内置 Widget 的装饰/交互默认跟随强调色，保留成功、警告、危险等真正的语义色。
5. 仅对明确需要保留特定品牌颜色、数据分类辨识度或用户自选颜色的地方使用显式 RGB，不将绿色=成功、红色=危险等业务语义强行替换为强调色。

## 2. main 现状与具体问题

| 位置 | 当前状态 | 问题 / 方向 |
| --- | --- | --- |
| `src/app/settings/model.rs` | 一级导航已经是“胶囊”“组件” | 保留；补齐子菜单、空态和各内页文本 |
| `src/app/settings/view.rs` | 子菜单仍有“Compact 样式/开关/顺序”“Widget 开关/顺序”；强调色是六项预设下拉 | 全部改成中文术语；下拉仍选基色，可显示色阶预览 |
| `src/extension/descriptor.rs` | `ContributionKind::Compact` | 改为 `ContributionKind::Capsule`，审视 serde 持久化影响 |
| `src/extension/view.rs` | `CompactView`、`CompactVisibility`、`compact_views`、`compact_refresh_interval` | 同步更名，并更新所有内置扩展 |
| `src/ui/declarative.rs`、`src/ui/declarative/compact.rs` | `Compact`、`CompactLayout`、`Node::Compact`、`ViewNode::compact` | 改为 Capsule 对应名字；保留现有布局能力 |
| `src/ui/renderer/mod.rs`、`src/ui/renderer/compact.rs` | `CompactTextAppearance`、`show_compact*`、`compact_content_width` 等 | 同步更名；避免命名停留在旧概念 |
| `src/app/capsule.rs`、`src/app/capsule/compact.rs`、`src/app/compact_layout.rs` | 已有 Capsule 外壳，但 collapsed 内容仍叫 compact | 将内部模块明确命名为胶囊条目/收起态，避免两个“Capsule”含义混淆 |
| `src/app/settings/preferences.rs` | 总体偏好类型仍叫 `CompactPreferences`；`widget_accent`、`compact_text_*`；胶囊条目以 `extension=...` 写盘 | 总体类型名、偏好键与持久化版本统一梳理 |
| `src/app/settings/theme.rs`、`src/ui/palette.rs` | 六种强调色；`SETTINGS_ACCENT_TEXT` 是固定浅蓝；按钮填充由单色调亮/压暗 | 将基色预设和各层级 token 分开，集中生成 |
| `src/ui/renderer/mod.rs` | `Theme` 从 `visuals.selection.bg_fill` 推断强调色；`AccentSubtle` 用 `gamma_multiply`；`OnAccent` 直接读取 selection 描边色 | 改为显式 ThemeTokens，不把 egui 控件状态当作主题数据库 |
| `src/ui/declarative.rs` | **已经有** `ColorSpec::Accent`、`OnAccent`、`AccentSubtle` | 扩展这套现有规约，而不是新增平行的“Widget 强调色开关” |
| `src/ui/declarative.rs` | `ViewNode::subtle_button` 的普通文字使用 `Info`，背景使用 `AccentSubtle` | 颜色语义不一致；改为合适的 accent 前景 token |
| `src/app/datasource/mod.rs` 等原生 egui UI | 通过 hyperlink_color 获取强调色，局部仍有固定蓝色 | 统一使用 ThemeTokens；不让业务 UI 自己推断色值 |
| `src/builtins/*` | 有的 UI 已用 Accent，有的装饰图标/按钮使用 Info、Cyan、Violet、硬编码 RGB | 逐个审计，仅将装饰性色改用强调色 |

现有 `TextPalette` 保存文本/状态色，`WidgetTheme::install` 写 egui Visuals；规划时应保留它们可用的功能，但把“色阶生成”和“语义 token 解析”集中到一个明确的主题模块。设置面板目前仍有固定深色中性色：本次先消除**强调色**耦合，不额外扩展为全应用亮/暗界面重做。

## 3. 阶段 A：Compact → Capsule 的完整重命名

### A1. 公共类型与规约（破坏性更名）

| 旧名称 | 新名称 |
| --- | --- |
| `ContributionKind::Compact` | `ContributionKind::Capsule` |
| `CompactView` / `CompactVisibility` | `CapsuleView` / `CapsuleVisibility` |
| `Compact` / `CompactLayout` | `CapsuleContent` / `CapsuleLayout`（避免和应用外壳 Capsule 概念混淆） |
| `Node::Compact` | `Node::Capsule` |
| `ViewNode::compact(...)` | `ViewNode::capsule(...)` |
| `RuntimeExtension::compact_views` | `RuntimeExtension::capsule_views` |
| `RuntimeExtension::compact_refresh_interval` | `RuntimeExtension::capsule_refresh_interval` |
| `CompactItem` / `CompactItemPreference` | `CapsuleItem` / `CapsuleItemPreference` |
| `CompactTextAppearance` | `CapsuleTextAppearance` |
| `CompactPreferences` | `AppPreferences`（目前它装载的是全应用偏好，不只是胶囊） |
| `compact_items` / `compact_text_size` / `compact_text_color` | `capsule_items` / `capsule_text_size` / `capsule_text_color` |
| `show_compact*` / `compact_content_width` | `show_capsule*` / `capsule_content_width` |

建议将 `src/ui/declarative/compact.rs` → `capsule.rs`、`src/ui/renderer/compact.rs` → `capsule.rs`、`src/app/compact_layout.rs` → `src/app/capsule/layout.rs`、`src/app/capsule/compact.rs` → `src/app/capsule/collapsed.rs`。按 Rust 模块依赖调整 `mod` / `pub use`，并重新评估是否需要从 `app::capsule` 对外转发布局逻辑，避免循环引用。

同步扫描 `src/core`、`src/sdk`、`src/builtins`、`src/app`、`src/ui` 的结构体字段、事件/状态、注册查询、字体设置、宽度测量、Prompt 交接与生命周期。`Prompt` 是独立贡献类型，**不得**因改名与 Capsule contribution 合并。

### A2. 用户可见文案与资源

- 一级导航已正确：胶囊 / 组件；二级改为“胶囊样式 / 胶囊开关 / 胶囊顺序”“组件开关 / 组件顺序”。
- 文案“Compact 文字样式”“没有已开启的 Compact”“Widget 开关/顺序”“没有已开启的 Widget”及 tooltip、帮助、Doctor、测试断言统一。
- `docs/architecture/*`、README、现行示例、`scripts/compact-demo.*`、`assets/icons/compact/*` 跟随新名字；必要时调整代码引用。历史录屏或图片仅作为历史媒体存在时可保留原始二进制文件，但活动文档不再称功能为 Compact。
- 第三方公开规约的字段、示例、序列化名称一并更新；不给新扩展暴露旧 `Compact` 别名。

### A3. 持久化与版本变更

重点：旧代码中的 `extension=<id>|<enabled>|<order>` 实际存的是胶囊顺序；`widget_accent` 实际是全局强调色。不能只替换变量名后直接遗失用户设置。

- 新配置键建议：`capsule_item`、`capsule_text_size`、`capsule_text_color`、`appearance_theme`、`accent_color`；组件顺序沿用语义准确的 `widget_order`。
- 旧键（`extension`、`compact_text_*`、`widget_theme`、`widget_accent`）只在**一次性文件迁移**中读取并写成新格式；之后正常运行只认新格式，不保留双轨写入或公开 API 兼容壳。
- `ContributionKind` 的 serde 值由 `Compact` 改为 `Capsule` 后，需处理持久化 `contribution-catalog.json` 的历史记录：为 catalog 引入明确版本并执行一次转写，覆盖静态/动态注册；已保存的启用状态、顺序与业务设置必须保留。
- 不随意修改已经部署的 `ExtensionId` 字符串；若发现稳定 ID 含 compact，先列清映射与引用它的配置/通知/历史数据，再决定是否单次重写。常量名可以重命名，但稳定 ID 变更须显式迁移。
- 迁移应具备：幂等、原子写入、失败时不覆盖原文件、未知字段不会静默触发数据丢失、重复启动不重复迁移。迁移完成后不留下常驻的 legacy 分支。

## 4. 阶段 B：单一的多阶强调色体系

### B1. 数据模型

新增统一 `AccentScale` + `ThemeTokens`（建议放 `src/ui/palette.rs` 或拆 `src/ui/theme/`），输入仅是 `accent_color: [u8; 3]` 与当前明暗模式，输出至少包括：

- **原始多阶色板**：`accent_50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950`。其梯度由**同一基色**生成，不为蓝/紫/青/绿等预设分别手写样式判断。
- **语义引用**：`accent`（主要交互）、`on_accent`（强调色背景上的文字/图标）、`accent_hover`、`accent_pressed`、`accent_subtle`、`on_accent_subtle`、`accent_border`、`accent_focus`，并保留既有表面/文本/成功/警告/危险等独立 token。
- **主题切换**：根据深色、浅色和用户选择一次性重建 token；注册的所有 ViewNode 和原生控件统一重绘，不依赖扩展手动重启/更新其颜色缓存。

推荐用感知均匀色彩空间（如 OKLCH）对亮度插值、适度调节 chroma，并做 sRGB 色域映射。色阶需单调且可复现；若引入颜色库，先评估 Rust/wasm/平台依赖与体积。禁止用 `Color32::gamma_multiply` 直接把单一颜色当作整个浅色阶。

`ThemeTokens` 是单一真相源：egui `Visuals` 是由 token **安装出来的输出**，声明式 Renderer 也从 token 读取；**不能**再从 `hyperlink_color` 或 `selection.bg_fill` 反向猜测当前强调色。根据上下文/viewport 管理一致的快照，避免设置窗口与组件视图读到不同主题。

### B2. 颜色规约升级（复用现有 ColorSpec）

保留已有 `ColorSpec::Accent`、`OnAccent`、`AccentSubtle`；新增带有严格枚举的色阶，例如：

~~~rust
pub enum AccentShade { S50, S100, S200, S300, S400, S500, S600, S700, S800, S900, S950 }

pub enum ColorSpec {
    // 其余现有颜色种类不变
    Accent,
    OnAccent,
    AccentSubtle,
    OnAccentSubtle,
    AccentShade(AccentShade),
    // ...
}
~~~

- `Paint.foreground / background / border_color`、`IconStyle.color`、`SvgStyle.fill / stroke`、`RingStyle.track`、文本及图表相关样式继续使用相同 `ColorSpec`；不针对 Widget 单独创造无法复用的色值接口。
- 扩展只声明颜色 token，不能在规约中传入 egui `Color32` 或要求 Core 对“蓝色主题”“绿色主题”分支判断；保留 `Rgba` 作为显式且可审计的例外。
- `AccentShade` 是严格的枚举（不接受任意整数或动态计算的色阶索引），serde round-trip 稳定，未知 token 明确报 schema 错误；同时约定“应该使用语义 token 的场合优先使用语义 token，而不是硬选 500/700”。
- 对开发者说明示例：高亮主按钮用 `Accent` + `OnAccent`；浅色卡片用 `AccentSubtle` + `OnAccentSubtle`；细微强调分割线才用 `AccentShade(S200)`。
- 不再为“允许 widget 用强调色”额外增加权限、进程依赖或 settings 项：声明式颜色本来是无副作用的纯展示能力。

### B3. 对比度、交互状态和半透明

- 基于 **线性 sRGB** 正确计算相对亮度/对比度，必须先将带 alpha 的前景合成到实际背景上再校验。一般文字目标 WCAG AA ≥ 4.5:1，大号文字/图标/控件边界目标 ≥ 3:1；可保留现有更严格的文字对比度策略。
- 根据交互背景自动选择 on-accent 前景，不能把白色硬编码为所有强调色的标签色；极浅、极深、灰度及高饱和基色均需测试。
- `hover/pressed/selected/disabled/focus` 统一使用语义 token，并确保相邻状态有可辨别差异；禁用态以不误导为原则，不强行要求达到正常态同一对比度。
- `ColorSpec::AccentSubtle` 和 `OnAccentSubtle` 必须在深/浅两种主题下可读。`ViewNode::subtle_button` 从目前 Info 文字 + AccentSubtle 背景的混搭改为同套语义色。
- 维持 `TextPalette::Success / Warning / Danger` 等**状态**颜色独立；应用自定义色不应改变“下跌/失败/紧急”语义的表达。

## 5. 阶段 C：Core 与官方扩展落地

### C1. Core

1. `WidgetTheme::install`、`src/ui/renderer/mod.rs::Theme`、`src/app/settings/theme.rs` 统一读取 `ThemeTokens`；单份逻辑处理交互色、选择、超链接、选中描边、边框与对比度。清理 `SETTINGS_ACCENT_TEXT`、`ACCENT_SELECTED` 及其他**颜色特定**常量；必要的中性色可保留为 neutral token。
2. 设置窗口 `general_accent` 保持用户选取一个基色（当前六个预设），可以在选项旁展示所选主题的几级渐变样本；**不**新增“选深浅级别”等无必要设置，色阶由组件/主题自动选择。
3. `src/app/datasource/mod.rs` 等 Core 自绘面板改从共享 token 取色，而非拿 `hyperlink_color` 伪装 accent 或固定蓝 RGB。
4. `Prompt.accent: Option<[u8;3]>` 的业务指定色继续允许；未指定时跟随全局 accent。Prompt 的宿主环境光、轮廓和文本对比同样应使用公共的色彩辅助函数，不单独维护一套亮暗算法。
5. 仅在 `theme/accent` 设置变化时重建计算后的调色板并触发当前 viewport 重绘，避免每个节点/每帧重复转换 OKLCH；主题切换后不得保留陈旧 SVG/绘制缓存颜色。

### C2. 官方 Widget / Capsule

逐项检查 `src/builtins/{calendar,device,health,weather,market,music,mail,rss,kuaidi,bilibili,download,performance,pomodoro}` 和数据源呈现：

- 主按钮、切换器、选中日期、当前页标记、进度条、卡片强调边框、焦点和一般装饰性图标：优先使用语义 Accent 系列。
- 已有的 `ColorSpec::Info` / `Cyan` / `Violet` 若只充当某官方组件的“默认品牌色”，换成 Accent 或合适 AccentShade；若明确表示信息类别或多条序列区分，保留该类别色或统一建立语义分类 token。
- 市场涨跌、系统成功/危险/低电量、天气预警等保持对应的 Success/Warning/Danger 语义，不因用户选择粉色或蓝色而改变警示信息。
- Calendar 当前已有 Accent/AccentSubtle，用作首个端到端回归基线；其它官方组件跟随完善。Capsule 文字颜色“跟随主题”是对比度安全的默认值，显式用户自定义文字色继续生效。
- 自绘 egui 控件与声明式 ViewNode **外观一致**，不允许内置模块通过 `match accent_rgb` 为某个预设单独写颜色。

### C3. 建议的调用关系

~~~text
用户设置：appearance_theme + accent_color
                   |
                   v
          Core Theme Builder
       (色阶、语义映射、对比度)
                   |
              ThemeTokens
               /       \
              v         v
         egui Visuals   ColorSpec Resolver
              |         |
              v         v
        原生设置 UI     Widget / Capsule / Prompt
~~~

扩展规约仅引用 token；Settings 保管用户的基色与主题选择；Core 生成颜色，Renderer 消费颜色，内置扩展声明颜色，避免四处各做一份。

## 6. 实施步骤与提交划分

当前代码分支已实现核心更名、色阶生成、主题规约、官方组件用色及旧偏好迁移；以下作为最终合并前的审查清单。

- [ ] **提交 1：命名契约**。更改公共类型、规约、模块、内置实现和编译期测试；不引入旧类型 alias。全仓检索大小写和下划线形式的 compact。
- [ ] **提交 2：持久化迁移与中文文案**。改总偏好命名、配置键、贡献目录 serde、数据恢复、UI 文案、资源路径、脚本和活动文档。验证旧数据可单次迁移；不留长期兼容分支。
- [ ] **提交 3：色阶与主题 token**。实现色阶生成、语义映射、明暗模式、对比度与缓存；统一 egui 和声明式 Renderer 的颜色读取。
- [ ] **提交 4：规约与默认控件**。新增 `AccentShade` / `OnAccentSubtle`，更新示例/规约、`subtle_button` 与官方 UI，排除装饰色硬编码。
- [ ] **提交 5：回归与精简**。跨平台 CI、配色和旧偏好回归、文档，删除遗留常量、冗余路径与过渡函数。可按审查压力拆成两个实施 PR，但最终合入应是同一套契约。

## 7. 验收检查表

**命名与设置**
- [ ] 除说明迁移的历史记录/测试输入外，`src`、活动文档、示例和 active 脚本中无旧 Compact API、类型、可见文案或资源路径。`Widget` 在中文设置内均显示“组件”，`Capsule` 显示“胶囊”。
- [ ] 展开/收起、胶囊顺序/开关、可变宽排版、Prompt FIFO、组件分页/排序和停启行为不变。
- [ ] 原 `Compact` 类型序列化记录与旧偏好迁移后保持启用状态、排序、字号、颜色及动态注册；迁移幂等，不触发重复配置条目。

**颜色与规约**
- [ ] 六种预设 × 深色/浅色/跟随系统（至少模拟两种系统模式）均生成完整、单调、可复现的 11 级色阶，极端 RGB 也能安全映射。
- [ ] 关键语义颜色在真实合成背景下满足规定对比度阈值；对正常/悬停/按下/禁用/选中/焦点做截图或 golden 测试。
- [ ] Widget/Capsule/Prompt 的 `Accent`、`OnAccent`、`AccentSubtle`、`OnAccentSubtle`、`AccentShade` 在 egui 原生控件和声明式控件中一致；serde 往返稳定。
- [ ] 更换强调色、切换深浅模式后官方组件立即换色，不需要重启扩展；状态警示色不变，内置 widget 不按具体预设色名分支。
- [ ] 验证 `calendar`、`market`、`weather`、`health`、`datasource`、通知与 Prompt 的样式与可读性；保留用户指定的自定义 capsule 字色。

**代码质量**
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --locked` 通过；现有 GitHub CI 在 macOS、Windows、Ubuntu 矩阵通过。
- [ ] 增加单测：色阶单调/明暗映射/对比度、serde enum、设置迁移、稳定 ID、官方声明式颜色；集成/渲染测试：主题切换、用户设置、状态与 Prompt 绘制。
- [ ] review 搜索：不得再用固定蓝色常量驱动全局 accent；不得通过 `visuals.hyperlink_color` 或 `visuals.selection.bg_fill` 反推主题输入；不得在扩展规约硬编码特定颜色名称。

## 8. 不做的事及风险

- 不新增扩展管理设置项、Theme 权限、业务脚本执行能力，也不修改通知开关/调度机制。
- 不重设计所有中性色、系统图标风格或 Core 设置布局；只统一强调色路径和官方 UI 的默认使用。
- 不把警示色、品牌图标和多曲线图表全部染成一种强调色。
- 命名变更影响公开 Rust API 与 JSON serde；必须先提交迁移测试，再更新调用点，避免半改半旧。
- 动态贡献 catalog 可能在扩展尚未启动时被读取，迁移应发生在载入/校验之前；对可恢复数据保留备份或原子恢复路径。
- `egui::Visuals` 仅是渲染配置，不承诺它能完整代表所有 token。设定明确的 ThemeTokens 来源，防止同一界面出现两套强调色。

**完成定义**：只有命名端到端一致、用户偏好不丢、所有官方 UI 随主题切换且跨平台测试通过，才将实施 PR 标为 Ready。