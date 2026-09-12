# shadcn/ui 缺失组件研究 — Marker / Message / Message Scroller / Native Select / Questionnaire

> 来源：ui.shadcn.com 官方文档（2026-06 "Components for Chat Interfaces" 与 2026-08 Questionnaire 更新）。
> 研究人：investigator-2（任务 t2）。文档抓取自 `https://ui.shadcn.com/docs/components/<name>.md`。

本 playground（components.json `style: "base-nova"`，基于 `@base-ui/react`）当前缺失这 5 个组件。注意：官方对每个组件同时提供 radix 与 base 两种变体；本文以 radix 变体为主参考（文档以 `base: radix` 标注），实现时如需贴合 base-nova 风格可参照 `/docs/components/base/<name>`。

---

## 1. Marker（标记）

- 文档：https://ui.shadcn.com/docs/components/marker （styleName: radix-rhea）
- 描述：在会话中显示内联状态、系统备注、带边框行或带标签分隔符。
- 安装：`npx shadcn@latest add marker`
- 新依赖：**无**（纯组合组件：Slot/`asChild` + cva + lucide 图标；可选配已有 `Spinner` 组件与 `shimmer` 工具类）。

### 组成
```
Marker
├── MarkerIcon（装饰性，aria-hidden）
└── MarkerContent（文本）
```

### API
| 部件 | Prop | 类型 | 默认 | 说明 |
|---|---|---|---|---|
| Marker | `variant` | `"default" \| "border" \| "separator"` | `"default"` | 内联标记 / 带底边框行 / 两侧带分割线的居中标签 |
| Marker | `asChild` | `boolean` | `false` | 渲染为子元素（链接/按钮标记） |
| Marker | `role`、`className` | — | — | 流式/进行中建议 `role="status"`；其余透传 |
| MarkerIcon | `className` | — | — | 装饰性图标槽，`aria-hidden` |
| MarkerContent | `className` | — | — | 文本内容；可加 `shimmer` 类做流式动画 |

另导出 `markerVariants` 供自定义组件复用样式。

### 典型用法
```tsx
<Marker role="status">
  <MarkerIcon><Spinner /></MarkerIcon>
  <MarkerContent className="shimmer">Thinking...</MarkerContent>
</Marker>
<Marker variant="separator">
  <MarkerContent>Conversation compacted</MarkerContent>
</Marker>
<Marker asChild>
  <a href="#"><MarkerIcon><GitBranchIcon /></MarkerIcon>
    <MarkerContent>View the pull request</MarkerContent></a>
</Marker>
```

### 无障碍要点
- 流式/进度标记设 `role="status"`；带文字的分隔符**不要**加 `role="separator"`；`MarkerIcon` 是装饰性的；可交互标记用 `asChild` 渲染真实 `<a>`/`<button>`。
- 与 `Message` 组合用于会话流；与 `Alert`/`Toast` 区别：Marker 是会话内的轻量行内状态行，非浮层、非块级提示。

---

## 2. Message（消息）

- 文档：https://ui.shadcn.com/docs/components/message （styleName: radix-rhea）
- 描述：会话中单条消息的行布局，负责头像、对齐、header、footer。
- 安装：`npx shadcn@latest add message`
- 新依赖：**组件本身无**；但消息表面需要配套 **Bubble** 组件（`npx shadcn add bubble`，同属 chat 组件族），头像用已有 `Avatar`。

### 组成
```
Message
├── MessageAvatar
└── MessageContent
    ├── MessageHeader
    ├── Bubble / BubbleGroup（可见消息表面）
    └── MessageFooter
MessageGroup
└── Message（连续同一发送者）
```

