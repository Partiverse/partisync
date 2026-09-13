// partisync T7 — L3 浏览器真实入口验收（MCD 完整闭环）
//
// 覆盖 MCD 闭环全链路（经真实浏览器 + 真实后端容器）：
//   启动 → 列表渲染 → 上传文件 → 搜索命中 → 预览 → 人工标签 → 提交慢标注任务
//   → 任务排队/执行 → AI 建议人工确认写入标注
//
// 前置：
//   1) 后端 docker compose up -d（127.0.0.1:8080）；
//   2) Vite dev server: cd playground && npm run dev -- --port 5201 --strictPort
//
// 运行：
//   L3_SHOTS=docs/verification/t7-l3 node scripts/l3-mcd-acceptance.mjs
//
// 退出码：0 = 全部断言通过；1 = 存在失败断言。

import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { execSync } from 'node:child_process'
import fs from 'node:fs'

const PROJ = fileURLToPath(new URL('../playground/', import.meta.url))
const require = createRequire(PROJ + 'package.json')
const { chromium } = require('playwright-core')

const EXEC = process.env.L3_CHROME || '/home/acme/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome'
const BASE = process.env.L3_BASE || 'http://127.0.0.1:5201/'
const SHOTS = process.env.L3_SHOTS || 'docs/verification/t7-l3'
fs.mkdirSync(SHOTS, { recursive: true })

// 准备一张唯一的测试图（内容唯一 → 不命中既有去重库）
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAGklEQVQYV2NkYPj/n4GBgYGRgYGBAQQABQAExgABKQK1NwAAAABJRU5ErkJggg==',
  'base64'
)
const TEST_NAME = `l3-mcd-${Date.now()}.png`
const TEST_FILE = `/tmp/l3-upload-${Date.now()}.png`
fs.writeFileSync(TEST_FILE, PNG)

const browser = await chromium.launch({
  executablePath: EXEC,
  headless: true,
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
})
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })

const consoleErrors = []
const badResponses = []
page.on('console', (m) => { if (m.type() === 'error') consoleErrors.push(m.text()) })
page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + e.message))
page.on('response', async (r) => {
  if (r.status() >= 400) {
    let body = ''
    try { body = (await r.text()).slice(0, 120) } catch {}
    badResponses.push({ status: r.status(), url: r.url(), body })
  }
})

const steps = []
const checks = []
const shot = (name) => page.screenshot({ path: `${SHOTS}/${name}.png` }).then(() => `${SHOTS}/${name}.png`)
const check = (name, ok, detail) => checks.push({ name, ok, detail })
const cards = () => page.locator('div.cursor-pointer').count()

// ── 1. 冷启动 → 列表渲染 ────────────────────────────────────────────
await page.goto(BASE, { waitUntil: 'networkidle' })
await page.waitForSelector('h1:has-text("资产管理")', { timeout: 20000 })
await page.waitForTimeout(900)
// 空库兼容（P2 残留清理后）：cards=0 且 API total=0 视为「空库正常渲染」；
// 只有「库里有资产却渲染不出」才是失败。
const totalAtStart = await page.evaluate(async () => {
  const r = await fetch('/api/v1/assets?limit=1')
  const j = await r.json()
  return j.total ?? (j.results ? j.results.length : -1)
})
const cardsAtStart = await cards()
check('1.1 列表渲染非空', cardsAtStart > 0 || totalAtStart === 0, `cards=${cardsAtStart}, total=${totalAtStart}`)
steps.push({ step: '1-冷启动列表', cardCount: cardsAtStart, total: totalAtStart, shot: await shot('01-list') })

// ── 2. 上传真实文件（multipart → SHA256 去重 → 内容寻址落盘）─────────
await page.setInputFiles('input[type=file]', TEST_FILE)
await page.waitForSelector('text=/已上传|已存在相同内容/', { timeout: 15000 })
check('2.1 上传/去重提示可见', await page.getByText(/已上传|已存在相同内容/).first().isVisible(), 'notice shown')
steps.push({ step: '2-上传文件', file: TEST_NAME, shot: await shot('02-upload') })

