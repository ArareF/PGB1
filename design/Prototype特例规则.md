# Prototype 功能分类特例

**最后更新**: 2026-02-13

---

## 📌 什么是Prototype？

**定义**：游戏刚开始制作时，接到游戏的第一批设计，导出的一份**简易的、可供游戏运行的素材**。

**目的**：
- 快速提供可运行的游戏原型
- 让游戏程序员可以开始集成和测试
- 在正式素材制作完成前作为占位符

---

## 🌟 特殊性：与其他功能分类不同

### 普通功能分类的结构
```
Export/Ambient/
├── 00_original/
│   ├── file1.jpg
│   ├── file2.jpg
│   └── sequence1/
│       ├── sequence1_01.png
│       └── ...
├── 01_scale/
├── 02_done/
└── 03_preview/
```

### Prototype的特殊结构
```
Export/Prototype/
├── 00_original/
│   ├── big_win/           ← 额外的一层子分类
│   ├── infoboard/
│   ├── loading_bonus/
│   ├── main_ui/
│   ├── spinbutton/
│   ├── symbol/
│   └── total_win/
├── 01_scale/
│   ├── big_win/           ← 同样的子分类
│   ├── infoboard/
│   ├── ...
│   └── total_win/
├── 02_done/
│   ├── big_win/           ← 同样的子分类
│   ├── infoboard/
│   ├── ...
│   └── total_win/
└── 03_preview/
```

---

## 📂 固定的子分类（7个）

Prototype下的所有工作流目录（00/01/02）都包含这7个**固定的子分类**：

1. **big_win** - 大赢画面
2. **infoboard** - 信息板
3. **loading_bonus** - 加载/奖励画面
4. **main_ui** - 主界面UI
5. **spinbutton** - 旋转按钮
6. **symbol** - 游戏符号（核心素材）
7. **total_win** - 总赢画面

**重要**：
- ✅ 这7个子分类是**固定的**，每个Prototype项目都一样
- ✅ 程序需要在创建Prototype时自动创建这7个子目录
- ❌ 不允许用户自定义或修改这些子分类名称

---

## 🔄 工作流程（与普通分类相同，只是多了一层）

### 静帧工作流
```
00_original/symbol/
├── h1_dice.jpg
└── h1_dice_blur.jpg
  ↓
01_scale/[100]/symbol/
├── h1_dice.png
└── h1_dice_blur.png
  ↓
02_done/[img-100]/symbol/
├── h1_dice.webp
└── h1_dice_blur.webp
```

**注意**：路径中多了 `symbol/` 这一层

### 序列帧工作流
```
00_original/main_ui/
└── animation_seq/
    ├── animation_seq_01.png
    └── ...
  ↓ (TexturePacker)
02_done/[an-50-18]/main_ui/
├── animation_seq.plist
├── animation_seq.tps
└── animation_seq-0.webp
```

---

## 🌐 nextcloud的特殊结构

### 普通功能分类的nextcloud（扁平化）
```
nextcloud/Ambient/
├── file1.webp
├── file2.webp
├── sequence1.plist
└── sequence1-0.webp
（所有文件在同一层级）
```

### Prototype的nextcloud（保留子分类 + _original副本）
```
nextcloud/Prototype/
├── big_win/
│   ├── _original/              ← 额外的_original目录
│   │   ├── bw_bg.png           ← 从01_scale复制的原始文件
│   │   ├── bw_text.png
│   │   └── ...
│   ├── bw_bg.webp              ← 处理后的文件（从02_done复制）
│   ├── bw_text.webp
│   └── ...
├── infoboard/
│   ├── _original/
│   │   └── ...
│   ├── info_a.webp
│   └── ...
├── loading_bonus/
│   ├── _original/
│   └── ...
├── main_ui/
│   ├── _original/
│   └── ...
├── spinbutton/
│   ├── _original/
│   └── ...
├── symbol/
│   ├── _original/
│   │   ├── h1_dice.png         ← 原始PNG文件
│   │   ├── h1_dice_blur.png
│   │   └── ...
│   ├── h1_dice.webp            ← 处理后的webp文件
│   ├── h1_dice_blur.webp
│   └── ...
└── total_win/
    ├── _original/
    └── ...
```

---

## ⚙️ nextcloud复制规则（Prototype特例）

### 普通功能分类
```
从 02_done/[img-XX]/ 或 [an-XX-YY]/
  ↓
复制到 nextcloud/{功能分类}/（扁平化）
```

### Prototype
```
从 02_done/[img-XX]/{子分类}/ 或 [an-XX-YY]/{子分类}/
  ↓
复制到 nextcloud/Prototype/{子分类}/

同时：
从 01_scale/[XX]/{子分类}/
  ↓
复制到 nextcloud/Prototype/{子分类}/_original/
```

**关键差异**：
1. **保留子分类结构**（不扁平化）
2. **额外复制_original**：从 `01_scale/` 复制原始文件到 `nextcloud/Prototype/{子分类}/_original/`

---

## 📊 复制内容详解

### 处理后的文件（从02_done）
- **来源**：`02_done/[img-XX]/{子分类}/` 或 `02_done/[an-XX-YY]/{子分类}/`
- **目标**：`nextcloud/Prototype/{子分类}/`
- **包含**：
  - 静帧：`.webp` 等图片文件
  - 序列帧：`.plist` + `-0.webp`（精灵图）
- **排除**：`.tps` 文件（与普通分类相同）

