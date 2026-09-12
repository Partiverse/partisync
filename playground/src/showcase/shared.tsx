import * as React from "react"
import { ReactNode } from "react"
import { ArrowLeftIcon, ArrowRightIcon, CheckIcon, Code2Icon, CopyIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { demoSources } from "../demo-sources.generated"

/** Section 所属 showcase 文件的键（用于 Demo 从构建时源码表查源码） */
export type SectionFileKey =
  | "basics"
  | "forms"
  | "overlays"
  | "navigation"
  | "data"

const FileKeyContext = React.createContext<SectionFileKey | null>(null)

/** Section 所属文档区块（对齐 ui.shadcn.com 的 Installation / Usage / Examples / API Reference） */
export type SectionGroup =
  | "installation"
  | "usage"
  | "examples"
  | "api-reference"

/** 官方 anchor id 规则：小写 + 非字母数字段转 kebab-case；保留 Unicode 字母（CJK 标题不丢字，避免 id 塌缩碰撞）
 *  保证 Demo h3 anchor id 与同页面其他 id（Section / 其他 Demo）不重复：
 *  - Section id 由外部传入（唯一）
 *  - Demo id = kebabCase(title)，同 Section 内的 Demo title 互不相同则无碰撞
 */
export function kebabCase(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "")
}

/** 展示区块：H2 标题（带自锚）+ 描述 + 预览卡片 */
export function Section({
  id,
  title,
  description,
  group,
  fileKey,
  children,
}: {
  id: string
  title: string
  description?: string
  /** 文档区块归类：installation / usage / examples / api-reference */
  group?: SectionGroup
  /** 5 个 showcase 文件必传；其他消费方（blocks/theme）可省略，Demo 走兼容回落 */
  fileKey?: SectionFileKey
  children: ReactNode
}) {
  return (
    <FileKeyContext.Provider value={fileKey ?? null}>
      <section id={id} data-group={group}>
        {/* 官方 typeset 排版：H2 = 1.25em/1.4，段前 = flow×1.4（≈1.75em），标题后 1em；首段不留段前距 */}
        <div className="flex flex-col gap-2 [&:not(:first-child)]:mt-[calc(1.25em*1.4)]">
          {/* 官方 H2 pattern：id + 自锚 <a>，# 悬停显示 */}
          <h2 className="text-[1.25em] font-semibold leading-[1.4]">
            <a className="group no-underline" href={`#${id}`}>
              <span className="underline-offset-4 group-hover:underline">
                {title}
              </span>
              <span
                aria-hidden="true"
                className="ml-2 text-muted-foreground opacity-0 group-hover:opacity-100"
              >
                #
              </span>
            </a>
          </h2>
          {description && (
            <p className="mt-2">{description}</p>
          )}
        </div>
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">{children}</div>
      </section>
    </FileKeyContext.Provider>
  )
}

function legacyCopyToClipboard(value: string): boolean {
  const textArea = document.createElement("textarea")
  textArea.value = value
  textArea.setAttribute("readonly", "")
  textArea.style.position = "fixed"
  textArea.style.opacity = "0"
  textArea.style.pointerEvents = "none"

  document.body.appendChild(textArea)
  textArea.focus()
  textArea.select()
  textArea.setSelectionRange(0, value.length)

  let hasCopied = false
  try {
    hasCopied = document.execCommand("copy")
  } catch {
    hasCopied = false
  }

  document.body.removeChild(textArea)
  return hasCopied
}

/** 复制按钮：官方 CopyButton pattern（outline icon + Copy/Check 2s 切换） */
function CopyButton({
  value,
  className,
  ...props
}: React.ComponentProps<typeof Button> & {
  value: string
}) {
  const [hasCopied, setHasCopied] = React.useState(false)

  React.useEffect(() => {
    if (hasCopied) {
      const timer = setTimeout(() => setHasCopied(false), 2000)
      return () => clearTimeout(timer)
    }
  }, [hasCopied])

  return (
    <Button
      data-slot="copy-button"
      data-copied={hasCopied}
      size="icon"
      variant="outline"
      className={className}
      onClick={async () => {
        let copied = false
        if (navigator.clipboard?.writeText) {
          try {
            await navigator.clipboard.writeText(value)
            copied = true
          } catch {
            copied = legacyCopyToClipboard(value)
          }
        } else {
          copied = legacyCopyToClipboard(value)
        }

        if (copied) {
          setHasCopied(true)
        }
      }}
      {...props}
    >
      <span className="sr-only">Copy</span>
      {hasCopied ? <CheckIcon /> : <CopyIcon />}
    </Button>
  )
}

