# PartiSync Playground — 页面条理性重构

## 阶段
- 当前: **Phase 1 MCD 已完成并 L3 验收通过**（可交付）
- 上一阶段已交付: sidebar-03 重构、Code 面板源码构建时提取 + dedent、复制兜底
- 下一阶段: Phase 2（See also 互链 + 按组件独立页 + api-reference 分组）

## 用户真实目标（我理解的）
1. **页面条理性差** — 当前 275 个 demo 在 5 个 showcase 文件里只用「Section 标题 + 描述」两层；官方文档页是「H1(组件名) → H2(分类,如 Installation/Usage/Examples/API Reference) → H3(单个 demo,如 default/icon/outline)」三级，加上左侧 TOC、右上 On This Page、右下 "Edit this page" 锚点链接、底部 Pagination。playground 没有这套结构。
2. **页面臃肿** — basics.tsx 1308 行、navigation 1742 行、overlays 1504 行、forms 1082 行、data 1218 行（5 个文件合计 6852 行），全是平铺的 `<Demo>`，没有任何分组、折叠、锚点、目录；页面没有粘性 TOC，定位靠滚轮。
3. **多组件结合的互链机制** — 用户明确举例"多个组件的结合使用考虑建立相互链接机制"。官方页底部的 **See also**（同页 demo 互链） + **Pagination**（上一/下一组件） + **Anchor 跳转**（On This Page + URL hash） 三件套缺失。

## 现实证据
- App.tsx: `flex flex-1 flex-col gap-4 p-4` 单层容器，无 TOC、无 anchor、无 sticky。
- shared.tsx Section: 只有 `<h2>` 一级，下面直接 `<Demo>`（标题 = `<span>`）。
- shared.tsx Demo: `<div data-slot="example">` 内 `text-xs` 的 demo 标题，没有 anchor 跳转、没有"See also"。
- basics.tsx 1308 行 / 76 demo，forms.tsx 1082 行 / 77 demo —— 缺乏分组容器。
- 官方 ui.shadcn.com/docs/components/base/button 实测：
  - `<h1 class="scroll-m-24 text-3xl font-semibold tracking-tight sm:text-3xl">Button</h1>`
  - 19 个 H2 锚点: `#installation #usage #examples #button-group #api-reference #button #cursor #default #destructive #ghost #icon #link #outline #rounded #rtl #secondary #size #spinner #with-icon`
  - 说明官方是 **H1(组件) → H2(章节) → H3(demo 分类/单 demo)** 三级结构。

## 范围与红线
- 红线: 不自定义样式；所有布局用 `@/components/ui/*` 官方 API；demo 内容 1:1 官方；源码仍走构建时提取表（不能割裂）。
- 可改: App.tsx 内容容器 + 新增 TOC/OnThisPage/Pagination/Anchor 组件；shared.tsx 增加 demo anchor id；为 5 个 showcase 文件的 Section/Demo 注入 anchor 和分组。
- 不可改: demo 内部 JSX、官方组件 API、showcase 数量。

## 关键技术决策（待用户拍板）
1. **TOC 形态**: 右侧 sticky `<aside>` 列 On This Page（官方形态），vs 左侧 sidebar 子目录折叠（更省空间）。推荐 **右侧 sticky** —— 与官方一致，保留现有 sidebar 不动。
2. **H2 分组依据**: 按官方 H2 章节（Installation / Usage / Examples / API Reference）。但当前 5 个 showcase 文件里 demo 已经按"组件"分组（如 basics.tsx 一个文件 76 个 demo 是 51 个组件的子集），需要在 Section 内再加一个"分类"层。
3. **互链机制**: 三选一/组合
   - (a) Demo 内嵌 "See also" 链接（指向同 Section 其他相关 demo）
   - (b) 页面底部 Pagination（← 上一组件 / 下一组件 →，跨 showcase）
   - (c) URL hash 跳转（On This Page 锚点 + `<a href="#demo-id">` 直接打开）
   - 推荐 **c + b + a**（按官方完整形态）

## 交付物（提议，未授权）
- MCD: 在 playground 内新增 `OnThisPage`（sticky 右侧目录）+ Section/Demo anchor + Pagination（页面底部跨组件导航）；在 5 个 showcase 文件里给 Section 增加 `group="examples"` 等分类锚点、给 Demo 自动生成 anchor id。
- 验证: build 0 错 + 浏览器实测 On This Page 跳转、anchor 高亮、Pagination 工作。

## 用户拍板
- 方案: A（三级 heading + 三件套导航 + 轻量 See also）
- TOC 位置: 右侧 sticky `<aside>`，保留现 sidebar 不动
- Pagination: 做（跨 showcase 组件跳转）
- Section 章节: 细化（按官方 H2: Installation / Usage / Examples / API Reference）

## 落地阶段
- Phase 1 MCD（已交付 ✅，L3 实测通过）:
  - shared.tsx: Demo H3 自动 anchor id + 官方自锚 pattern（kebabCase 保留 Unicode 字母，修中文塌缩）、Section group 分类 + H2 自锚、`<OnThisPage>`（sticky top-[calc(var(--header-height)+1px)] + IntersectionObserver rootMargin "0% 0% -80% 0%"）、`<Pagination>`（Button secondary/sm 链接 + lucide 箭头，PAGE_ORDER 线性前后）
  - App.tsx: SidebarInset 加 `[--header-height:calc(var(--spacing)*16)]`、header 改 sticky top-0 z-40、docs flex 双列（主内容列 + 右侧 OnThisPage，toc 空不渲染）、collectToc DOM 收集器（section→depth2、h3→depth3、group 继承 data-group 稳定分组）
  - 5 个 showcase 文件: 重构为 49 个按组件的 Section（basics13/forms12/overlays8/navigation9/data7），每 Section 带 kebab id + 组件名 title + group（首=installation，其余=examples，38/49）+ 官方逐字 description（48 组件页 curl 实抓）；275 个 Demo 块与改动前逐字节一致（python 校验）
  - PAGE_ORDER: basics → forms → overlays → navigation → data → blocks → dashboard → theme（与现 sidebar 一致）
  - 验证: build 0 错 + playwright L3 实测（OnThisPage sticky/highlight/anchor 跳转/URL hash/Pagination 双向切页/中文 anchor 不塌缩/无重复 id/0 console error）
- Phase 2（待定）: See also 互链（仅官方有互链证据时）+ Section 章节细化到组件级（Button 单独一页）+ api-reference 分组（当前 49 Section 无 api-reference，Field 因"每文件 ≥1 installation"验收保持 installation）
- 验证: build 0 错 + 浏览器实测 anchor 跳转、TOC 高亮当前章节、Pagination 跨页生效、刷新不丢状态
