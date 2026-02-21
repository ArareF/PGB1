# PGB1 迁移到新电脑 — AI 检查清单

> **给新电脑上的 AI**：这份文档是从旧电脑迁移过来的。请按顺序完成以下检查，确保开发环境就绪后再开始工作。

---

## 项目概况

PGB1 是 2D 游戏特效师文件整理工具，Tauri 2.x + Vue 3 + Rust，目标平台 Windows。功能已全部完成，目前阶段是**收尾完善**。

**项目文件夹已复制过来，但 `node_modules/` 和 `src-tauri/target/` 不会随项目复制（太大了），需要在新电脑上重新生成。**

---

## 第一步：检查必装软件

请逐一运行以下命令，确认版本符合要求：

```bash
node --version       # 需要 v18+（当前旧机：v24.11.0）
npm --version        # 需要 v9+（当前旧机：v11.6.1）
rustc --version      # 需要 1.70+（当前旧机：1.91.1）
cargo --version      # 随 Rust 一起装
rustup target list --installed  # 必须包含 x86_64-pc-windows-msvc
```

**如果缺少任何工具：**

| 工具 | 安装方式 |
|------|---------|
| Node.js | 官网下载 LTS 版 |
| Rust + Cargo | `winget install Rustlang.Rustup` 或官网 rustup.rs |
| MSVC target | `rustup target add x86_64-pc-windows-msvc` |
| Visual Studio Build Tools | 需要 MSVC 工具链，安装 VS Build Tools 并勾选"C++ 生成工具" |
| WebView2 | Windows 11 已内置，Windows 10 需手动安装 |

---

## 第二步：安装依赖

```bash
# 在项目根目录（PGB1/）执行
npm install

# Rust 依赖会在首次编译时自动下载，无需手动操作
```

**预期结果：** `node_modules/` 目录创建成功，无报错。

---

## 第三步：验证能否启动

```bash
npm run tauri dev
```

**预期结果：** 应用窗口弹出，显示 PGB1 主界面（毛玻璃风格，左侧快捷方式栏，中间内容区）。

**首次编译 Rust 会很慢（5~15分钟），正常。**

**常见错误排查：**

| 错误信息 | 原因 | 解决 |
|---------|------|------|
| `LINK : fatal error LNK1181` | 缺少 MSVC 工具链 | 安装 Visual Studio Build Tools |
| `error[E0463]: can't find crate` | Rust 依赖下载失败 | 检查网络，或配置 crates.io 镜像 |
| `WebView2 not found` | 缺少 WebView2 | 安装 Microsoft Edge WebView2 Runtime |
| `windows-rs` 相关编译错误 | Windows SDK 版本不对 | 安装/更新 Windows SDK |

---

## 第四步：读取项目文档（AI 必做）

项目有完整的设计文档体系，**每次新会话必须先读以下文件**：

```
INDEX.md         — 全局索引，所有设计文档的入口
CODE_INDEX.md    — 代码索引，50+ 个源文件的职责说明
CLAUDE.md        — 角色定义和协作规范（AI 行为准则）
```

然后按需查阅 `design/` 目录下的具体设计文档。

---

## 关于 Memory（跨会话记忆）

**Memory 文件不随项目复制，需要手动迁移或重建。**

Memory 文件在旧电脑的路径：
```
C:\Users\{用户名}\.claude\projects\D--work-pgsoft-PGB1\memory\
├── MEMORY.md         ← 核心，开局必读清单和关键决策
└── collaboration.md  ← 和产品总监的协作偏好笔记
```

**方案 A（推荐）：手动复制**
把旧电脑上 `C:\Users\{用户名}\.claude\projects\D--work-pgsoft-PGB1\memory\` 整个文件夹复制到新电脑同路径。

**方案 B（如果无法复制）：AI 从项目文档重建**
MEMORY.md 的核心内容在 `INDEX.md` 和 `CODE_INDEX.md` 里都有记录，AI 读完这两个文件后可以重建。告诉 AI：「请读取 INDEX.md 和 CODE_INDEX.md，然后重建 memory/MEMORY.md」。

---

## 项目当前状态（2026-02-20）

- **阶段**：全功能完成，待收尾完善
- **已完成**：Phase 1~7（全部核心功能）+ 多分辨率响应式缩放
- **待做事项**：收尾完善（具体内容由产品总监决定）

**主要功能一览：**
- 项目管理（扫描/新建/重命名/改截止日期/删除到回收站）
- 任务管理（启用/归档/时光机）
- 素材工作流（规范化→缩放→格式转换→上传nextcloud）
- 日报打卡（WebView 自动化）
- 翻译功能（全局快捷键，Gemini API）
- 多分辨率缩放（useScale.ts，基准1920px，自动跟随窗口）

---

## 测试数据路径

| 路径 | 内容 |
|------|------|
| `D:\work\pgsoft\exp\` | 项目根目录（开发测试用） |
| `D:\work\pgsoft\exp\217_RedDevil\` | 真实项目，完整结构 |

**注意：测试数据不在项目文件夹内，需要在新电脑上单独准备或创建新的测试目录。**

---

## 完成标志

- [ ] `node --version` 输出 v18+
- [ ] `rustc --version` 输出 1.70+
- [ ] `npm install` 无报错
- [ ] `npm run tauri dev` 能启动，窗口正常显示
- [ ] 读完 INDEX.md、CODE_INDEX.md、CLAUDE.md

全部打勾后，告诉产品总监「新电脑环境就绪，可以继续工作」。
