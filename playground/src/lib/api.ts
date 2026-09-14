// Typed API client for partisync backend

export interface Asset {
  id: string
  name: string
  path: string
  sha256: string
  size_bytes: number
  mime_type: string
  resource_type: string
  metadata: Record<string, unknown>
  source_id?: string
  created_at: string
  updated_at: string
}

export interface AnnotationJob {
  id: string
  asset_id: string
  status: 'pending' | 'processing' | 'completed' | 'failed'
  prompt: string
  result: AnnotateResult | Record<string, never>
  retry_count: number
  error_message?: string
  created_at: string
  updated_at: string
}

export interface AnnotateResult {
  tags: Array<{ name: string; confidence: number }>
  model: string
  asset_id: string
  description: string
  prompt_used: string
}

export interface ListAssetsResponse {
  // 后端 GET /api/v1/assets 统一返回 results（Meili 路径与 PG 路径一致）。
  // 注意：Meili 路径的命中只带索引字段（id/name/sha256/mime_type/resource_type/
  // size_bytes），不含 path/metadata/created_at/updated_at —— 详情视图必须改用
  // getAsset(id) 取全量字段，不能依赖列表项（T6′ C2-6）。
  results: Asset[]
  total: number
  limit: number
  offset: number
}

export interface ListJobsResponse {
  jobs: AnnotationJob[]
  limit: number
}

export interface AssetTag {
  tag_id: string
  name: string
  color: string
  source: 'human' | 'ai'
  confidence: number
  created_at: string
}

export interface Tag {
  id: string
  name: string
  color: string
  created_at: string
}

export interface ListAssetTagsResponse {
  tags: AssetTag[]
}

export interface ListTagsResponse {
  tags: Tag[]
}

export interface ConfirmSuggestionsResponse {
  confirmed: number
  asset_id: string
  tags: AssetTag[]
}

export interface UploadAssetResponse {
  asset?: Asset
  existing?: boolean
  file_deduped?: boolean
  warning?: string
  error?: string
}

export interface ListAssetParams {
  q?: string
  resourceType?: string
  mimeType?: string
  sort?: string
  limit?: number
  offset?: number
  sourceId?: string
}

// DataSource 类型
export interface DataSource {
  id: string
  name: string
  type: string
  url?: string
  username?: string
  remote_path?: string
  last_scan_at?: string
  last_scan_result?: { imported: number; skipped: number; errors?: string[] }
  asset_count?: number
  created_at: string
}

export interface ScanJob {
  id: string
  source_id: string
  status: 'queued' | 'running' | 'completed' | 'failed'
  total_files: number
  processed_files: number
  imported_count: number
  skipped_count: number
  error?: string
  started_at?: string
  finished_at?: string
  created_at: string
}

export interface ListSourcesResponse {
  sources: DataSource[]
}

