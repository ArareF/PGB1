# PGB1 DesignSystem 设计系统

**最后更新**: 2026-02-21
**状态**: v2.0 - 工业科幻控制面板风格重做
**原则**: SSOT（单一事实来源）- 所有 UI 参数必须引用此系统，严禁硬编码

**更新记录**:
- v1.0 (2026-02-16): 初版定义
- v1.1 (2026-02-16): 添加明暗主题支持、功能色语义映射、确认最小窗口尺寸
- v1.2 (2026-02-16): 指定自定义字体（猫啃网糖圆体）、暂定所有卡片尺寸
- v2.0 (2026-02-21): 暗色主题视觉重做 — 冷色工业终端风格。字体替换为 URW DIN + 更纱黑体，色板转冷蓝灰，圆角收窄，阴影弱化，新增噪点纹理

---

## 📐 系统架构

```
原子级 Token（基础变量）
    ↓
语义级 Token（用途变量）
    ↓
组件级规范（具体应用）
```

**核心原则**:
1. ✅ 所有颜色、尺寸、动画参数必须定义在此系统中
2. ✅ 组件开发时只能引用Token，不能硬编码
3. ✅ 修改设计只需修改此文件，自动全局生效
4. ✅ 暗色主题采用冷色工业终端 / 深空站控制面板风格

---

## 🎨 一、颜色系统

### 1.1 基础色板（原子级）

#### 主题色（Primary — 冷科技蓝）
```css
--color-primary-50:  #E8F4FD;   /* 最浅 */
--color-primary-100: #C5E3FA;
--color-primary-200: #9DD0F7;
--color-primary-300: #6BB8FF;   /* 主强调色上限 / 链接色 */
--color-primary-400: #5AACF8;
--color-primary-500: #4A9EF5;   /* 基准色 */
--color-primary-600: #3D8AE0;
--color-primary-700: #2D5A8A;   /* 次级强调 / 暗蓝 */
--color-primary-800: #1E3F66;
--color-primary-900: #102A4C;   /* 最深 */
```

