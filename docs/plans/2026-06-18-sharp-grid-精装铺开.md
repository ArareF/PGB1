# Sharp Grid 质感精装层·全界面铺开 实施计划

> **For Claude（新窗口执行者）：** 本计划在**新会话冷启动**执行。开工前**必读**三份输入文件（见下），尤其 `docs/新设计风格.md` §14 是规范、`docs/新设计风格-精装样板.html` 是已验收的 canonical 实现（CSS 直接抄）。这是设计原型（纯 HTML/CSS/JS），**无自动化测试**——每步的"验证"= 浏览器打开、拨 `原版⇄精装` 开关肉眼核对清单。

**Goal:** 把已在主页卡片验收的 §14「质感精装层」，铺到 `新设计风格示意.html` 的全部四个视图 + 共享外壳，并加一个全局 `原版⇄精装` 对比开关，使该文件成为唯一的可交互精装原型。

**Architecture:** 不重写结构——`示意.html` 已有完整四视图扁平版。做法是**叠加质感层**：把样板里已证明的 craft CSS 整段移植进来，用 `body.craft` 类门控，再给三个剩余视图的卡片/素材/Inspector/按钮/侧边栏补上 craft 标记与小料。骨架与产品决策（§13）一律不动。

**Tech Stack:** 纯静态 HTML + CSS（CSS 变量 / clip-path / mask / box-shadow inset）+ 原生 JS。字体 Chakra Petch + IBM Plex Mono（已在 `<head>` 引入）。

---

## 输入文件（开工前必读）

| 文件 | 作用 |
|------|------|
| `docs/新设计风格.md` **§14** | **规范**。质感原料 token、选中=状态色、工业小料、动效、组件基线、踩坑清单 |
| `docs/新设计风格-精装样板.html` | **canonical 实现**。主页卡 + 组件样板 + 点阵控制台，craft CSS 全在这里，**直接抄** |
| `docs/新设计风格示意.html` | **改造目标**。四视图扁平版（1057 行）。视图锚点见下 |

**`示意.html` 视图锚点**（行号近似，以实际为准）：
- `:root` token：~16–53　|　卡片 CSS：~264–306　|　标签：~308–319　|　筛选 chip：~331–346　|　素材/Inspector：~348–460　|　设计规范页：~462–506
- `#view-home`：~558–618　|　`#view-tasks`：~620–687　|　`#view-assets`：~689–814　|　`#view-style`：~816–918　|　`<script>`：~924–1054

## 产出物

- 改造后的 `docs/新设计风格示意.html`：四视图全精装 + 全局 `原版⇄精装` 开关。
- 同步更新 `docs/新设计风格.md`（若铺开中发现样板未覆盖的新决策，回填 §14）。

## 范围红线（不做）

- ❌ 不破三铁律：无玻璃 backdrop-filter、无大阴影模糊、无大圆角。
- ❌ 不碰真实 `src/` Vue 代码（本计划只动 docs 原型）。
- ❌ 不改产品决策（§13：左栏=启动台 / 无面包屑 / 顶栏两行 / 无统计块）。
- ❌ 类型标签（序列帧/静帧/视频/源文件）保持中性灰，颜色只给状态（§12.9）。

---

## 踩坑清单（来自 §14，务必先看，避免重犯）

1. **书签角元素不要挂 `craft-only` 类**——会被 `body.craft .craft-only{display:block}` 覆盖而常驻；它只受 `.sel` 控制。
2. **停用斜纹必须用亮线** `rgba(255,255,255,.055)`，黑线叠黑底=隐形。
3. **hover 只走 `transform`/`filter`，绝不碰 `box-shadow`**——否则与选中态色框抢同优先级，hover 选中卡会丢顶线。
4. **box-shadow 要能过渡**，未选中态须补"透明占位层"使层数与选中态一致（见 §14.6）。
5. **点阵层 `z-index:-1`** 落在 `.cbody`（须 `position:relative;z-index:1`）的文字之下、底色之上，否则糊字。
6. **选中=状态色**用 `--sel` 变量（§14.3）；但**素材卡选中仍为蓝**（选中语义，见 §14.3 表格末行）。

---

## Craft 层样式（DRY 核心：从样板整段移植）

**Task 0 会把下面这块一次性粘进 `示意.html` 的 `<style>`**（值与样板一致；如样板有微调以样板为准）。后续任务只加 class / markup，不重复写质感。

```css
/* —— 质感原料 token（追加进 :root） —— */
--edge-hi: inset 0 1px 0 0 rgba(255,255,255,.045);
--edge-lo: inset 0 -1px 0 0 rgba(0,0,0,.45);
--noise: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
--dot-gap:8.5px; --dot-r:.8px; --dot-fade:45%; --dot-alpha:.18;
```