export const api = {
  listAssets: (params?: string | ListAssetParams, legacyLimit = 20, legacyOffset = 0): Promise<ListAssetsResponse> => {
    let q = ''
    let limit = legacyLimit
    let offset = legacyOffset
    let resourceType = ''
    let mimeType = ''
    let sort = ''
    let sourceId = ''

    if (typeof params === 'string') {
      q = params
    } else if (params) {
      q = params.q ?? ''
      limit = params.limit ?? 20
      offset = params.offset ?? 0
      resourceType = params.resourceType ?? ''
      mimeType = params.mimeType ?? ''
      sort = params.sort ?? ''
      sourceId = params.sourceId ?? ''
    }

    const searchParams = new URLSearchParams()
    if (q) searchParams.set('q', q)
    if (resourceType) searchParams.set('resource_type', resourceType)
    if (mimeType) searchParams.set('mime_type', mimeType)
    if (sort) searchParams.set('sort', sort)
    if (sourceId) searchParams.set('source_id', sourceId)
    searchParams.set('limit', String(limit))
    searchParams.set('offset', String(offset))

    return fetch(`/api/v1/assets?${searchParams.toString()}`).then(
      (r) => r.json() as Promise<ListAssetsResponse>
    )
  },

  getAsset: (id: string): Promise<{ asset: Asset }> =>
    fetch(`/api/v1/assets/${id}`).then((r) => r.json() as Promise<{ asset: Asset }>),

  // uploadAsset 上传真实文件字节（multipart/form-data）；后端做 SHA256 去重与安全落盘。
  // 415/413 的后端错误串偏机器向，这里转成人话（内容与扩展名不符 → 415；超上限 → 413）。
  uploadAsset: (file: File): Promise<UploadAssetResponse> => {
    const form = new FormData()
    form.append('file', file)
    return fetch('/api/v1/assets/upload', { method: 'POST', body: form }).then(async (r) => {
      const body = (await r.json()) as UploadAssetResponse
      if (!r.ok && body.error) {
        if (r.status === 415) {
          body.error = body.error.includes('content does not match')
            ? '文件内容与其扩展名不符（疑似伪装文件），为安全起见已拒绝'
            : '不支持的文件类型（仅允许 png/jpg/jpeg/gif/webp/pdf/txt/md/csv/json）'
        } else if (r.status === 413) {
          body.error = '文件或请求体超过大小上限'
        }
      }
      return body
    })
  },

  createAsset: (body: object): Promise<{ asset: Asset } | { error: string }> =>
    fetch('/api/v1/assets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then((r) => r.json() as Promise<{ asset: Asset } | { error: string }>),

  annotateAsset: (assetId: string, prompt: string): Promise<{ job: AnnotationJob }> =>
    fetch(`/api/v1/assets/${assetId}/annotate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ prompt }),
    }).then((r) => r.json() as Promise<{ job: AnnotationJob }>),

  listJobs: (limit = 20): Promise<ListJobsResponse> =>
    fetch(`/api/v1/jobs?limit=${limit}`).then(
      (r) => r.json() as Promise<ListJobsResponse>
    ),

  getJob: (jobId: string): Promise<{ job: AnnotationJob }> =>
    fetch(`/api/v1/jobs/${jobId}`).then((r) => r.json() as Promise<{ job: AnnotationJob }>),

  // 资产预览：返回原始文件字节（图片可直接作为 <img src>）。
  previewUrl: (assetId: string): string => `/api/v1/assets/${assetId}/preview`,

  listAssetTags: (assetId: string): Promise<ListAssetTagsResponse> =>
    fetch(`/api/v1/assets/${assetId}/tags`).then((r) => r.json() as Promise<ListAssetTagsResponse>),

  tagAsset: (assetId: string, body: { name?: string; tag_id?: string; color?: string; confidence?: number }): Promise<ListAssetTagsResponse> =>
    fetch(`/api/v1/assets/${assetId}/tags`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then((r) => r.json() as Promise<ListAssetTagsResponse>),

  untagAsset: (assetId: string, tagId: string): Promise<{ removed: boolean }> =>
    fetch(`/api/v1/assets/${assetId}/tags/${tagId}`, { method: 'DELETE' }).then(
      (r) => r.json() as Promise<{ removed: boolean }>
    ),

  listTags: (): Promise<ListTagsResponse> =>
    fetch('/api/v1/tags').then((r) => r.json() as Promise<ListTagsResponse>),

  // 人工确认 AI 建议并写入资产标注（asset_tags, source='ai'）。
  confirmSuggestions: (jobId: string, suggestions: Array<{ name: string; confidence: number }>): Promise<ConfirmSuggestionsResponse> =>
    fetch(`/api/v1/jobs/${jobId}/confirm`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ suggestions }),
    }).then((r) => r.json() as Promise<ConfirmSuggestionsResponse>),

  // —— 数据源 API ——
  listSources: (): Promise<ListSourcesResponse> =>
    fetch('/api/v1/sources').then((r) => r.json() as Promise<ListSourcesResponse>),

  createSource: (body: { name: string; type: string; url?: string; username?: string; password?: string; remote_path?: string }): Promise<{ source: DataSource }> =>
    fetch('/api/v1/sources', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then((r) => r.json() as Promise<{ source: DataSource }>),

  deleteSource: (id: string): Promise<void> =>
    fetch(`/api/v1/sources/${id}`, { method: 'DELETE' }).then((r) => {
      if (!r.ok && r.status !== 204) throw new Error('delete failed')
    }),

  updateSource: (id: string, body: { name?: string; type?: string; url?: string; username?: string; password?: string; remote_path?: string }): Promise<{ source: DataSource }> =>
    fetch(`/api/v1/sources/${id}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then((r) => r.json() as Promise<{ source: DataSource }>),

  scanSource: (id: string): Promise<{ job: ScanJob }> =>
    fetch(`/api/v1/sources/${id}/scan`, { method: 'POST' }).then((r) => r.json() as Promise<{ job: ScanJob }>),

  getScanJob: (jobId: string): Promise<{ job: ScanJob }> =>
    fetch(`/api/v1/source-jobs/${jobId}`).then((r) => r.json() as Promise<{ job: ScanJob }>),
}
