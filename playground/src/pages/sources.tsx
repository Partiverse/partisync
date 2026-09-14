import { useEffect, useState, useCallback } from "react"
import { api, type DataSource, type ScanJob } from "@/lib/api"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import { Progress } from "@/components/ui/progress"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { toast } from "sonner"

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

function SourceCard({ source, onDelete, onEdit }: {
  source: DataSource
  onDelete: (id: string) => void
  onEdit: (source: DataSource) => void
}) {
  const [scanning, setScanning] = useState(false)
  const [pollingJob, setPollingJob] = useState<ScanJob | null>(null)

  const handleScan = async () => {
    setScanning(true)
    try {
      const r = await api.scanSource(source.id)
      setPollingJob(r.job)
    } catch (e) {
      toast.error("触发扫描失败")
      setScanning(false)
    }
  }

  // 轮询扫描任务状态
  useEffect(() => {
    if (!pollingJob || pollingJob.status === "completed" || pollingJob.status === "failed") {
      if (pollingJob) setScanning(false)
      return
    }
    const timer = setTimeout(async () => {
      try {
        const r = await api.getScanJob(pollingJob.id)
        setPollingJob(r.job)
      } catch {
        // ignore
      }
    }, 1500)
    return () => clearTimeout(timer)
  }, [pollingJob])

  const scanProgress = pollingJob
    ? pollingJob.total_files === 0
      ? 0 // 总数未知（流式扫描中），不显示百分比
      : Math.round((pollingJob.processed_files / pollingJob.total_files) * 100)
    : 0

  return (
    <div className="border rounded-lg p-4 space-y-3">
      <div className="flex items-start justify-between">
        <div>
          <div className="flex items-center gap-2">
            <h3 className="font-medium">{source.name}</h3>
            <Badge variant="outline" className="text-xs">{source.type}</Badge>
          </div>
          <p className="text-sm text-muted-foreground">
            {source.type === "123pan"
              ? source.username ? `手机: ${source.username}` : source.url ?? ""
              : source.url ?? source.remote_path ?? ""}
          </p>
          {source.type === "webdav" && source.remote_path && (
            <p className="text-xs text-muted-foreground mt-1">路径: {source.remote_path}</p>
          )}
        </div>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={handleScan}
            disabled={scanning}
          >
            {scanning ? "扫描中…" : "扫描"}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => onEdit(source)}
          >
            编辑
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => onDelete(source.id)}
          >
            删除
          </Button>
        </div>
      </div>

      {scanning && pollingJob && (
        <div className="space-y-1">
          <div className="flex justify-between text-xs text-muted-foreground">
            <span>
              {pollingJob.status === "running"
                ? pollingJob.total_files === 0
                  ? `已处理 ${pollingJob.processed_files} 个文件（正在扫描…）`
                  : `处理中 ${pollingJob.processed_files}/${pollingJob.total_files}`
                : pollingJob.status}
            </span>
            <span>{scanProgress === 0 ? "" : `${scanProgress}%`}</span>
          </div>
          <Progress value={scanProgress} className="h-1.5" />
        </div>
      )}

      <div className="flex gap-4 text-xs text-muted-foreground">
        <span>资产: {source.asset_count ?? 0}</span>
        {source.last_scan_at && (
          <span>上次扫描: {formatDate(source.last_scan_at)}</span>
        )}
        {source.last_scan_result && (
          <Badge variant="secondary" className="text-xs">
            入库 {source.last_scan_result.imported} / 跳过 {source.last_scan_result.skipped}
          </Badge>
        )}
      </div>
    </div>
  )
}