> 表面三件套（微渐变 + `--edge-hi` 顶光 + 点阵 `::after`）、选中 `--sel`、卡片小料、动效——**完整 CSS 见样板对应 selector，逐段复制**。样板里 selector 已用 `body.craft` 门控，可直接套用到 `示意.html` 同名 class（`.card/.tcard/.asset/.tbtn/.lnch/.chip/.tag/.seg` 等命名两边一致，迁移成本低）。

---

## Task 0：搭脚手架（craft CSS + 全局开关）

**Files:** Modify `docs/新设计风格示意.html`

**Step 1** — 备份当前文件（防回退）：
```bash
git add -A && git commit -m "chore: 铺开前快照 新设计风格示意.html"
```

**Step 2** — 把上面「Craft 层样式」四个 `--dot-*`/`--edge-*`/`--noise` token 追加进 `:root`（~16–53）。

**Step 3** — 从样板 `<style>` 复制全部 `body.craft …` 规则块（卡片表面三件套、`--sel` 选中、书签角、小料、动效、按钮、侧边栏、chip、输入、点阵 `.cbody::after`），粘进 `示意.html` 的 `<style>` 末尾。命名对齐：样板用 `.card/.cbody/.pcode/.seg/.tag/.kbtn/.kchip/.mlnch`，`示意.html` 用 `.card/.pcard/.tcard/.seg/.tag/.tbtn/.chip/.lnch`——**逐一核对 selector 名，必要时改名匹配**（这是本任务最易错处）。

**Step 4** — 顶栏（titlebar 或新增一条）加全局开关，复制样板的 `.toggle` 两个按钮 + JS：
```html
<div class="toggle"><button id="btnPlain">原版</button><button id="btnCraft" class="on">精装</button></div>
```
```js
const bP=document.getElementById('btnPlain'),bC=document.getElementById('btnCraft');
bP.onclick=()=>{document.body.classList.remove('craft');bP.classList.add('on');bC.classList.remove('on')};
bC.onclick=()=>{document.body.classList.add('craft');bC.classList.add('on');bP.classList.remove('on')};
```
默认 `<body class="craft">`。

**Step 5（验证）** — 浏览器打开，拨开关：精装态全局底色/边框有顶光与微渐变即通过；切原版应回到现状扁平。

**Step 6** — `git commit -m "feat(prototype): 接入 craft 质感层 + 全局原版⇄精装开关"`

---

## Task 1：共享外壳（侧边栏 + 顶栏按钮 + 更多菜单）

> 外壳在四视图都可见，先做收益最大。对照样板「组件样板」区。

**Files:** Modify `docs/新设计风格示意.html`（`.nav/.lnch`、`.tbtn/.more-btn/.cut-frame`、`.more-menu`）

**Step 1** — 侧边栏启动台 `.lnch .tile`：套样板 `.mlnch .tile` 的 craft（微渐变 + `--edge-hi/lo` + 噪点 `::after` + hover 抬起 `-2px`）。三类色分保持（应用蓝/文件夹黄/网页青）。

**Step 2** — 顶栏工具按钮 `.tbtn`、`.more-btn`：套样板 `.kbtn` craft（描边 + 微渐变 + 顶光 + hover 抬起 `-1px`）。`.cut-frame` 主操作切角保留。

**Step 3** — `.more-menu` 浮层：加 `--edge-hi`，项分隔用蚀刻弱线。

**Step 4（验证）** — 拨精装：侧边栏图标块金属化、hover 抬起；工具按钮有顶光、hover 微浮；切原版还原。四视图切换时外壳一致。

**Step 5** — `git commit -m "feat(prototype): 共享外壳精装（侧边栏/按钮/菜单）"`

---

## Task 2：主页视图 view-home（项目卡片）

> 直接移植样板已定稿的项目卡，最省事。

**Files:** Modify `docs/新设计风格示意.html`（`#view-home` ~558–618，`.card/.pcard` CSS）

**Step 1** — 卡片表面三件套（微渐变 + 顶光/底暗 + 点阵 `.cbody::after`）。若 `示意.html` 卡片无 `.cbody` 内层，需按样板把卡片内容包一层 `.cbody{position:relative;z-index:1}` 承载点阵与小料。

**Step 2** — 选中 `--sel`：按状态给 `.card.st-*{--sel:…}`，选中框/顶线/书签角统一引用（§14.3）。书签角 27px、缩放弹入动画；**书签角不挂 craft-only**（坑1）。

**Step 3** — 小料：右上 `N°编号` + 登记十字、四角铆钉、未开始卡亮线斜纹（坑2）。

**Step 4** — 排版重音（名 17px / 百分比 22px hero、meta 仪表读数块）+ 进度条刻度短线。

**Step 5** — hover 抬起 `-2px` + 左色带 `brightness(1.4)`（坑3：勿碰 box-shadow）。