### API
| 部件 | Prop | 类型 | 默认 | 说明 |
|---|---|---|---|---|
| Message | `align` | `"start" \| "end"` | `"start"` | 消息在会话中的对齐（接收者/发送者） |
| Message | `className` | — | — | 行样式 |
| MessageGroup | `className` | — | — | 同一发送者连续消息分组 |
| MessageAvatar | `className` | — | — | 头像槽，锚定行底部；有 footer 时自动上移对齐消息面 |
| MessageContent | `className` | — | — | 包裹 header/表面/footer |
| MessageHeader | `className` | — | — | 消息上方内容（如发送者名），始终 start 对齐 |
| MessageFooter | `className` | — | — | 消息下方内容（状态、copy/retry/点赞等操作），随消息侧对齐 |

### 典型用法
```tsx
<Message align="end">
  <MessageAvatar><Avatar>…</Avatar></MessageAvatar>
  <MessageContent>
    <Bubble><BubbleContent>Deploying to prod.</BubbleContent></Bubble>
    <MessageFooter>Delivered</MessageFooter>
  </MessageContent>
</Message>
```

### 与 Alert / Toast(Sonner) 的区别
- **Message**：会话流中的持久消息行（含头像/对齐/分组/footer），内容放在 `Bubble` 里；是布局层，不承载弹出行为。
- **Alert**：静态块级提示（info/warning/error），无对话语义、无对齐/头像。
- **Toast/Sonner**：临时浮层通知，自动消失，脱离文档流。
- AI 场景中 Message 还可用于渲染推理步骤、工具调用与助手消息；流式状态用 `Marker role="status"` 放进 Message。

---

## 3. Message Scroller（消息滚动）

- 文档：https://ui.shadcn.com/docs/components/message-scroller （styleName: radix-nova）；底层 headless：`@shadcn/react` Message Scroller
- 描述：会话转写（transcript）的滚动容器：自动跟随、滚动锚定、历史消息前置保持、滚动位置状态、跳转。
- 安装：`npx shadcn@latest add message-scroller`
- **新依赖：需要 `npm install @shadcn/react`**（行为逻辑来自该 headless 包；这是唯一要新增的 npm 依赖）。

### 组成（styled 包装）
```
MessageScrollerProvider（headless root，持有滚动状态）
└── MessageScroller（styled frame，填满父容器，需放在限高布局内）
    ├── MessageScrollerViewport（滚动视口，role=region）
    │   └── MessageScrollerContent（role=log，live region）
    │       └── MessageScrollerItem × N（每行一个：消息/Marker/typing/分隔/load-more）
    └── MessageScrollerButton（滚动到 start/end，无可滚方向时惰性移出 tab 序）
```

### 关键 Props（Provider，headless 名 `MessageScroller.Provider`）
| Prop | 类型 | 默认 | 说明 |
|---|---|---|---|
| `autoScroll` | `boolean` | `false` | 仅当读者已在 live edge 时跟随新内容；滚轮/触摸/键盘/显式跳转会解除 |
| `defaultScrollPosition` | `"start" \| "end" \| "last-anchor"` | `"end"` | 首次非空渲染的打开位置；应用前视口带 `data-pending-scroll`（应 `visibility:hidden` 防闪烁） |
| `scrollEdgeThreshold` | `number` | `8` | 距边缘多少像素内仍算“在边上” |
| `scrollMargin` | `number` | `0` | 对齐边的额外边距 |
| `scrollPreviousItemPeek` | `number` | `64` | 新追加锚定行时让上一行保持可见的额外边距 |

- **Item**：`messageId`（稳定 id，用于 scrollToMessage/visibility/前置保持）、`scrollAnchor`（标记 turn 边界行，通常 user 消息）。
- **Viewport**：`preserveScrollOnPrepend`（默认 true）、`role="region"`、`aria-label="Messages"`。
- **状态属性**：`data-scrollable="start"|"end"|"start end"`、`data-autoscrolling`、`data-pending-scroll`（Root 与 Viewport 都镜像）。
- **Hooks**：`useMessageScroller()`（`scrollToMessage/scrollToEnd/scrollToStart`，均返回 boolean）、`useMessageScrollerScrollable()`（start/end 布尔）、`useMessageScrollerVisibility()`（`currentAnchorId`、`visibleMessageIds`）。