export function DataSourcesPage() {
  const [sources, setSources] = useState<DataSource[]>([])
  const [loading, setLoading] = useState(true)
  const [showAdd, setShowAdd] = useState(false)
  const [name, setName] = useState("")
  const [sourceType, setSourceType] = useState<"webdav" | "123pan">("webdav")
  const [url, setUrl] = useState("")
  const [username, setUsername] = useState("")
  const [password, setPassword] = useState("")
  const [remotePath, setRemotePath] = useState("/")
  const [saving, setSaving] = useState(false)
  // 编辑相关
  const [showEdit, setShowEdit] = useState(false)
  const [editingId, setEditingId] = useState("")
  const [editName, setEditName] = useState("")
  const [editSourceType, setEditSourceType] = useState<"webdav" | "123pan">("webdav")
  const [editUrl, setEditUrl] = useState("")
  const [editUsername, setEditUsername] = useState("")
  const [editPassword, setEditPassword] = useState("")
  const [editRemotePath, setEditRemotePath] = useState("/")

  const fetchSources = useCallback(async () => {
    try {
      const r = await api.listSources()
      setSources(r.sources ?? [])
    } catch (e) {
      console.error("list sources failed", e)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchSources()
  }, [fetchSources])

  const handleAdd = async () => {
    if (!name) {
      toast.error("显示名称不能为空")
      return
    }
    if (sourceType === "webdav" && !url) {
      toast.error("WebDAV URL 不能为空")
      return
    }
    if (sourceType === "123pan" && !username) {
      toast.error("手机号不能为空")
      return
    }
    setSaving(true)
    try {
      await api.createSource({ name, type: sourceType, url, username, password, remote_path: remotePath })
      toast.success("数据源已创建")
      setShowAdd(false)
      setName("")
      setSourceType("webdav")
      setUrl("")
      setUsername("")
      setPassword("")
      setRemotePath("/")
      fetchSources()
    } catch (e) {
      toast.error("创建失败")
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (id: string) => {
    if (!confirm("确认删除该数据源？资产不会被删除。")) return
    try {
      await api.deleteSource(id)
      toast.success("已删除")
      fetchSources()
    } catch {
      toast.error("删除失败")
    }
  }

  const handleEdit = (source: DataSource) => {
    setEditingId(source.id)
    setEditName(source.name)
    setEditSourceType(source.type === "123pan" ? "123pan" : "webdav")
    setEditUrl(source.url ?? "")
    setEditUsername(source.username ?? "")
    setEditPassword("") // 不回填密码
    setEditRemotePath(source.remote_path ?? "/")
    setShowEdit(true)
  }

  const handleUpdate = async () => {
    if (!editName) {
      toast.error("显示名称不能为空")
      return
    }
    if (editSourceType === "webdav" && !editUrl) {
      toast.error("WebDAV URL 不能为空")
      return
    }
    if (editSourceType === "123pan" && !editUsername) {
      toast.error("手机号不能为空")
      return
    }
    setSaving(true)
    try {
      const body: Record<string, string> = { name: editName, type: editSourceType }
      if (editSourceType === "webdav") {
        body.url = editUrl
        body.remote_path = editRemotePath
      }
      if (editUsername) body.username = editUsername
      if (editPassword) body.password = editPassword
      const res = await api.updateSource(editingId, body)
      toast.success("数据源已更新")
      setShowEdit(false)
      // 用 API 返回的完整数据原地更新列表
      setSources((prev) =>
        prev.map((s) => (s.id === editingId ? res.source : s))
      )
    } catch {
      toast.error("更新失败")
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-6 max-w-2xl">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold">数据源</h2>
          <p className="text-sm text-muted-foreground">配置 WebDAV 或 123pan 数据源，扫描后资产入库（默认仅元数据）</p>
        </div>
        <Button onClick={() => setShowAdd(true)}>添加数据源</Button>
      </div>

      {loading ? (
        <div className="space-y-3">
          {[1, 2].map((i) => (
            <Skeleton key={i} className="h-28 w-full" />
          ))}
        </div>
      ) : sources.length === 0 ? (
        <div className="text-center py-12 text-muted-foreground border rounded-lg">
          暂无数据源。点击「添加数据源」配置一个 WebDAV 服务器。
        </div>
      ) : (
        <div className="space-y-3">
          {sources.map((s) => (
            <SourceCard
              key={s.id}
              source={s}
              onDelete={handleDelete}
              onEdit={handleEdit}
            />
          ))}
        </div>
      )}

      <Dialog open={showAdd} onOpenChange={setShowAdd}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>添加数据源</DialogTitle>
            <DialogDescription>
              配置 WebDAV 或 123pan 账号，扫描后资产入库（SHA256 自动去重，默认仅元数据）。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-2">
              <Label>显示名称</Label>
              <Input
                placeholder="我的 NAS / 123pan"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label>类型</Label>
              <RadioGroup
                value={sourceType}
                onValueChange={(v) => { setSourceType(v as "webdav" | "123pan"); setUrl(""); setRemotePath("/") }}
                className="flex gap-4"
              >
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="webdav" id="type-webdav" />
                  <Label htmlFor="type-webdav" className="font-normal cursor-pointer">WebDAV</Label>
                </div>
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="123pan" id="type-123pan" />
                  <Label htmlFor="type-123pan" className="font-normal cursor-pointer">123pan</Label>
                </div>
              </RadioGroup>
            </div>
            {sourceType === "webdav" ? (
              <>
                <div className="space-y-2">
                  <Label>WebDAV URL</Label>
                  <Input
                    placeholder="https://nas.example.com/webdav/"
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                  />
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label>用户名</Label>
                    <Input
                      placeholder="user"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label>密码</Label>
                    <Input
                      type="password"
                      placeholder="••••••"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <Label>远程路径（留空为根目录）</Label>
                  <Input
                    placeholder="/documents"
                    value={remotePath}
                    onChange={(e) => setRemotePath(e.target.value)}
                  />
                </div>
              </>
            ) : (
              <>
                <div className="space-y-2">
                  <Label>手机号（用户名）</Label>
                  <Input
                    placeholder="13812345678"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label>密码</Label>
                  <Input
                    type="password"
                    placeholder="••••••"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                  />
                </div>
              </>
            )}
            <div className="flex justify-end gap-2 pt-2">
              <Button variant="outline" onClick={() => setShowAdd(false)}>
                取消
              </Button>
              <Button onClick={handleAdd} disabled={saving}>
                {saving ? "创建中…" : "创建"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={showEdit} onOpenChange={setShowEdit}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>编辑数据源</DialogTitle>
            <DialogDescription>修改数据源配置。留空密码表示不修改。</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-2">
            <div className="space-y-2">
              <Label>显示名称</Label>
              <Input
                placeholder="我的 NAS / 123pan"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label>类型</Label>
              <RadioGroup
                value={editSourceType}
                onValueChange={(v) => { setEditSourceType(v as "webdav" | "123pan"); setEditUrl(""); setEditRemotePath("/") }}
                className="flex gap-4"
              >
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="webdav" id="edit-type-webdav" />
                  <Label htmlFor="edit-type-webdav" className="font-normal cursor-pointer">WebDAV</Label>
                </div>
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="123pan" id="edit-type-123pan" />
                  <Label htmlFor="edit-type-123pan" className="font-normal cursor-pointer">123pan</Label>
                </div>
              </RadioGroup>
            </div>
            {editSourceType === "webdav" ? (
              <>
                <div className="space-y-2">
                  <Label>WebDAV URL</Label>
                  <Input
                    placeholder="https://nas.example.com/webdav/"
                    value={editUrl}
                    onChange={(e) => setEditUrl(e.target.value)}
                  />
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label>用户名</Label>
                    <Input
                      placeholder="user"
                      value={editUsername}
                      onChange={(e) => setEditUsername(e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <Label>密码（留空不修改）</Label>
                    <Input
                      type="password"
                      placeholder="••••••"
                      value={editPassword}
                      onChange={(e) => setEditPassword(e.target.value)}
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <Label>远程路径</Label>
                  <Input
                    placeholder="/documents"
                    value={editRemotePath}
                    onChange={(e) => setEditRemotePath(e.target.value)}
                  />
                </div>
              </>
            ) : (
              <>
                <div className="space-y-2">
                  <Label>手机号（用户名）</Label>
                  <Input
                    placeholder="13812345678"
                    value={editUsername}
                    onChange={(e) => setEditUsername(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label>密码（留空不修改）</Label>
                  <Input
                    type="password"
                    placeholder="••••••"
                    value={editPassword}
                    onChange={(e) => setEditPassword(e.target.value)}
                  />
                </div>
              </>
            )}
            <div className="flex justify-end gap-2 pt-2">
              <Button variant="outline" onClick={() => setShowEdit(false)}>
                取消
              </Button>
              <Button onClick={handleUpdate} disabled={saving}>
                {saving ? "更新中…" : "保存"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}