**Step 6（验证）** — 点不同状态卡：选中色随状态、书签角弹入、hover 浮起点亮；切原版还原扁平。

**Step 7** — `git commit -m "feat(prototype): 主页项目卡精装"`

---

## Task 3：项目视图 view-tasks（筛选条 + 任务卡）

**Files:** Modify `docs/新设计风格示意.html`（`#view-tasks` ~620–687，`.chip` ~331–346，`.tcard`）

**Step 1** — 筛选 chip `.chip`：下划线改"从左 `scaleX(0→1)` 生长"（套样板 `.kchip::after`），选中色按状态（all=蓝）。

**Step 2** — 任务卡 `.tcard`：套 Task 2 同款卡片 craft（表面三件套 + `--sel` 选中 + 小料 + 排版重音 + hover）。任务卡 hero = 任务名；进度条一格一子任务（保留 §13.6）。

**Step 3** — 标签 `.tag`：状态标签彩色、类型标签中性灰（坑/§12.9）。

**Step 4（验证）** — 切筛选项看下划线生长；任务卡选中/hover 同主页一致；类型标签是灰的。

**Step 5** — `git commit -m "feat(prototype): 任务看板精装（筛选条+任务卡）"`

---

## Task 4：任务视图 view-assets（素材卡 + Inspector）

**Files:** Modify `docs/新设计风格示意.html`（`#view-assets` ~689–814）

**Step 1** — 素材卡 `.asset`：缩略图井 `--bg-thumb` 保留；卡身加顶光 + 噪点。**选中仍为蓝**（§14.3 末行，坑6），书签角 craft 化（缩放弹入），hover 抬起。

**Step 2** — Inspector `.inspector`：面板加 `--edge-hi`；`.ins-head`/`.props .pr` 行用蚀刻弱线；进度链条 `.chain` 节点过站绿/当前蓝/未到灰保留；`.ins-actions` 按钮套 `.kbtn` craft。

**Step 3** — Inspector 属性表 `.props`：等宽值右对齐保留，行高固定、分割线极弱（蚀刻）。

**Step 4（验证）** — 点素材：选中蓝框 + 书签角弹入、Inspector 同步；面板有顶光、行分割是内凹弱线；动作按钮 hover 微浮。

**Step 5** — `git commit -m "feat(prototype): 素材页+Inspector精装"`

---

## Task 5：设计规范视图 view-style

**Files:** Modify `docs/新设计风格示意.html`（`#view-style` ~816–918）

**Step 1** — `.panel` 面板加 craft（顶光 + 微渐变）；色板/字体/按钮/标签/进度示例沿用前面任务的成品 class。

**Step 2** — **更新本页"选中态语法"表**：把"卡片=蓝边框"改为"卡片=状态色 `--sel`（框/顶线/书签角）"，书签角 17→27px，与 §14.3 一致。

**Step 3** — 色板若是 `:root` 实时读取（样板做法），确认新增 `--edge-*/--dot-*` 是否需要展示一组"质感原料"示例块（可选）。

**Step 4（验证）** — 规范页与实际界面同源、无自相矛盾；选中语法表已是"状态色"版。

**Step 5** — `git commit -m "feat(prototype): 设计规范页精装 + 选中语法表订正"`

---

## Task 6：点阵控制台（可选）+ 全局验收

**Step 1（可选）** — 若希望规范页也能调点阵，移植样板的 `.console` 控制台与四旋钮 + 两开关 JS。否则跳过（token 已是定稿值）。

**Step 2（全局验收清单）** — 逐项核对：
- [ ] 四视图拨 `原版⇄精装` 均正确切换、无残留蓝/灰半吊子态
- [ ] 选中=状态色在项目卡/任务卡生效；素材卡选中=蓝
- [ ] 书签角仅选中出现、27px、弹入动画；未选中无角（坑1）
- [ ] 未开始卡斜纹可见（亮线，坑2）
- [ ] hover 抬起不丢选中顶线（坑3）
- [ ] 类型标签中性灰、状态标签彩色
- [ ] 全程无 backdrop-filter / 无大阴影模糊 / 无大圆角
- [ ] 外壳（侧边栏/按钮/菜单）四视图一致

**Step 3** — 回填文档：铺开中若出现样板未覆盖的新决策（如 Inspector 蚀刻线规格），补进 `docs/新设计风格.md` §14。

**Step 4** — `git commit -m "feat(prototype): 全界面精装铺开收尾 + 文档回填"`

---

## 完成定义（DoD）

四视图全精装、全局开关可对比、六条踩坑全规避、规范页与实现同源、文档无残留旧语法。交付后下一步才是**迁移真实 Vue/DesignSystem**（另立计划，§14.8 已列迁移优先级：先换变量层、组件结构基本不动）。