### 典型用法
```tsx
<div className="flex h-screen flex-col">
  <MessageScrollerProvider defaultScrollPosition="end" autoScroll>
    <MessageScroller className="flex-1">
      <MessageScrollerViewport>
        <MessageScrollerContent>
          {messages.map(m => (
            <MessageScrollerItem key={m.id} messageId={m.id} scrollAnchor={m.role === "user"}>
              <Message>…</Message>
            </MessageScrollerItem>
          ))}
        </MessageScrollerContent>
      </MessageScrollerViewport>
      <MessageScrollerButton />
    </MessageScroller>
  </MessageScrollerProvider>
</div>
```

### 使用场景
AI 聊天/Agent 会话转写流式滚动：跟随 live edge、跳到某条消息、outline/搜索可见性、prepend 历史消息不跳动、刷新不闪烁。虚拟化在大列表时可选（文档有 Virtualization 章节）。

---

## 4. Native Select（原生下拉）

- 文档：https://ui.shadcn.com/docs/components/native-select （styleName: radix-nova）
- 描述：带设计系统样式的原生 HTML `<select>`。
- 安装：`npx shadcn@latest add native-select`
- 新依赖：**无**（原生元素 + 样式 + ChevronDown 图标）。

### 组成
```
NativeSelect
├── NativeSelectOption（value, disabled）
└── NativeSelectOptGroup（label, disabled）[可选分组]
```

### API
| 部件 | Prop | 类型 | 默认 |
|---|---|---|---|
| NativeSelect | `disabled` | `boolean` | `false` |
| NativeSelect | `aria-invalid` | `boolean` | — （显示校验错误态，可配 `Field` 的 `data-invalid`） |
| NativeSelectOption | `value` / `disabled` | `string` / `boolean` | — / `false` |
| NativeSelectOptGroup | `label` / `disabled` | `string` / `boolean` | — / `false` |

其余原生 `<select>` props 均透传（`dir` 支持 RTL）。

### Native Select vs 现有 Select（官方口径）
- **NativeSelect**：原生浏览器行为、更好性能、移动端优化的下拉（原生 picker、原生键盘/无障碍行为）；自定义能力有限（仅 option 文本，无法自定义选项样式/动画）。
- **Select**（现有 `@radix-ui/react-select`）：完全自定义样式、动画、复杂交互（选项内富内容、搜索等）。
- 建议：表单里简单枚举值、移动端优先场景用 NativeSelect；需要品牌化弹层时用 Select。

```tsx
<NativeSelect aria-invalid="true">
  <NativeSelectOption value="">Select status</NativeSelectOption>
  <NativeSelectOptGroup label="Engineering">
    <NativeSelectOption value="frontend">Frontend</NativeSelectOption>
  </NativeSelectOptGroup>
</NativeSelect>
```

---

## 5. Questionnaire（问卷）

- 文档：https://ui.shadcn.com/docs/components/questionnaire （styleName: radix-nova）；底层 headless：`@shadcn/react` Questionnaire
- 描述：逐题推进的问卷/表单流：有序 items、活动题、作答状态、校验、进度、导航（上一题/跳过/下一题/提交）。
- 安装：`npx shadcn@latest add questionnaire`
- **新依赖：需要 `npm install @shadcn/react`**。
- 页面/卡片/Dialog 负责关闭、持久化、传输、分支；Questionnaire 只管题内逻辑。

### 组成
```
Questionnaire（Root，<form>）
├── QuestionnaireProgress（"Question {current} of {total}"）
├── QuestionnaireItem（name 必填）
│   ├── QuestionnaireTitle（<legend>）
│   ├── QuestionnaireDescription（aria-describedby）
│   ├── QuestionnaireChoices
│   │   ├── QuestionnaireChoice（<label>）→ ChoiceInput(原生 radio/checkbox) + ChoiceLabel + ChoiceShortcut
│   │   └── QuestionnaireInput（自由作答，type=text/email/…）
│   └── QuestionnaireError（校验失败才显示）
└── QuestionnaireActions
    ├── QuestionnairePrevious / Skip / Next / Submit
```