**说明**：
- **冷科技蓝** - 功能类按钮和交互元素
- 500 为基准色，比旧版 Material 蓝(#2196F3)更冷、更淡
- 300(#6BB8FF) 用于暗色主题链接色和发光效果
- 蓝色作为强调角色（占比约30%），仅用于边框亮线、激活态、LED发光

#### 中性色（Neutral — 去蓝化中性灰）
```css
--color-neutral-0:    #F0F1F3;   /* 最亮 */
--color-neutral-50:   #E2E4E8;   /* 暗色主题主要文字 */
--color-neutral-100:  #CACED2;
--color-neutral-200:  #B2B4BA;
--color-neutral-300:  #9A9DA4;
--color-neutral-400:  #8A8D96;   /* 暗色主题次级文字 */
--color-neutral-500:  #6A6D76;
--color-neutral-600:  #2E3036;   /* 非活跃元素 */
--color-neutral-700:  #24262A;
--color-neutral-800:  #18191E;   /* 面板底色 */
--color-neutral-900:  #0C0D10;   /* 最深（应用背景） */
--color-neutral-1000: #060708;
```

#### 功能色（Functional）
```css
/* 确认/成功 - 低饱和青绿 */
--color-success-light:  #6DD8B5;
--color-success:        #4ECBA0;   /* 基准 - 用于确认按钮 */
--color-success-dark:   #3AA882;

/* 警戒/警告 - 降饱和暖色 */
--color-warning-light:  #F0D060;
--color-warning:        #D4B040;   /* 基准 - 用于警戒提示 */
--color-warning-dark:   #B89830;

/* 取消/关闭/危险 - 冷红 */
--color-danger-light:   #F07070;
--color-danger:         #E05A5A;   /* 基准 - 用于取消/关闭/删除按钮 */
--color-danger-dark:    #C04444;

/* 信息 - 蓝色系（与主题色相同） */
--color-info-light:     #64B5F6;
--color-info:           #2196F3;   /* 基准 */
--color-info-dark:      #1976D2;
```

**功能色语义映射**（已确认）:
- **蓝色系（Primary）**: 功能类按钮、主要交互元素
- **绿色系（Success）**: 确认按钮、成功状态
- **黄色系（Warning）**: 警戒提示、警告状态
- **红色系（Danger）**: 取消按钮、关闭按钮、危险操作、删除操作

---

### 1.2 主题模式系统

#### 主题切换机制

应用支持**明暗两种主题模式**，通过根元素的 `data-theme` 属性切换：

```html
<!-- 亮色模式 -->
<html data-theme="light">

<!-- 暗色模式 -->
<html data-theme="dark">
```

**实现方式**：
```css
/* 定义两套主题变量 */
:root[data-theme="light"] {
  /* 亮色主题变量 */
}

:root[data-theme="dark"] {
  /* 暗色主题变量 */
}
```

---

### 1.3 语义化颜色（语义级）

#### 背景色

**亮色模式**：
```css
:root[data-theme="light"] {
  /* 应用背景 */
  --bg-app:               #F0F2F5;   /* 浅灰色背景 */

  /* 毛玻璃背景（白色基底） */
  --bg-glass-subtle:      rgba(255, 255, 255, 0.05);
  --bg-glass-medium:      rgba(255, 255, 255, 0.10);
  --bg-glass-strong:      rgba(255, 255, 255, 0.15);

  /* 内容背景 */
  --bg-primary:           rgba(255, 255, 255, 0.9);    /* 主要内容区 */
  --bg-secondary:         rgba(255, 255, 255, 0.6);    /* 次要内容区 */
  --bg-tertiary:          rgba(255, 255, 255, 0.3);    /* 三级内容区 */

  /* 悬停/激活状态 */
  --bg-hover:             rgba(33, 150, 243, 0.08);    /* 悬停背景 */
  --bg-active:            rgba(33, 150, 243, 0.16);    /* 激活背景 */
  --bg-selected:          rgba(33, 150, 243, 0.12);    /* 选中背景 */
}
```

**暗色模式**（高对比工业终端风格，6:3:1 配比）：
```css
:root[data-theme="dark"] {
  /* 应用背景 — 纯净深灰黑 */
  --bg-app:               #0C0D10;

  /* 毛玻璃背景（中性深灰黑基底） */
  --bg-glass-subtle:      rgba(14, 15, 18, 0.30);
  --bg-glass-medium:      rgba(14, 15, 18, 0.45);
  --bg-glass-strong:      rgba(14, 15, 18, 0.60);

  /* 内容背景 */
  --bg-primary:           rgba(22, 23, 28, 0.85);       /* 主要内容区 */
  --bg-secondary:         rgba(22, 23, 28, 0.55);       /* 次要内容区 */
  --bg-tertiary:          rgba(22, 23, 28, 0.30);       /* 三级内容区 */

  /* 悬停/激活状态 — 蓝色 wash（仅小面积交互反馈） */
  --bg-hover:             rgba(74, 158, 245, 0.10);     /* 悬停背景 */
  --bg-active:            rgba(74, 158, 245, 0.18);     /* 激活背景 */
  --bg-selected:          rgba(74, 158, 245, 0.14);     /* 选中背景 */
}
```

#### 文字颜色

**亮色模式**：
```css
:root[data-theme="light"] {
  --text-primary:         rgba(0, 0, 0, 0.87);         /* 主要文字 */
  --text-secondary:       rgba(0, 0, 0, 0.60);         /* 次要文字 */
  --text-tertiary:        rgba(0, 0, 0, 0.38);         /* 三级文字 */
  --text-inverse:         rgba(255, 255, 255, 0.95);   /* 反色文字 */

  /* 链接文字 */
  --text-link:            var(--color-primary-600);
  --text-link-hover:      var(--color-primary-700);
}
```

**暗色模式**（高对比近白色调）：
```css
:root[data-theme="dark"] {
  --text-primary:         #E2E4E8;                     /* 近白灰，高对比 */
  --text-secondary:       #8A8D96;                     /* 中灰 */
  --text-tertiary:        #4A4D54;                     /* 深灰 */
  --text-inverse:         #0C0D10;                     /* 反色 */

  /* 链接文字 */
  --text-link:            var(--color-primary-300);    /* #6BB8FF */
  --text-link-hover:      #8ECFFF;                    /* 亮冰蓝 */
}
```

#### 边框颜色

**亮色模式**：
```css
:root[data-theme="light"] {
  --border-light:         rgba(0, 0, 0, 0.08);         /* 轻边框 */
  --border-medium:        rgba(0, 0, 0, 0.12);         /* 中边框 */
  --border-heavy:         rgba(0, 0, 0, 0.20);         /* 重边框 */

  --border-focus:         var(--color-primary-500);    /* 聚焦边框 */
  --border-error:         var(--color-danger);         /* 错误边框 */
}
```

**暗色模式**（冷蓝色亮线 — 蓝色的主要载体）：
```css
:root[data-theme="dark"] {
  --border-light:         rgba(100, 180, 255, 0.10);   /* 轻边框 */
  --border-medium:        rgba(100, 180, 255, 0.14);   /* 中边框 */
  --border-heavy:         rgba(100, 180, 255, 0.22);   /* 重边框 */

  --border-focus:         #6BB8FF;                     /* 聚焦边框 */
  --border-error:         var(--color-danger);         /* 错误边框 */
}
```

#### 状态色（已知需求）
```css
/* 任务状态标签（不区分主题） */
--status-pending:       var(--color-neutral-400);    /* 灰色 - 未开始 */
--status-in-progress:   var(--color-warning);        /* 黄色 - 进行中 */
--status-completed:     var(--color-success);        /* 绿色 - 已完成 */

/* 素材进度状态（不区分主题） */
--progress-none:        var(--color-neutral-300);    /* 未开始 */
--progress-original:    var(--color-info-light);     /* 原始文件 */
--progress-scaled:      var(--color-warning-light);  /* 已缩放 */
--progress-done:        var(--color-success-light);  /* 已完成 */
--progress-uploaded:    var(--color-success);        /* 已上传 */
```

#### 功能色语义应用
```css
/* 功能类（蓝色系） */
--action-primary:       var(--color-primary-500);    /* 主要操作 */
--action-primary-hover: var(--color-primary-600);
--action-primary-active: var(--color-primary-700);

/* 确认类（绿色系） */
--action-confirm:       var(--color-success);        /* 确认操作 */
--action-confirm-hover: var(--color-success-dark);

/* 警戒类（黄色系） */
--action-warning:       var(--color-warning);        /* 警戒提示 */
--action-warning-hover: var(--color-warning-dark);

/* 取消/关闭类（红色系） */
--action-danger:        var(--color-danger);         /* 危险操作 */
--action-danger-hover:  var(--color-danger-dark);
```

---

### 1.3 透明度层级
```css
--opacity-0:    0;
--opacity-5:    0.05;
--opacity-10:   0.10;
--opacity-15:   0.15;
--opacity-20:   0.20;
--opacity-30:   0.30;
--opacity-40:   0.40;
--opacity-50:   0.50;
--opacity-60:   0.60;
--opacity-70:   0.70;
--opacity-80:   0.80;
--opacity-90:   0.90;
--opacity-100:  1.0;
```

---

## 📏 二、间距系统

### 2.1 基础间距（8px基准）
```css
--spacing-0:    0px;
--spacing-1:    4px;     /* 0.25rem */
--spacing-2:    8px;     /* 0.5rem */
--spacing-3:    12px;    /* 0.75rem */
--spacing-4:    16px;    /* 1rem */
--spacing-5:    20px;    /* 1.25rem */
--spacing-6:    24px;    /* 1.5rem */
--spacing-8:    32px;    /* 2rem */
--spacing-10:   40px;    /* 2.5rem */
--spacing-12:   48px;    /* 3rem */
--spacing-16:   64px;    /* 4rem */
--spacing-20:   80px;    /* 5rem */
--spacing-24:   96px;    /* 6rem */
```

### 2.2 语义化间距
```css
/* 组件内边距 */
--padding-xs:   var(--spacing-1);    /* 4px */
--padding-sm:   var(--spacing-2);    /* 8px */
--padding-md:   var(--spacing-4);    /* 16px */
--padding-lg:   var(--spacing-6);    /* 24px */
--padding-xl:   var(--spacing-8);    /* 32px */

/* 组件外边距 */
--margin-xs:    var(--spacing-1);    /* 4px */
--margin-sm:    var(--spacing-2);    /* 8px */
--margin-md:    var(--spacing-4);    /* 16px */
--margin-lg:    var(--spacing-6);    /* 24px */
--margin-xl:    var(--spacing-8);    /* 32px */

/* 卡片间距 */
--gap-card:     var(--spacing-4);    /* 卡片之间的间距 */

/* 页面边距 */
--padding-page: var(--spacing-6);    /* 页面四周的内边距 */
```

---

## 🔘 三、圆角系统

```css
--radius-none:  0px;
--radius-sm:    4px;
--radius-md:    8px;
--radius-lg:    12px;
--radius-xl:    16px;
--radius-2xl:   20px;
--radius-3xl:   24px;
--radius-full:  9999px;   /* 完全圆形 */

/* 语义化圆角 — 工业风硬边 */
--radius-button:          var(--radius-sm);     /* 4px — 按钮圆角 */
--radius-card:            var(--radius-md);     /* 8px — 卡片圆角 */
--radius-floating-island: var(--radius-md);     /* 8px — 悬浮岛圆角 */
--radius-tag:             var(--radius-full);   /* 药丸标签不变 */
--radius-input:           var(--radius-sm);     /* 4px — 输入框圆角 */
```

---

## ✨ 四、毛玻璃效果系统

### 4.1 毛玻璃预设

毛玻璃效果在明暗主题下有不同的参数。

#### 极轻毛玻璃（Light Glass）

**两主题通用**（仅 blur，无 bg/border token）：
```css
:root {
  --glass-light-blur: 4px;  /* 下拉菜单遮罩、弹窗内层覆盖层 */
  --panel-blur:       12px; /* 配置面板（状态栏长按）*/
}
```
**用途**：最轻量的模糊遮罩，不影响颜色层，仅需 blur 效果时使用

---

#### 轻微毛玻璃（Subtle Glass）

**亮色模式**：
```css
:root[data-theme="light"] {
  --glass-subtle-bg:          rgba(255, 255, 255, 0.60);
  --glass-subtle-blur:        8px;
  --glass-subtle-border:      1px solid rgba(0, 0, 0, 0.08);
  --glass-subtle-shadow:      0 4px 6px rgba(0, 0, 0, 0.05);
}
```

**暗色模式**（中性深灰黑底 + 冷蓝边框）：
```css
:root[data-theme="dark"] {
  --glass-subtle-bg:          rgba(22, 23, 28, 0.45);
  --glass-subtle-blur:        12px;
  --glass-subtle-border:      1px solid rgba(100, 180, 255, 0.10);
  --glass-subtle-shadow:      0 2px 8px rgba(0, 0, 0, 0.35);
}
```
**用途**: 根层毛玻璃、次要悬浮元素、背景层

#### 中等毛玻璃（Medium Glass）

**亮色模式**：
```css
:root[data-theme="light"] {
  --glass-medium-bg:          rgba(255, 255, 255, 0.70);
  --glass-medium-blur:        16px;
  --glass-medium-border:      1px solid rgba(0, 0, 0, 0.12);
  --glass-medium-shadow:      0 8px 16px rgba(0, 0, 0, 0.10);
}
```

**暗色模式**（一层面板/主功能区）：
```css
:root[data-theme="dark"] {
  --glass-medium-bg:          rgba(22, 23, 28, 0.60);
  --glass-medium-blur:        24px;
  --glass-medium-border:      1px solid rgba(100, 180, 255, 0.12);
  --glass-medium-shadow:      0 4px 12px rgba(0, 0, 0, 0.40);
}
```
**用途**: 主要悬浮岛、卡片、导航栏、主功能区

#### 强毛玻璃（Strong Glass）

**亮色模式**：
```css
:root[data-theme="light"] {
  --glass-strong-bg:          rgba(255, 255, 255, 0.80);
  --glass-strong-blur:        24px;
  --glass-strong-border:      1px solid rgba(0, 0, 0, 0.15);
  --glass-strong-shadow:      0 12px 24px rgba(0, 0, 0, 0.15);
}
```

**暗色模式**（二层面板/弹窗）：
```css
:root[data-theme="dark"] {
  --glass-strong-bg:          rgba(14, 15, 18, 0.75);
  --glass-strong-blur:        18px;
  --glass-strong-border:      1px solid rgba(100, 180, 255, 0.18);
  --glass-strong-shadow:      0 0 24px rgba(74, 158, 245, 0.10);  /* 蓝色外发光 */
}
```
**用途**: 模态弹窗、侧边栏、强调元素

### 4.2 CSS 完整示例
```css
/* 中等毛玻璃效果 */
.glass-medium {
  background: var(--glass-medium-bg);
  backdrop-filter: blur(var(--glass-medium-blur));
  -webkit-backdrop-filter: blur(var(--glass-medium-blur));
  border: var(--glass-medium-border);
  box-shadow: var(--glass-medium-shadow);
  border-radius: var(--radius-card);
}
```

**注意事项**:
- `backdrop-filter` 需要元素下方有内容才能显示效果
- Safari 需要 `-webkit-backdrop-filter` 前缀
- 性能敏感场景（如大量卡片）可考虑降低blur值
- 暗色模式下背景透明度更低，确保内容可读性

### 4.3 噪点纹理叠加

暗色模式下，毛玻璃面板自动叠加冷色噪点纹理（通过 `::after` 伪元素），增强工业磨砂金属质感。

```css
/* 噪点通过 CSS 变量控制 */
:root {
  --noise-opacity: 0;        /* 亮色模式关闭 */
}
:root[data-theme="dark"] {
  --noise-opacity: 0.03;     /* 暗色模式 3% 不透明度 */
}
```

**实现方式**：SVG `feTurbulence` 滤镜作为 data URI 背景，`mix-blend-mode: overlay` 混合。
**效果**：在深灰黑面板表面增加微弱的颗粒感，模拟磨砂金属面板在低照度下的质感。
**性能**：SVG 256x256 平铺，GPU 加速渲染，对性能影响极小。

---

## 📝 五、排版系统

### 5.1 字体家族

#### 自定义字体加载
```css
@font-face {
  font-family: 'URWDINRegular';
  src: url('/fonts/urw-din-regular.ttf') format('truetype');
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}

@font-face {
  font-family: 'SarasaTermSC';
  src: url('/fonts/SarasaTermSCNerd.ttc') format('collection');
  font-weight: normal;
  font-style: normal;
  font-display: swap;
}
```

#### 字体家族定义
```css
/* 主字体（URW DIN 西文 + 更纱黑体 中文） */
--font-sans: "URWDINRegular", "SarasaTermSC", "Microsoft YaHei", sans-serif;

/* 等宽字体（更纱黑体 Term 变体本身是等宽字体） */
--font-mono: "SarasaTermSC", "Consolas", monospace;

/* 默认字体 */
--font-family-base: var(--font-sans);
```

**说明**:
- **西文字体**: URW DIN Regular — 工业感几何无衬线体
- **中文字体**: 更纱黑体 Term SC Nerd — 等宽中文字体，兼顾代码和界面
- **Fallback**: 系统默认字体（确保字体加载失败时的兼容性）
- **font-display: swap**: 优先显示备用字体，避免FOIT（Flash of Invisible Text）

### 5.2 字号系统
```css
--text-2xs:     10px;    /* 0.625rem - 紧凑标签/角标 */
--text-xs:      12px;    /* 0.75rem */
--text-sm:      14px;    /* 0.875rem */
--text-base:    16px;    /* 1rem - 基准字号 */
--text-lg:      18px;    /* 1.125rem */
--text-xl:      20px;    /* 1.25rem */
--text-2xl:     24px;    /* 1.5rem */
--text-3xl:     30px;    /* 1.875rem */
--text-4xl:     36px;    /* 2.25rem */
--text-5xl:     48px;    /* 3rem */

/* 语义化字号 */
--text-caption:     var(--text-xs);      /* 说明文字 */
--text-body:        var(--text-base);    /* 正文 */
--text-title:       var(--text-2xl);     /* 标题 */
--text-heading:     var(--text-3xl);     /* 大标题 */
```

### 5.3 行高
```css
--leading-none:     1;
--leading-tight:    1.25;
--leading-snug:     1.375;
--leading-normal:   1.5;      /* 默认 */
--leading-relaxed:  1.625;
--leading-loose:    2;

/* 语义化行高 */
--leading-body:     var(--leading-normal);    /* 正文行高 */
--leading-title:    var(--leading-tight);     /* 标题行高 */
```

### 5.4 字重
```css
--font-light:       300;
--font-normal:      400;      /* 默认 */
--font-medium:      500;
--font-semibold:    600;
--font-bold:        700;
--font-extrabold:   800;

/* 语义化字重 */
--font-weight-body:     var(--font-normal);   /* 正文 */
--font-weight-title:    var(--font-semibold); /* 标题 */
--font-weight-emphasis: var(--font-bold);     /* 强调 */
```

### 5.5 字间距
```css
--tracking-tighter:  -0.05em;
--tracking-tight:    -0.025em;
--tracking-normal:   0;
--tracking-wide:     0.025em;
--tracking-wider:    0.05em;
--tracking-widest:   0.1em;
```

---

## 🌑 六、阴影系统

### 6.1 阴影预设
```css
--shadow-none:   none;
--shadow-xs:     0 1px 2px 0 rgba(0, 0, 0, 0.05);
--shadow-sm:     0 2px 4px 0 rgba(0, 0, 0, 0.06);
--shadow-md:     0 4px 6px -1px rgba(0, 0, 0, 0.1),
                 0 2px 4px -1px rgba(0, 0, 0, 0.06);
--shadow-lg:     0 10px 15px -3px rgba(0, 0, 0, 0.1),
                 0 4px 6px -2px rgba(0, 0, 0, 0.05);
--shadow-xl:     0 20px 25px -5px rgba(0, 0, 0, 0.1),
                 0 10px 10px -5px rgba(0, 0, 0, 0.04);
--shadow-2xl:    0 25px 50px -12px rgba(0, 0, 0, 0.25);
--shadow-inner:  inset 0 2px 4px 0 rgba(0, 0, 0, 0.06);

/* 语义化阴影 — 弱化 Material 阴影，层级靠透明度/边框拉开 */
--shadow-card:           0 2px 6px rgba(0, 0, 0, 0.25);
--shadow-floating:       0 4px 12px rgba(0, 0, 0, 0.30);
--shadow-modal:          0 0 20px rgba(74, 158, 245, 0.08),
                         0 8px 24px rgba(0, 0, 0, 0.40);
--shadow-dropdown:       var(--shadow-floating);
```

### 6.2 悬停阴影增强
```css
--shadow-card-hover:     0 4px 12px rgba(0, 0, 0, 0.30),
                         0 0 1px rgba(100, 180, 255, 0.15);  /* 微弱蓝色边缘光 */
```

---

## ⏱️ 七、动画系统

### 7.1 过渡时长
```css
--duration-instant:  0ms;
--duration-fast:     150ms;
--duration-normal:   300ms;
--duration-slow:     500ms;
--duration-slower:   700ms;

/* 语义化时长 */
--duration-hover:       var(--duration-fast);    /* 悬停动画 */
--duration-slide:       var(--duration-normal);  /* 滑动动画 */
--duration-fade:        var(--duration-normal);  /* 淡入淡出 */
--duration-modal:       var(--duration-slow);    /* 模态弹窗 */
```

### 7.2 缓动函数
```css
--ease-linear:      cubic-bezier(0, 0, 1, 1);
--ease-in:          cubic-bezier(0.4, 0, 1, 1);
--ease-out:         cubic-bezier(0, 0, 0.2, 1);
--ease-in-out:      cubic-bezier(0.4, 0, 0.2, 1);
--ease-bounce:      cubic-bezier(0.68, -0.55, 0.265, 1.55);
--ease-smooth:      cubic-bezier(0.25, 0.1, 0.25, 1);

/* 语义化缓动 */
--ease-hover:       var(--ease-out);        /* 悬停效果 */
--ease-slide-in:    var(--ease-out);        /* 滑入效果 */
--ease-slide-out:   var(--ease-in);         /* 滑出效果 */
--ease-modal:       var(--ease-in-out);     /* 模态弹窗 */
```

### 7.3 常用过渡组合
```css
/* 通用过渡 */
--transition-all:       all var(--duration-normal) var(--ease-in-out);
--transition-color:     color var(--duration-fast) var(--ease-out);
--transition-bg:        background-color var(--duration-fast) var(--ease-out);
--transition-transform: transform var(--duration-normal) var(--ease-out);
--transition-opacity:   opacity var(--duration-normal) var(--ease-in-out);

/* 卡片悬停 */
--transition-card-hover:
  transform var(--duration-hover) var(--ease-hover),
  box-shadow var(--duration-hover) var(--ease-hover);
```

---

## 🧱 八、组件规范

### 8.1 按钮（Button）

#### 尺寸
```css
/* 小按钮 */
--button-sm-height:     32px;
--button-sm-padding-x:  var(--spacing-3);   /* 12px */
--button-sm-font-size:  var(--text-sm);     /* 14px */

/* 中按钮（默认） */
--button-md-height:     40px;
--button-md-padding-x:  var(--spacing-4);   /* 16px */
--button-md-font-size:  var(--text-base);   /* 16px */

/* 大按钮 */
--button-lg-height:     48px;
--button-lg-padding-x:  var(--spacing-6);   /* 24px */
--button-lg-font-size:  var(--text-lg);     /* 18px */

/* 通用 */
--button-border-radius: var(--radius-button);
--button-font-weight:   var(--font-medium);
```

#### 变体样式

**功能类按钮（蓝色系）**
```css
/* Primary Button - 功能类 */
--button-primary-bg:            var(--color-primary-500);
--button-primary-bg-hover:      var(--color-primary-600);
--button-primary-bg-active:     var(--color-primary-700);
--button-primary-text:          var(--color-neutral-0);
```

**确认类按钮（绿色系）**
```css
/* Success Button - 确认类 */
--button-success-bg:            var(--color-success);
--button-success-bg-hover:      var(--color-success-dark);
--button-success-bg-active:     #2E7D32;
--button-success-text:          var(--color-neutral-0);
```

**警戒类按钮（黄色系）**
```css
/* Warning Button - 警戒类 */
--button-warning-bg:            var(--color-warning);
--button-warning-bg-hover:      var(--color-warning-dark);
--button-warning-bg-active:     #F57C00;
--button-warning-text:          rgba(0, 0, 0, 0.87);   /* 暗色文字，确保对比度 */
```

**取消/关闭/危险类按钮（红色系）**
```css
/* Danger Button - 取消/关闭/删除 */
--button-danger-bg:             var(--color-danger);
--button-danger-bg-hover:       var(--color-danger-dark);
--button-danger-bg-active:      #C62828;
--button-danger-text:           var(--color-neutral-0);
```

**次要按钮（毛玻璃风格）**
```css
/* Secondary Button（暗色主题） */
--button-secondary-bg:          var(--glass-subtle-bg);
--button-secondary-bg-hover:    rgba(100, 180, 255, 0.12);
--button-secondary-border:      var(--border-medium);
--button-secondary-text:        var(--text-primary);
```

**幽灵按钮（透明背景）**
```css
/* Ghost Button（暗色主题） */
--button-ghost-bg:              transparent;
--button-ghost-bg-hover:        rgba(74, 158, 245, 0.10);
--button-ghost-text:            var(--color-primary-300);
```

**禁用状态**
```css
/* Disabled（暗色适配） */
--button-disabled-bg:           rgba(42, 47, 62, 0.60);  /* 暗蓝灰 */
--button-disabled-text:         var(--text-tertiary);
--button-disabled-opacity:      0.6;
```

**使用指南**：
- **功能类（蓝色）**: 普通操作按钮（打开、编辑、查看等）
- **确认类（绿色）**: 确认操作（提交、保存、完成等）
- **警戒类（黄色）**: 需要用户注意的操作（重置、覆盖等）
- **危险类（红色）**: 危险操作（删除、取消、关闭等）

---

### 8.2 卡片（Card）

#### 通用卡片属性
```css
--card-border-radius:       var(--radius-card);      /* 16px */
--card-padding:             var(--spacing-4);        /* 16px */
--card-shadow:              var(--shadow-card);
--card-shadow-hover:        var(--shadow-card-hover);
--card-transition:          var(--transition-card-hover);

/* 毛玻璃卡片 */
--card-glass-bg:            var(--glass-medium-bg);
--card-glass-border:        var(--glass-medium-border);
--card-glass-backdrop:      blur(var(--glass-medium-blur));
```

#### 项目卡片（Project Card）
```css
--card-project-width:       320px;
--card-project-height:      160px;   /* 2:1 比例 */
--card-project-icon-size:   80px;    /* ICON区域尺寸 */
--card-project-padding:     var(--spacing-5);  /* 20px */
--card-project-gap:         var(--spacing-3);  /* 内部元素间距 12px */
```

**布局说明**:
- 左侧：80x80 ICON区域
- 右侧：项目名 + 截止日期
- 底部：进度条（全宽）
- 总高度考虑：内边距(20x2) + ICON(80) + 间距(12) + 进度条(20) ≈ 152px，实际设为160px（~2:1比例）

#### 任务卡片（Task Card）
```css
--card-task-width:          280px;
--card-task-height:         140px;   /* 2:1 比例 */
--card-task-padding:        var(--spacing-4);  /* 16px */
--card-task-gap:            var(--spacing-3);  /* 内部元素间距 12px */
```

**布局说明**:
- 顶部：任务名（单行或两行）
- 底部：状态标签（左） + 文件大小（右）
- 总高度考虑：内边距(16x2) + 标题(40) + 间距(12) + 底部信息(32) ≈ 116px，实际设为140px（2:1比例）

#### 素材卡片（Material Card）
```css
--card-material-width:      220px;   /* 暂定，后续细调 */
--card-material-height:     280px;   /* 暂定，后续细调 */
--card-material-padding:    var(--spacing-3);  /* 12px */
--card-material-gap:        var(--spacing-2);  /* 内部元素间距 8px */
--card-material-preview-height: 165px;  /* 预览图固定高度（3:4比例） */
```

**布局说明**:
- 预览图区域：220x165（接近3:4比例，适合竖版素材）
- 文件名：单行或两行，约30px
- 底部标签：进度标签 + 大小标签，约32px
- 总高度：内边距(12x2) + 预览图(165) + 间距(8) + 文件名(30) + 间距(8) + 标签(32) ≈ 267px，实际设为280px

#### 普通卡片（Normal Card）
```css
--card-normal-width:        200px;   /* 暂定，后续细调 */
--card-normal-height:       240px;   /* 暂定，后续细调 */
--card-normal-padding:      var(--spacing-3);  /* 12px */
--card-normal-gap:          var(--spacing-2);  /* 内部元素间距 8px */
--card-normal-preview-height: 150px; /* 预览图固定高度（4:3比例） */
```

**布局说明**:
- 预览图区域：200x150（4:3比例）
- 格式标签：右上角悬浮
- 文件名：底部，约40px
- 总高度：内边距(12x2) + 预览图(150) + 间距(8) + 文件名(40) ≈ 222px，实际设为240px

#### 卡片悬停效果（工业风：不放大，轻微上浮）
```css
--card-hover-scale:         1.0;     /* 不放大 */
--card-hover-lift:          -2px;    /* 轻微上浮 */
```

#### 卡片网格布局
```css
/* 卡片之间的间距 */
--card-grid-gap:            var(--spacing-4);    /* 16px，暂定 */
--card-grid-gap-large:      var(--spacing-6);    /* 24px，大间距 */

/* 网格列数（响应式） */
--card-grid-columns-sm:     2;    /* 小屏幕：2列 */
--card-grid-columns-md:     3;    /* 中屏幕：3列 */
--card-grid-columns-lg:     4;    /* 大屏幕：4列 */
--card-grid-columns-xl:     5;    /* 超大屏：5列 */
```

**网格布局示例**:
```css
.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--card-project-width), 1fr));
  gap: var(--card-grid-gap);
  padding: var(--padding-page);
}
```

**说明**:
- 所有卡片尺寸均为**暂定值**，后续根据实际内容和视觉效果细调
- 使用 `auto-fill` + `minmax()` 实现响应式网格布局
- 卡片宽度为最小值，允许自动拉伸填充空间

---

### 8.3 标签（Tag）

#### 状态标签（Status Tag）
```css
--tag-height:               24px;
--tag-padding-x:            var(--spacing-3);    /* 12px */
--tag-font-size:            var(--text-xs);      /* 12px */
--tag-font-weight:          var(--font-medium);
--tag-border-radius:        var(--radius-tag);   /* 完全圆形 */

/* 状态颜色（任务卡片） */
--tag-status-pending-bg:    var(--color-neutral-400);     /* 灰色 */
--tag-status-progress-bg:   var(--color-warning);         /* 黄色 */
--tag-status-completed-bg:  var(--color-success);         /* 绿色 */
--tag-status-text:          var(--color-neutral-0);       /* 白色文字 */

/* 素材进度标签 */
--tag-progress-none-bg:     var(--progress-none);
--tag-progress-original-bg: var(--progress-original);
--tag-progress-scaled-bg:   var(--progress-scaled);
--tag-progress-done-bg:     var(--progress-done);
--tag-progress-uploaded-bg: var(--progress-uploaded);
```

#### 格式标签（Format Tag）
```css
--tag-format-bg:            var(--glass-subtle-bg);
--tag-format-text:          var(--text-secondary);
--tag-format-border:        var(--border-light);
```

---

### 8.4 输入框（Input）

```css
--input-height-sm:          32px;
--input-height-md:          40px;
--input-height-lg:          48px;

--input-padding-x:          var(--spacing-3);    /* 12px */
--input-border-radius:      var(--radius-input); /* 8px */
--input-border:             1px solid var(--border-heavy);
--input-border-focus:       2px solid var(--border-focus);
--input-border-error:       2px solid var(--border-error);

--input-bg:                 rgba(255, 255, 255, 0.8);
--input-bg-disabled:        var(--color-neutral-100);
--input-text:               var(--text-primary);
--input-placeholder:        var(--text-tertiary);

--input-transition:         border-color var(--duration-fast) var(--ease-out);
```

---

### 8.5 悬浮岛（Floating Island）

```css
/* 顶部导航栏悬浮岛 */
--floating-navbar-height:       100px;
--floating-navbar-padding-x:    var(--spacing-4);
--floating-navbar-padding-y:    var(--spacing-2);
--floating-navbar-radius:       var(--radius-floating-island);  /* 20px */
--floating-navbar-bg:           var(--glass-medium-bg);
--floating-navbar-border:       var(--glass-medium-border);
--floating-navbar-shadow:       var(--shadow-floating);
--floating-navbar-gap:          var(--spacing-4);    /* 悬浮岛之间的间距 */

/* 左侧边栏 */
--floating-sidebar-width:       100px;
--floating-sidebar-padding:     var(--spacing-3);
--floating-sidebar-item-size:   60px;    /* 图标按钮尺寸 */
--floating-sidebar-gap:         var(--spacing-2);    /* 图标之间间距 */

/* 主功能区 */
--floating-main-bg:             var(--glass-medium-bg);
--floating-main-padding:        var(--spacing-6);
--floating-main-radius:         var(--radius-floating-island);
```

---

### 8.6 侧边栏详情页（Sidebar Detail）

```css
--sidebar-width-min:        20%;         /* 最小宽度（相对应用窗口） */
--sidebar-width-default:    30%;         /* 默认宽度 */
--sidebar-width-max:        60%;         /* 最大宽度 */

--sidebar-bg:               var(--glass-strong-bg);
--sidebar-backdrop:         blur(var(--glass-strong-blur));
--sidebar-border:           var(--glass-strong-border);
--sidebar-shadow:           var(--shadow-xl);

/* 动画 */
--sidebar-slide-in-duration:  var(--duration-normal);   /* 300ms */
--sidebar-slide-out-duration: 200ms;
--sidebar-slide-in-ease:      var(--ease-slide-in);
--sidebar-slide-out-ease:     var(--ease-slide-out);
```

---

### 8.7 窗口控制按钮

```css
--window-control-size:      32px;    /* 按钮尺寸 */
--window-control-gap:       var(--spacing-2);    /* 按钮间距 */
--window-control-radius:    var(--radius-full);

/* 颜色 — 冷蓝色悬停 */
--window-control-bg:        rgba(22, 23, 28, 0.55);
--window-close-bg:          rgba(224, 90, 90, 0.7);
--window-minimize-hover:    rgba(100, 180, 255, 0.12);
--window-maximize-hover:    rgba(100, 180, 255, 0.12);
--window-close-hover:       var(--color-danger);
--window-close-text:        var(--color-neutral-0);
```

**说明**：
- 关闭按钮使用红色系（--color-danger），符合"关闭类用红色"的规范
- 最小化和最大化按钮使用中性色悬停效果

---

### 8.8 悬浮操作按钮（Floating Action Button）

导航栏区域的独立悬浮岛按钮（如「更多」菜单），也作为后续同类按钮的尺寸基准。

```css
--floating-action-height:       var(--button-lg-height);   /* 48px — 与大按钮统一 */
--floating-action-padding-x:    var(--spacing-5);           /* 20px */
--floating-action-radius:       var(--floating-navbar-radius); /* 与悬浮岛统一圆角 */
--floating-action-icon-size:    16px;
```

**说明**：
- 圆角与悬浮岛统一（`--floating-navbar-radius`），视觉上属于同一层级
- 高度采用 `--button-lg-height`（48px），确保可点击区域足够大
- 后续导航栏区域新增同类按钮时，复用这组 token

---

## 🔲 噪点纹理

暗色主题下，主布局 `.main-layout::before` 叠加一层极低不透明度的 SVG 噪点：

```css
.main-layout::before {
  content: '';
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 9998;
  background-image: url("data:image/svg+xml,...SVG噪点...");
  opacity: 0.03;           /* 2-5%，几乎不可见但增加质感 */
  mix-blend-mode: overlay;
}
```

**作用**：消除纯色大面积区域的"数字感"，增加工业面板的物理质感。

---

## 📱 九、响应式断点

```css
/* 屏幕尺寸断点 */
--breakpoint-sm:    640px;
--breakpoint-md:    768px;
--breakpoint-lg:    1024px;
--breakpoint-xl:    1280px;
--breakpoint-2xl:   1536px;

/* 最小窗口尺寸（已确认） */
--window-min-width:     1280px;
--window-min-height:    720px;
```

---

## 🎯 十、Z-Index 层级系统

```css
--z-base:           0;
--z-dropdown:       1000;
--z-sticky:         1020;
--z-fixed:          1030;
--z-modal-backdrop: 1040;
--z-modal:          1050;
--z-popover:        1060;
--z-tooltip:        1070;
--z-notification:   1080;
--z-max:            9999;

/* 语义化层级 */
--z-sidebar:        var(--z-fixed);
--z-navbar:         var(--z-fixed);
--z-floating-window: var(--z-modal);
```

---

## 📦 十一、实际应用示例

### 示例1：中等毛玻璃卡片
```css
.card {
  /* 布局 */
  width: var(--card-material-width);
  height: var(--card-material-height);
  padding: var(--card-padding);
  border-radius: var(--card-border-radius);

  /* 毛玻璃效果 */
  background: var(--card-glass-bg);
  backdrop-filter: blur(var(--glass-medium-blur));
  border: var(--card-glass-border);
  box-shadow: var(--card-shadow);

  /* 动画 */
  transition: var(--card-transition);
}

.card:hover {
  transform: scale(var(--card-hover-scale)) translateY(var(--card-hover-lift));
  box-shadow: var(--card-shadow-hover);
}
```

### 示例2：按钮变体

#### 功能类按钮（蓝色）
```css
.button-primary {
  /* 尺寸 */
  height: var(--button-md-height);
  padding: 0 var(--button-md-padding-x);
  font-size: var(--button-md-font-size);
  font-weight: var(--button-font-weight);
  border-radius: var(--button-border-radius);

  /* 颜色 */
  background-color: var(--button-primary-bg);
  color: var(--button-primary-text);
  border: none;

  /* 动画 */
  transition: var(--transition-bg);
}

.button-primary:hover {
  background-color: var(--button-primary-bg-hover);
}

.button-primary:active {
  background-color: var(--button-primary-bg-active);
}
```

#### 确认按钮（绿色）
```css
.button-success {
  height: var(--button-md-height);
  padding: 0 var(--button-md-padding-x);
  font-size: var(--button-md-font-size);
  font-weight: var(--button-font-weight);
  border-radius: var(--button-border-radius);

  background-color: var(--button-success-bg);
  color: var(--button-success-text);
  border: none;

  transition: var(--transition-bg);
}

.button-success:hover {
  background-color: var(--button-success-bg-hover);
}
```

#### 警戒按钮（黄色）
```css
.button-warning {
  height: var(--button-md-height);
  padding: 0 var(--button-md-padding-x);
  font-size: var(--button-md-font-size);
  font-weight: var(--button-font-weight);
  border-radius: var(--button-border-radius);

  background-color: var(--button-warning-bg);
  color: var(--button-warning-text);
  border: none;

  transition: var(--transition-bg);
}

.button-warning:hover {
  background-color: var(--button-warning-bg-hover);
}
```

#### 危险按钮（红色）
```css
.button-danger {
  height: var(--button-md-height);
  padding: 0 var(--button-md-padding-x);
  font-size: var(--button-md-font-size);
  font-weight: var(--button-font-weight);
  border-radius: var(--button-border-radius);

  background-color: var(--button-danger-bg);
  color: var(--button-danger-text);
  border: none;

  transition: var(--transition-bg);
}

.button-danger:hover {
  background-color: var(--button-danger-bg-hover);
}
```

### 示例3：状态标签
```css
.tag-status {
  /* 布局 */
  display: inline-flex;
  align-items: center;
  height: var(--tag-height);
  padding: 0 var(--tag-padding-x);
  border-radius: var(--tag-border-radius);

  /* 文字 */
  font-size: var(--tag-font-size);
  font-weight: var(--tag-font-weight);
  color: var(--tag-status-text);
}

.tag-status-pending {
  background-color: var(--tag-status-pending-bg);
}

.tag-status-in-progress {
  background-color: var(--tag-status-progress-bg);
}

.tag-status-completed {
  background-color: var(--tag-status-completed-bg);
}
```

---

## 🔧 十二、技术实现建议

### 12.1 主题切换实现

#### 方式1：JavaScript 动态切换
```javascript
// 切换主题
function toggleTheme() {
  const root = document.documentElement;
  const currentTheme = root.getAttribute('data-theme');
  const newTheme = currentTheme === 'light' ? 'dark' : 'light';

  root.setAttribute('data-theme', newTheme);

  // 保存用户偏好
  localStorage.setItem('theme', newTheme);
}

// 初始化主题（从本地存储读取）
function initTheme() {
  const savedTheme = localStorage.getItem('theme') || 'light';
  document.documentElement.setAttribute('data-theme', savedTheme);
}

// 页面加载时初始化
document.addEventListener('DOMContentLoaded', initTheme);
```

#### 方式2：Tauri 命令（推荐）
```rust
// src-tauri/src/main.rs
use tauri::Manager;

#[tauri::command]
fn set_theme(window: tauri::Window, theme: String) -> Result<(), String> {
    window
        .eval(&format!(
            "document.documentElement.setAttribute('data-theme', '{}')",
            theme
        ))
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![set_theme])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

```javascript
// 前端调用
import { invoke } from '@tauri-apps/api/tauri';

async function toggleTheme() {
  const currentTheme = document.documentElement.getAttribute('data-theme');
  const newTheme = currentTheme === 'light' ? 'dark' : 'light';

  await invoke('set_theme', { theme: newTheme });
  localStorage.setItem('theme', newTheme);
}
```

### 12.2 CSS变量定义
```css
/* ===== 亮色主题 ===== */
:root[data-theme="light"] {
  /* 背景色 */
  --bg-app: #F0F2F5;
  --bg-glass-medium: rgba(255, 255, 255, 0.70);
  /* ... 其他亮色变量 */
}

/* ===== 暗色主题 ===== */
:root[data-theme="dark"] {
  /* 背景色 */
  --bg-app: #0C0D10;
  --bg-glass-medium: rgba(22, 23, 28, 0.60);
  /* ... 其他暗色变量 */
}

/* ===== 通用变量（不区分主题） ===== */
:root {
  /* 基础色板 */
  --color-primary-500: #4A9EF5;
  --color-success: #4ECBA0;
  /* ... */

  /* 间距、圆角、阴影等 */
  --spacing-4: 16px;
  --radius-card: 16px;
  /* ... */
}

/* 组件中引用 */
.my-component {
  color: var(--text-primary);            /* 自动适配主题 */
  background: var(--bg-glass-medium);    /* 自动适配主题 */
  padding: var(--spacing-4);             /* 通用变量 */
}
```

### 12.3 Tauri 窗口配置
```json
// tauri.conf.json
{
  "tauri": {
    "windows": [{
      "title": "PGB1",
      "width": 1280,
      "height": 720,
      "minWidth": 1280,
      "minHeight": 720,
      "decorations": false,  // 无边框窗口
      "transparent": true     // 支持毛玻璃效果
    }]
  }
}
```

### 12.4 可选：JSON配置
如果需要在JavaScript中动态访问这些值，可以创建对应的JSON配置文件：

```json
{
  "colors": {
    "primary": {
      "500": "#4A9EF5"
    },
    "success": "#4ECBA0",
    "warning": "#D4B040",
    "danger": "#E05A5A"
  },
  "spacing": {
    "4": "16px"
  }
}
```

---

## ❓ 待明确的细节

### 设计细节
- [x] ✅ **主题色方案确认**：蓝色系（已确认）
- [x] ✅ **功能色语义**：蓝色-功能类、绿色-确认类、黄色-警戒类、红色-取消/关闭类（已确认）
- [x] ✅ **暗色/亮色模式**：支持明暗主题切换（已确认）
- [x] ✅ **最小窗口尺寸**：1280x720（已确认）
- [x] ✅ **自定义字体**：URW DIN + 更纱黑体 Term SC（v2.0 更新）
- [x] ✅ **卡片尺寸**：已暂定所有卡片尺寸，后续根据实际效果细调
  - 项目卡片：320x220
  - 任务卡片：280x180
  - 素材卡片：220x280
  - 普通卡片：200x240
- [ ] **动画参数微调**：卡片悬停放大比例、过渡时长等需实际测试后确定

### 扩展功能
- [ ] **自定义主题**：是否允许用户自定义颜色方案？
- [ ] **高对比度模式**：是否需要无障碍支持？
- [ ] **性能优化**：大量卡片时毛玻璃效果的性能表现？

---

## 📝 使用指南

### 开发流程
1. **前端开发**：引入 DesignSystem.css，所有样式必须使用CSS变量
2. **禁止硬编码**：严禁在组件中写死颜色、尺寸等值
3. **新增组件**：如需新组件规范，先在此文档定义，再实现
4. **视觉调整**：所有视觉调整只需修改此文件，自动全局生效

### 命名规范
- 使用 `--` 前缀定义CSS变量
- 使用语义化命名（如 `--button-primary-bg` 而非 `--blue-500`）
- 分层命名（原子级 → 语义级 → 组件级）

### 文档维护
- 新增Design Token时必须更新此文档
- 组件开发完成后，将实际使用的参数回填到此文档
- 定期review，清理未使用的Token

---

## 📚 参考资源

- **Material Design 3**: https://m3.material.io/
- **Glassmorphism**: https://glassmorphism.com/
- **Tailwind CSS**: https://tailwindcss.com/docs/customizing-colors
- **Design Tokens**: https://spectrum.adobe.com/page/design-tokens/

---

**相关文档**:
- `界面设计.md` - 整体界面布局和页面结构
- `卡片设计.md` - 各级卡片的详细设计
- `../项目指南.md` - 核心原则（SSOT、模块化）
- `../开发规范.md` - 代码规范（编程阶段必读）

---

**最后更新**: 2026-02-21
**维护者**: Tech Lead
**版本**: v2.0