/** 单个组件演示卡片（官方文档页模式） */
export function Demo({
  title,
  description,
  children,
  className,
  center = false,
  sourceOverride,
  source: legacySource,
  docs = false,
}: {
  title: string
  description?: string
  children: ReactNode
  className?: string
  center?: boolean
  /** 兼容过渡期：context 查表无命中时的回落源码（优先使用构建时提取值） */
  sourceOverride?: string
  /** @deprecated 过渡期别名，等价 sourceOverride；手写快照将由下游任务删除 */
  source?: string
  /** 官方文档卡面：rounded-2xl 预览卡（min-h-72 居中）+ 卡内底部常驻代码块 */
  docs?: boolean
}) {
  const [showCode, setShowCode] = React.useState(false)
  const [codeOpen, setCodeOpen] = React.useState(false)
  const fileKey = React.useContext(FileKeyContext)

  // 官方 pattern：kebab-case 自动 anchor id + 自锚链接（group hover 显示 #）
  const demoId = kebabCase(title)

  // 源码优先来自构建时提取表（"文件基名#标题"），其次 sourceOverride / 旧 source prop
  const source =
    (fileKey ? demoSources[`${fileKey}#${title}`] : undefined) ??
    sourceOverride ??
    legacySource

  // 向后兼容：无 source 时保持旧渲染（title + 预览框）
  if (!source) {
    return (
      <div className="rounded-xl border bg-card text-card-foreground shadow-xs">
        <div className="border-b px-4 py-2.5">
          <span className="font-mono text-xs text-muted-foreground">
            {title}
          </span>
          {description && (
            <p className="mt-1 text-sm text-muted-foreground">{description}</p>
          )}
        </div>
        <div
          className={cn(
            "p-5",
            center && "flex flex-wrap items-center justify-center gap-3",
            className
          )}
        >
          {children}
        </div>
      </div>
    )
  }

  // 官方文档卡面模式：rounded-2xl 卡 = h-72 预览（Copy 悬浮右上，hover 显现）
  //  + 底部代码区（max-h-96 截断 + 渐变遮罩 + View Code 按钮，点击展开完整代码）
  if (docs) {
    return (
      <div data-slot="example" className="flex w-full min-w-0 flex-col">
        {/* 官方卡面无 H3 标题：H2 段落 → 直接预览卡；title 仅作源码查表键 */}
        <div
          data-slot="component-preview"
          className="group relative mt-4 mb-12 flex flex-col overflow-hidden rounded-2xl border bg-card text-card-foreground"
        >
          <div
            data-slot="preview"
            dir="ltr"
            className={cn(
              "relative flex h-72 w-full justify-center p-10",
              center ? "items-center" : "items-start",
              className
            )}
          >
            {children}
            <CopyButton
              value={source}
              className="absolute top-4 right-4 z-10 opacity-0 transition-opacity group-hover:opacity-100"
            />
          </div>
          <div data-slot="code" className="relative overflow-hidden border-t">
            <figure
              data-rehype-pretty-code-figure=""
              className={cn("[&>pre]:max-h-96", codeOpen && "[&>pre]:max-h-none")}
            >
              <pre className="no-scrollbar min-w-0 overflow-auto px-4 py-3.5 text-[12px] font-mono">
                <code>{source}</code>
              </pre>
            </figure>
            {!codeOpen && (
              <div className="absolute inset-0 flex items-center justify-center pb-4">
                <div className="absolute inset-0 bg-gradient-to-t from-card via-card/60 to-transparent" />
                <Button
                  variant="outline"
                  size="sm"
                  className="relative z-10 bg-background shadow-none"
                  onClick={() => setCodeOpen(true)}
                >
                  View Code
                </Button>
              </div>
            )}
          </div>
        </div>
      </div>
    )
  }

  return (
    <div data-slot="example" className="flex w-full min-w-0 flex-col gap-1">
      <div className="flex items-center justify-between gap-2">
        {/* h3 anchor id 由 demoId (kebabCase) 生成，唯一性由 Section id + Demo title 组合保证 */}
        <h3 id={demoId} className="px-1.5 py-2 text-sm font-medium">
          <a className="group no-underline" href={`#${demoId}`}>
            <span>{title}</span>
            <span
              aria-hidden="true"
              className="ml-2 text-muted-foreground opacity-0 group-hover:opacity-100"
            >
              #
            </span>
          </a>
        </h3>
        <div className="flex items-center gap-1">
          <Button
            size="icon"
            variant="outline"
            aria-expanded={showCode}
            onClick={() => setShowCode((v) => !v)}
          >
            <span className="sr-only">Code</span>
            <Code2Icon />
          </Button>
          <CopyButton value={source} />
        </div>
      </div>
      {description && (
        <div className="px-1.5 text-sm text-muted-foreground">{description}</div>
      )}
      <div
        data-slot="example-content"
        className={cn(
          "flex min-w-0 flex-1 flex-col items-start gap-6 rounded-xl border bg-card p-12 text-foreground",
          center && "items-center justify-center",
          className
        )}
      >
        {children}
      </div>
      {showCode && (
        <div className="overflow-hidden rounded-xl border">
          <div className="flex items-center justify-between border-b px-4 py-2">
            <span className="text-xs font-medium text-muted-foreground">
              Code
            </span>
            <CopyButton value={source} />
          </div>
          <pre className="max-h-[480px] overflow-auto bg-muted p-4 text-[12px] font-mono">
            <code>{source}</code>
          </pre>
        </div>
      )}
    </div>
  )
}