### 原始文件（从01_scale）→ _original
- **来源**：`01_scale/[XX]/{子分类}/`
- **目标**：`nextcloud/Prototype/{子分类}/_original/`
- **包含**：
  - 所有从 `01_scale/` 中缩放后的文件（通常是 `.png` 或 `.jpg`）
  - 保持原始格式（不转换为webp）
- **目的**：
  - 提供原始的PNG/JPG文件给游戏开发者
  - 在需要时可以快速访问未压缩的版本

---

## 🎯 PGB1程序需要实现的特殊逻辑

### 1. 创建Prototype项目时
- **自动创建7个固定子目录**：
  ```
  Export/Prototype/
    ├── 00_original/
    │   ├── big_win/
    │   ├── infoboard/
    │   ├── loading_bonus/
    │   ├── main_ui/
    │   ├── spinbutton/
    │   ├── symbol/
    │   └── total_win/
    ├── 01_scale/（同样的7个子目录）
    ├── 02_done/（同样的7个子目录）
    └── 03_preview/
  ```

### 2. 规范化操作
- **路径调整**：在子分类目录内执行规范化
- **示例**：
  - 扫描 `00_original/symbol/`
  - 识别序列帧和静帧
  - 在 `00_original/symbol/` 内整理

### 3. 缩放处理
- **路径调整**：从 `00_original/{子分类}/` 缩放到 `01_scale/[XX]/{子分类}/`
- **示例**：
  - 来源：`00_original/symbol/h1_dice.jpg`
  - 目标：`01_scale/[100]/symbol/h1_dice.png`

### 4. 整理02_done
- **路径调整**：整理到 `02_done/[img-XX]/{子分类}/` 或 `02_done/[an-XX-YY]/{子分类}/`
- **示例**：
  - 来源：散乱在 `02_done/`
  - 识别所属子分类（从文件名或用户指定）
  - 目标：`02_done/[img-100]/symbol/h1_dice.webp`

### 5. 标记已上传并同步到nextcloud（特殊逻辑）
**两步复制**：

**步骤1：复制处理后的文件**
```
从：02_done/[img-XX]/{子分类}/ 或 [an-XX-YY]/{子分类}/
到：nextcloud/Prototype/{子分类}/
排除：.tps文件
```

**步骤2：复制原始文件到_original**
```
从：01_scale/[XX]/{子分类}/
到：nextcloud/Prototype/{子分类}/_original/
包含：所有文件（png、jpg等）
保持格式：不转换
```

**示例**：
```
用户上传了symbol的文件后，程序执行：

1. 从 02_done/[img-100]/symbol/ 复制所有.webp
   → nextcloud/Prototype/symbol/

2. 从 01_scale/[100]/symbol/ 复制所有.png
   → nextcloud/Prototype/symbol/_original/

结果：
nextcloud/Prototype/symbol/
├── _original/
│   ├── h1_dice.png
│   ├── h1_dice_blur.png
│   └── ...
├── h1_dice.webp
├── h1_dice_blur.webp
└── ...
```

### 6. 进度追踪
- **整体级别追踪**：Prototype作为一个整体追踪进度（不单独追踪子分类）
- **原因**：这些子分类的素材基本会同时做完
- **UI展示**：
  ```
  Prototype  [✅✅✅✅] 100% (已上传)
  ```
  或
  ```
  Prototype  [✅✅✅❌] 75% (未上传)
  ```
- **检测逻辑**：
  - 检查所有7个子分类下的文件状态
  - 计算整体进度（所有子分类的平均值或最小值）
  - 建议使用最小值：只有所有子分类都完成才显示100%

---

## 🔍 与普通分类的对比总结

| 特性 | 普通功能分类 | Prototype |
|------|-------------|-----------|
| **子分类层级** | 无（直接存放文件） | 有（7个固定子分类） |
| **子分类名称** | 不适用 | 固定：big_win、infoboard等7个 |
| **nextcloud结构** | 扁平化（所有文件在根目录） | 保留子分类结构 |
| **nextcloud内容** | 仅处理后的文件 | 处理后的文件 + _original目录 |
| **_original目录** | 无 | 每个子分类下都有，存放01_scale的原始文件 |
| **复制步骤** | 1步（02_done → nextcloud） | 2步（02_done → nextcloud + 01_scale → _original） |
| **工作流路径** | 简单路径 | 路径包含子分类 |

---

## 📝 已明确的规则

- [x] ✅ 是否所有项目都有Prototype？
  - **通常有**：大部分项目都会有Prototype
  - **有时没有**：如果是帮别人做项目，可能只负责其中几个功能分类，就不会有Prototype
  - **取决于**：用户在程序内是否添加了Prototype这个任务

- [x] ✅ Prototype的子分类是否永远固定为这7个？
  - **基本固定**：这7个子分类基本永远固定
  - **未来可能变化**：如果后面有变化，届时再调整

- [x] ✅ 进度追踪是按Prototype整体还是按每个子分类？
  - **按整体**：Prototype作为一个整体追踪进度
  - **原因**：这些子分类的素材基本会同时做完
  - **UI展示**：显示"Prototype"的整体进度，不单独显示每个子分类的进度

## 📝 待明确的问题

- [ ] 如果用户删除了nextcloud/Prototype/{子分类}/_original中的文件，如何处理？
  - 可能答案：类似处理后的文件，直接显示为缺失状态

---

**参考项目**: D:\work\pgsoft\exp\217_RedDevil\03_Render_VFX\VFX\Export\Prototype