// ── 3. 搜索命中新上传资产 ───────────────────────────────────────────
await page.getByPlaceholder('搜索资产名称、标签或描述...').fill(TEST_NAME.replace('.png', ''))
await page.waitForTimeout(1200)
const hitCards = await cards()
check('3.1 搜索命中上传资产', hitCards === 1, `cards=${hitCards}`)
steps.push({ step: '3-搜索命中', cardCount: hitCards, shot: await shot('03-search-hit') })

// ── 4. 打开详情 → 预览图片真实加载 ──────────────────────────────────
await page.locator('div.cursor-pointer').first().click()
await page.waitForSelector('text=资产详情', { timeout: 10000 })
// 预览 img 的请求必须 200 且非空
const imgStatus = await page.evaluate(async () => {
  const img = document.querySelector('div[role="dialog"] img, img[alt*="l3-mcd"]')
  if (!img) return { found: false }
  const resp = await fetch(img.src)
  const buf = await resp.arrayBuffer()
  return { found: true, status: resp.status, bytes: buf.byteLength, ok: resp.ok && buf.byteLength > 0 }
})
check('4.1 预览图加载成功', imgStatus.found && imgStatus.ok === true, JSON.stringify(imgStatus))
steps.push({ step: '4-预览', imgStatus, shot: await shot('04-preview') })

// ── 5. 人工标签（添加 + 展示）───────────────────────────────────────
await page.getByPlaceholder('添加标签...').fill('l3-human-tag')
await page.getByRole('button', { name: '添加', exact: true }).click()
await page.waitForSelector('text=l3-human-tag', { timeout: 10000 })
const humanTagVisible = await page.getByText('l3-human-tag', { exact: false }).first().isVisible()
check('5.1 人工标签写入并展示', humanTagVisible, 'badge visible')
steps.push({ step: '5-人工标签', shot: await shot('05-human-tag') })

// ── 6. 提交慢标注任务（排队 → Worker 执行 → completed）──────────────
await page.getByPlaceholder(/输入标注提示词/).fill('classify this test image')
await page.getByRole('button', { name: '提交标注' }).click()
// 等任务完成（worker poll 2s + 标注延迟 0.1-0.5s）
let completedSeen = false
for (let i = 0; i < 20; i++) {
  await page.waitForTimeout(1000)
  const badge = await page.locator('div[role="dialog"] >> text=/^completed$/').count()
  if (badge > 0) { completedSeen = true; break }
}
check('6.1 标注任务完成', completedSeen, `polled 20s, completed=${completedSeen}`)
steps.push({ step: '6-慢标注任务', completed: completedSeen, shot: await shot('06-job-completed') })

// ── 7. AI 建议人工确认写入标注 ──────────────────────────────────────
// 弹窗内容较长，按钮可能在视口外：先滚动到可见再点。
const confirmBtn = page.getByRole('button', { name: '确认并写入标签' })
await confirmBtn.scrollIntoViewIfNeeded()
await confirmBtn.click()
await page.waitForSelector('text=已确认并写入资产标签', { timeout: 10000 })
// AI 标签出现在标签区（source=ai 的 badge）
const aiBadges = await page.locator('div[role="dialog"] >> text=/AI \\d+%/').count()
check('7.1 确认成功提示可见', await page.getByText('已确认并写入资产标签').isVisible(), 'confirm notice')
check('7.2 AI 标签写入并展示', aiBadges > 0, `ai badges=${aiBadges}`)
steps.push({ step: '7-人工确认AI建议', aiBadges, shot: await shot('07-confirmed') })

// ── 8. 汇总 ────────────────────────────────────────────────────────
await browser.close()

const failed = checks.filter((c) => !c.ok)
const result = {
  // L2（P2 独立复审）：badResponses 必须参与 ok 判定（AC-08 要求其为 0），
  // 否则坏响应只在明细里可见、不影响退出码。
  ok: failed.length === 0 && consoleErrors.length === 0 && badResponses.length === 0,
  checks,
  failed,
  consoleErrors,
  badResponses,
  steps,
}
console.log(JSON.stringify(result, null, 2))
process.exit(result.ok ? 0 : 1)