/** On This Page 目录条目：Section → depth 2，Demo → depth 3，group 继承所在 Section */
export interface TocEntry {
  /** 锚点 id（不含 #） */
  id: string
  title: string
  depth: number
  /** 所属文档区块（Section 的 data-group） */
  group?: SectionGroup
}

/** 官方 DocsTableOfContents scroll-spy：IntersectionObserver，视口底部 20% 为触发带 */
function useActiveItem(ids: string[]) {
  const [activeId, setActiveId] = React.useState<string | null>(null)

  React.useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id)
          }
        }
      },
      { rootMargin: "0px 0px -65% 0px" }
    )

    for (const id of ids ?? []) {
      const el = document.getElementById(id)
      if (el) {
        observer.observe(el)
      }
    }
    return () => {
      for (const id of ids ?? []) {
        const el = document.getElementById(id)
        if (el) {
          observer.unobserve(el)
        }
      }
    }
  }, [ids])

  return activeId
}

/** 右侧 sticky 目录（官方 "On This Page" pattern，--header-height 全局为 64px / h-16） */
export function OnThisPage({ toc }: { toc: TocEntry[] }) {
  const ids = React.useMemo(() => toc.map((item) => item.id), [toc])
  const activeId = useActiveItem(ids)

  return (
    <div className="sticky top-[calc(var(--header-height)+1px)] z-30 ml-auto hidden h-[90svh] w-(--sidebar-width) flex-col gap-4 overflow-hidden overscroll-none pb-8 xl:flex">
      <div className="flex min-h-0 flex-1 flex-col gap-8 overflow-y-auto px-8">
        <div className="flex flex-col gap-2 p-4 pt-0 text-sm">
          <p className="h-6 text-xs font-medium text-muted-foreground">
            On This Page
          </p>
          {toc.map((item) => (
            <a
              key={item.id}
              href={`#${item.id}`}
              data-active={item.id === activeId}
              data-depth={item.depth}
              data-group={item.group}
              className="text-[0.8rem] text-muted-foreground no-underline transition-colors hover:text-foreground data-[active=true]:font-medium data-[active=true]:text-foreground data-[depth=3]:pl-4 data-[depth=4]:pl-6"
            >
              {item.title}
            </a>
          ))}
        </div>
      </div>
    </div>
  )
}

/** Pagination 上一页/下一页条目（顺序取自侧栏目录） */
export interface PaginationLink {
  href: string
  title: string
  /** 客户端路由拦截锚点跳转（如 playground 的 state 切页） */
  onClick?: React.MouseEventHandler<HTMLAnchorElement>
}

/** 底部跨组件翻页（官方 Pagination pattern：Button secondary/sm 链接 + lucide 箭头） */
export function Pagination({
  previous,
  next,
}: {
  previous?: PaginationLink
  next?: PaginationLink
}) {
  return (
    <div className="hidden h-16 w-full items-center gap-2 px-4 sm:flex sm:px-0">
      {previous && (
        <Button
          variant="secondary"
          size="sm"
          render={
            <a href={previous.href} onClick={previous.onClick} />
          }
        >
          <ArrowLeftIcon />
          {previous.title}
        </Button>
      )}
      {next && (
        <Button
          variant="secondary"
          size="sm"
          className="ml-auto"
          render={<a href={next.href} onClick={next.onClick} />}
        >
          {next.title}
          <ArrowRightIcon />
        </Button>
      )}
    </div>
  )
}