### 关键 API（Root / Item）
| Prop | 类型 | 默认 | 说明 |
|---|---|---|---|
| Root `item` / `defaultItem` / `onItemChange` | `string` | 首个可用题 | 受控/非受控当前题 |
| Root `items` | `QuestionnaireItemDefinition[]` | — | 服务端渲染与题序声明（`{name, required?, disabled?, choices?}`） |
| Root `shortcuts` | `"letters" \| "numbers"` | — | 按定义/渲染顺序给选项分配快捷键 |
| Root `noValidate` | `boolean` | `true` | 关闭原生约束 UI，保留问卷自身校验 |
| Root `onSubmit` / `onReset` | FormHandler | — | 全部题校验通过后提交；preventDefault 阻止重置 |
| Item `name` | `string` | 必填 | 唯一题名 = 原生表单字段名 |
| Item `required` | `boolean` | `false` | 必答且禁用 Skip |
| Item `multiple` | `boolean` | `false` | 固定选项渲染为 checkbox（多选） |
| Item `invalid` | `boolean` | `false` | 外部校验器标记无效 |
| Item `disabled` | `boolean` | `false` | 从进度与导航中剔除 |
| Item `onStatusChange` | `(status) => void` | — | `"unanswered" \| "answered" \| "skipped"` |

- 导航按钮可见性：Previous（非第一题）、Skip（选做题）、Next（非最后题）、Submit（最后题）；均接受 Button 的 `size`/`variant` 与 `render`。
- 支持条件题（conditional items）、受控模式、恢复（Resume）、自定义校验/进度/动画、Card/Dialog 容器形态。
- 数据读取：标准 `new FormData(event.currentTarget)`（`answers.get(name)` / 多选用 `getAll`）。

### 典型用法
```tsx
<Questionnaire items={items} onSubmit={handleSubmit}>
  <QuestionnaireProgress />
  {items.map(q => (
    <QuestionnaireItem key={q.name} name={q.name} required={q.required}>
      <QuestionnaireTitle>{q.prompt}</QuestionnaireTitle>
      <QuestionnaireChoices>
        {q.choices?.map(c => (
          <QuestionnaireChoice key={c.value} value={c.value}>
            <span className="font-medium">{c.label}</span>
          </QuestionnaireChoice>
        ))}
        <QuestionnaireInput placeholder="Type another answer…" />
      </QuestionnaireChoices>
      <QuestionnaireError />
    </QuestionnaireItem>
  ))}
  <QuestionnaireActions>
    <QuestionnairePrevious /><QuestionnaireSkip />
    <QuestionnaireNext /><QuestionnaireSubmit />
  </QuestionnaireActions>
</Questionnaire>
```

### 交互模式
一屏一题（或 Card/Dialog 内逐题）；选项即答即存；字母/数字快捷键选择；Skip 只对选做题可见；全部题校验通过才能 Submit；通过 `items` 可服务端渲染当前题/进度/快捷键。

---

## 依赖汇总（对照 playground 现有 package.json）

| 组件 | 新 npm 依赖 | 组件间依赖 | 备注 |
|---|---|---|---|
| Marker | 无 | 可选：Spinner（已有）、shimmer 工具类 | 纯样式组合 |
| Message | 无 | **Bubble**（需一并安装）、Avatar（已有）、Attachment（已有）、Marker（可选） | 行布局层 |
| Message Scroller | **@shadcn/react** | Message/Marker 作为 Item 内容 | headless 逻辑在 @shadcn/react |
| Native Select | 无 | 可选：Field | 原生 select |
| Questionnaire | **@shadcn/react** | 无 | headless 逻辑在 @shadcn/react |

`shimmer` 工具类随 `shadcn` 包提供（见 /docs/utils/shimmer），Marker 流式文本需要它。
