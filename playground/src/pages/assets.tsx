import { useEffect, useState, useCallback, useRef } from "react"
import { api, type Asset, type AnnotationJob, type AnnotateResult, type AssetTag } from "@/lib/api"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Textarea } from "@/components/ui/textarea"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

// Meili 列表命中只带被索引的字段，created_at 可能缺失（T6′ C2-6）；
// 缺失或非法时返回占位符，避免把 "Invalid Date" 渲染给用户。
function formatDate(iso: string | undefined): string {
  if (!iso) return "—"
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return "—"
  return d.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  })
}

function formatDay(iso: string | undefined): string {
  if (!iso) return "—"
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? "—" : d.toLocaleDateString("zh-CN")
}

// 后端 GET /api/v1/assets/{id} 只接受 UUID；列表里可能混入非 UUID 的 id
// （压测语料 bench-*），回源前必须先判定形态。
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

const STATUS_COLORS: Record<AnnotationJob["status"], "default" | "secondary" | "outline" | "destructive"> = {
  pending: "secondary",
  processing: "outline",
  completed: "default",
  failed: "destructive",
}

export function AssetLibraryPage() {
  const [assets, setAssets] = useState<Asset[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [q, setQ] = useState("")
  const [resourceType, setResourceType] = useState<string>("all")
  // C2-7（T6′ 审计）：MIME 类型过滤 UI——后端为精确匹配（mime_type = "type/subtype"）。
  const [mimeType, setMimeType] = useState<string>("all")
  // C2-9（T6′ 审计）：排序 UI——后端白名单 name/size_bytes × asc/desc。
  const [sort, setSort] = useState<string>("")
  const [viewMode, setViewMode] = useState<"grid" | "table">("grid")
  const [offset, setOffset] = useState(0)
  const limit = 24

  const fetchAssets = useCallback(
    (searchQ: string, filterType: string, mime: string, sortParam: string, searchOffset: number) => {
      setLoading(true)
      api
        .listAssets({
          q: searchQ,
          resourceType: filterType === "all" ? "" : filterType,
          mimeType: mime === "all" ? "" : mime,
          sort: sortParam,
          limit,
          offset: searchOffset,
        })
        .then((r) => {
          // 后端两种路径都返回 results（T6′ C2-1：此前读 r.assets 导致列表恒为空）
          setAssets(r.results ?? [])
          setTotal(r.total ?? 0)
        })
        .catch(console.error)
        .finally(() => setLoading(false))
    },
    []
  )

  useEffect(() => {
    const timer = setTimeout(() => {
      fetchAssets(q, resourceType, mimeType, sort, offset)
    }, 150)
    return () => clearTimeout(timer)
  }, [q, resourceType, mimeType, sort, offset, fetchAssets])

  const handleSearch = (val: string) => {
    setQ(val)
    setOffset(0)
  }

  const handleFilterChange = (type: string) => {
    setResourceType(type)
    setOffset(0)
  }

  // 真实文件上传（multipart）：后端做扩展名白名单、大小上限、SHA256 去重与内容寻址落盘。
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [uploading, setUploading] = useState(false)
  const [uploadNotice, setUploadNotice] = useState<string | null>(null)
  const [uploadError, setUploadError] = useState<string | null>(null)

  const handleFileSelected = async (file: File | undefined) => {
    if (!file) return
    setUploading(true)
    setUploadError(null)
    setUploadNotice(null)
    try {
      const res = await api.uploadAsset(file)
      if (res.error) {
        setUploadError(res.error)
        return
      }
      const name = res.asset?.name ?? file.name
      setUploadNotice(
        res.existing
          ? `已存在相同内容（SHA256 去重命中）：${name}`
          : `已上传：${name}${res.warning ? "（检索索引更新失败，稍后重试）" : ""}`
      )
      setQ("")
      setOffset(0)
      fetchAssets("", "all", "all", "", 0)
    } catch (e) {
      setUploadError(e instanceof Error ? e.message : "上传失败")
    } finally {
      setUploading(false)
      if (fileInputRef.current) fileInputRef.current.value = ""
    }
  }

  // Detail modal
  const [selectedAsset, setSelectedAsset] = useState<Asset | null>(null)
  const [prompt, setPrompt] = useState("")
  const [activeJob, setActiveJob] = useState<AnnotationJob | null>(null)
  const [pollingJobId, setPollingJobId] = useState<string | null>(null)
  // 资产标签（human + ai）与预览
  const [assetTags, setAssetTags] = useState<AssetTag[]>([])
  const [tagInput, setTagInput] = useState("")
  const [confirmedJobId, setConfirmedJobId] = useState<string | null>(null)

  const refreshTags = useCallback((assetId: string) => {
    api
      .listAssetTags(assetId)
      .then((r) => setAssetTags(r.tags ?? []))
      .catch(console.error)
  }, [])

  const handleAddTag = async () => {
    if (!selectedAsset || !tagInput.trim()) return
    try {
      const r = await api.tagAsset(selectedAsset.id, { name: tagInput.trim() })
      if (r.tags) setAssetTags(r.tags)
      setTagInput("")
    } catch (e) {
      console.error("tag asset failed", e)
    }
  }

  const handleRemoveTag = async (tagId: string) => {
    if (!selectedAsset) return
    try {
      await api.untagAsset(selectedAsset.id, tagId)
      setAssetTags((cur) => cur.filter((t) => t.tag_id !== tagId))
    } catch (e) {
      console.error("untag failed", e)
    }
  }

  // 人工确认 AI 建议并写入资产标注（MCD 闭环最后一步）
  const handleConfirmSuggestions = async () => {
    if (!activeJob || !selectedAsset) return
    const tags = (activeJob.result as AnnotateResult)?.tags
    if (!tags?.length) return
    try {
      const r = await api.confirmSuggestions(activeJob.id, tags)
      setConfirmedJobId(activeJob.id)
      setAssetTags(r.tags ?? [])
      refreshTags(selectedAsset.id)
    } catch (e) {
      console.error("confirm suggestions failed", e)
    }
  }

  const openAsset = (asset: Asset) => {
    // 先用列表项占位，避免弹窗空窗
    setSelectedAsset(asset)
    setPrompt("")
    setActiveJob(null)
    setPollingJobId(null)
    setAssetTags([])
    setTagInput("")
    setConfirmedJobId(null)
    // 列表命中（尤其 Meili 路径）字段不完整，详情回源权威记录（T6′ C2-6）。
    // 但只对 UUID 形态的 id 回源：压测语料用 bench-* 之类非 UUID id，且不存在于 PG，
    // 回源会得到 400 并把弹窗置空（反而比不回源更糟）。
    if (!UUID_RE.test(asset.id)) return
    api
      .getAsset(asset.id)
      .then((res) => {
        // 仅在仍是同一个资产且确实取到记录时才覆盖，否则保留列表项占位
        setSelectedAsset((cur) => (cur && cur.id === asset.id && res.asset ? res.asset : cur))
      })
      .catch(console.error)
    refreshTags(asset.id)
  }

  const handleAnnotate = async () => {
    if (!selectedAsset || !prompt.trim()) return
    try {
      const res = (await api.annotateAsset(selectedAsset.id, prompt)) as { job: AnnotationJob }
      setActiveJob(res.job)
      setPollingJobId(res.job.id)
    } catch (e) {
      console.error("annotate failed", e)
    }
  }

  // Poll job status
  useEffect(() => {
    if (!pollingJobId) return
    const poll = async () => {
      try {
        const res = (await api.getJob(pollingJobId)) as { job: AnnotationJob }
        const job = res.job
        setActiveJob(job)
        if (job.status !== "pending" && job.status !== "processing") {
          setPollingJobId(null)
        }
      } catch (e) {
        console.error("poll job failed", e)
      }
    }
    const id = setInterval(poll, 2000)
    return () => clearInterval(id)
  }, [pollingJobId])

  const prevDisabled = offset === 0
  const nextDisabled = offset + limit >= total

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center gap-4">
        <h1 className="text-2xl font-semibold">资产管理</h1>
        <span className="text-sm text-muted-foreground">共 {total} 个资产</span>
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          aria-label="选择要上传的文件"
          accept=".png,.jpg,.jpeg,.gif,.webp,.pdf,.txt,.md,.csv,.json"
          onChange={(e) => void handleFileSelected(e.target.files?.[0])}
        />
        <Button
          className="ml-auto"
          disabled={uploading}
          onClick={() => fileInputRef.current?.click()}
        >
          {uploading ? "上传中…" : "上传文件"}
        </Button>
      </div>
      {uploadNotice && <p className="text-sm text-muted-foreground">{uploadNotice}</p>}
      {uploadError && <p className="text-sm text-destructive">{uploadError}</p>}

      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="flex flex-wrap items-center gap-2">
          <Input
            placeholder="搜索资产名称、标签或描述..."
            defaultValue={q}
            onChange={(e) => handleSearch(e.target.value)}
            className="w-72"
          />
          <div className="flex items-center gap-1 rounded-md border p-1 bg-muted/20">
            {[
              { label: "全部", value: "all" },
              { label: "图片", value: "image" },
              { label: "文档", value: "document" },
              { label: "视频", value: "video" },
            ].map((t) => (
              <Button
                key={t.value}
                size="sm"
                variant={resourceType === t.value ? "secondary" : "ghost"}
                className="h-8 text-xs font-normal"
                onClick={() => handleFilterChange(t.value)}
              >
                {t.label}
              </Button>
            ))}
          </div>
          {/* C2-7：MIME 类型精确过滤（与后端 mime_type = "type/subtype" 语义一致） */}
          <select
            aria-label="按 MIME 类型过滤"
            value={mimeType}
            onChange={(e) => {
              setMimeType(e.target.value)
              setOffset(0)
            }}
            className="h-9 rounded-md border bg-transparent px-2 text-xs text-muted-foreground"
          >
            <option value="all">MIME：全部</option>
            {[
              "image/png",
              "image/jpeg",
              "image/gif",
              "image/webp",
              "application/pdf",
              "text/plain",
              "text/markdown",
              "text/csv",
              "application/json",
            ].map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
          {/* C2-9：排序（后端白名单 name/size_bytes × asc/desc；空串 = 默认相关度） */}
          <select
            aria-label="排序方式"
            value={sort}
            onChange={(e) => {
              setSort(e.target.value)
              setOffset(0)
            }}
            className="h-9 rounded-md border bg-transparent px-2 text-xs text-muted-foreground"
          >
            <option value="">排序：默认</option>
            <option value="name:asc">名称 A–Z</option>
            <option value="name:desc">名称 Z–A</option>
            <option value="size_bytes:asc">大小从小到大</option>
            <option value="size_bytes:desc">大小从大到小</option>
          </select>
        </div>

        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant={viewMode === "grid" ? "secondary" : "outline"}
            className="h-8 text-xs"
            onClick={() => setViewMode("grid")}
          >
            卡片网格
          </Button>
          <Button
            size="sm"
            variant={viewMode === "table" ? "secondary" : "outline"}
            className="h-8 text-xs"
            onClick={() => setViewMode("table")}
          >
            表格
          </Button>
        </div>
      </div>

      {loading ? (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {Array.from({ length: 12 }).map((_, i) => (
            <Skeleton key={i} className="h-36 w-full rounded-lg" />
          ))}
        </div>
      ) : assets.length === 0 ? (
        <p className="text-muted-foreground py-8 text-center">暂无符合条件的资产</p>
      ) : viewMode === "grid" ? (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {assets.map((asset) => (
            <div
              key={asset.id}
              onClick={() => openAsset(asset)}
              className="group flex flex-col justify-between overflow-hidden rounded-lg border bg-card p-3 shadow-sm transition hover:border-primary hover:shadow-md cursor-pointer"
            >
              <div className="flex flex-col gap-1.5">
                <div className="flex items-center justify-between">
                  <Badge variant="outline" className="text-[10px] uppercase">
                    {asset.resource_type || "file"}
                  </Badge>
                  <span className="text-[10px] text-muted-foreground">
                    {formatBytes(asset.size_bytes)}
                  </span>
                </div>
                <h3 className="line-clamp-2 text-sm font-medium leading-tight group-hover:text-primary">
                  {asset.name}
                </h3>
              </div>
              <div className="mt-3 flex items-center justify-between border-t pt-2 text-[11px] text-muted-foreground">
                <span className="truncate max-w-[90px]">{asset.mime_type}</span>
                <span>{formatDay(asset.created_at)}</span>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>名称</TableHead>
                <TableHead>类型</TableHead>
                <TableHead>MIME</TableHead>
                <TableHead>大小</TableHead>
                <TableHead>创建时间</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {assets.map((asset) => (
                <TableRow
                  key={asset.id}
                  className="cursor-pointer hover:bg-muted/50"
                  onClick={() => openAsset(asset)}
                >
                  <TableCell className="font-medium">{asset.name}</TableCell>
                  <TableCell>
                    <Badge variant="secondary" className="capitalize">
                      {asset.resource_type}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <Badge variant="outline">{asset.mime_type}</Badge>
                  </TableCell>
                  <TableCell>{formatBytes(asset.size_bytes)}</TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatDate(asset.created_at)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => setOffset((o) => Math.max(0, o - limit))}
          disabled={prevDisabled}
        >
          上一页
        </Button>
        <span className="text-sm text-muted-foreground">
          {offset + 1}–{Math.min(offset + limit, total)} / {total}
        </span>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setOffset((o) => o + limit)}
          disabled={nextDisabled}
        >
          下一页
        </Button>
      </div>

      <Dialog open={!!selectedAsset} onOpenChange={(v) => !v && setSelectedAsset(null)}>
        <DialogContent className="max-h-[85vh] max-w-lg overflow-y-auto">
          <DialogHeader>
            <DialogTitle>资产详情</DialogTitle>
          </DialogHeader>
          {selectedAsset && (
            <div className="flex flex-col gap-4">
              {/* 预览：图片资产直接内联预览（真实文件读取端点） */}
              {selectedAsset.mime_type.startsWith("image/") && UUID_RE.test(selectedAsset.id) && (
                <div className="overflow-hidden rounded-md border bg-muted/30">
                  <img
                    src={api.previewUrl(selectedAsset.id)}
                    alt={selectedAsset.name}
                    className="max-h-64 w-full object-contain"
                    onError={(e) => {
                      ;(e.currentTarget.parentElement as HTMLElement).style.display = "none"
                    }}
                  />
                </div>
              )}
              <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
                <div>
                  <dt className="text-muted-foreground">ID</dt>
                  <dd className="font-mono text-xs break-all">{selectedAsset.id}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">名称</dt>
                  <dd>{selectedAsset.name}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">路径</dt>
                  <dd>{selectedAsset.path}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">MIME</dt>
                  <dd>{selectedAsset.mime_type}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">类型</dt>
                  <dd>{selectedAsset.resource_type}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">大小</dt>
                  <dd>{formatBytes(selectedAsset.size_bytes)}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">SHA256</dt>
                  <dd className="font-mono text-xs break-all">{selectedAsset.sha256}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground">创建时间</dt>
                  <dd>{formatDate(selectedAsset.created_at)}</dd>
                </div>
              </dl>

              {/* 标签管理：human 标签增删 + AI 确认结果展示（MCD「人工分类/文本标签」） */}
              {UUID_RE.test(selectedAsset.id) && (
                <div className="border-t pt-4">
                  <h3 className="mb-2 font-medium">标签</h3>
                  <div className="mb-2 flex flex-wrap gap-1">
                    {assetTags.length === 0 && (
                      <span className="text-xs text-muted-foreground">暂无标签</span>
                    )}
                    {assetTags.map((t) => (
                      <Badge
                        key={t.tag_id}
                        variant={t.source === "ai" ? "secondary" : "outline"}
                        className="gap-1"
                      >
                        <span className="h-1.5 w-1.5 rounded-full" style={{ background: t.color }} />
                        {t.name}
                        {t.source === "ai" && (
                          <span className="text-[10px] opacity-60">AI {(t.confidence * 100).toFixed(0)}%</span>
                        )}
                        <button
                          aria-label={`移除标签 ${t.name}`}
                          className="ml-0.5 opacity-50 hover:opacity-100"
                          onClick={() => void handleRemoveTag(t.tag_id)}
                        >
                          ×
                        </button>
                      </Badge>
                    ))}
                  </div>
                  <div className="flex gap-2">
                    <Input
                      placeholder="添加标签..."
                      value={tagInput}
                      onChange={(e) => setTagInput(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && void handleAddTag()}
                      className="h-8 text-xs"
                    />
                    <Button size="sm" className="h-8" disabled={!tagInput.trim()} onClick={() => void handleAddTag()}>
                      添加
                    </Button>
                  </div>
                </div>
              )}

              <div className="border-t pt-4">
                <h3 className="mb-2 font-medium">提交标注任务</h3>
                <Textarea
                  placeholder="输入标注提示词，例如：classify this image"
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                  rows={3}
                  className="mb-3"
                />
                <div className="flex items-center gap-2">
                  <Button onClick={handleAnnotate} disabled={!prompt.trim() || !!pollingJobId}>
                    {pollingJobId ? "标注中..." : "提交标注"}
                  </Button>
                  {activeJob && (
                    <Badge variant={STATUS_COLORS[activeJob.status]}>
                      {activeJob.status}
                    </Badge>
                  )}
                </div>

                {activeJob && activeJob.status === "completed" && (
                  <div className="mt-4 rounded-md bg-muted p-3">
                    <h4 className="mb-2 text-sm font-medium">标注结果</h4>
                    {(activeJob.result as AnnotateResult)?.tags && (
                      <div className="mb-2 flex flex-wrap gap-1">
                        {(activeJob.result as AnnotateResult).tags.map((t) => (
                          <Badge key={t.name} variant="secondary">
                            {t.name} {(t.confidence * 100).toFixed(0)}%
                          </Badge>
                        ))}
                      </div>
                    )}
                    {(activeJob.result as AnnotateResult)?.description && (
                      <p className="text-sm text-muted-foreground">
                        {(activeJob.result as AnnotateResult).description}
                      </p>
                    )}
                    {(activeJob.result as AnnotateResult)?.model && (
                      <p className="mt-1 text-xs text-muted-foreground">
                        模型：{(activeJob.result as AnnotateResult).model}
                      </p>
                    )}
                    {/* 人工确认：把 AI 建议写入资产标注（asset_tags, source='ai'） */}
                    {confirmedJobId === activeJob.id ? (
                      <p className="mt-2 text-xs text-primary">✓ 已确认并写入资产标签</p>
                    ) : (
                      <Button size="sm" className="mt-2" onClick={() => void handleConfirmSuggestions()}>
                        确认并写入标签
                      </Button>
                    )}
                  </div>
                )}

                {activeJob && activeJob.status === "failed" && (
                  <p className="mt-2 text-sm text-destructive">
                    标注失败：{activeJob.error_message ?? "未知错误"}
                  </p>
                )}
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}
