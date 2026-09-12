// partisync 浏览器回归脚本（无头 Chromium / playwright-core）
//
// 用途：验证 T6′ 复审发现的前端阻断项已修复，并为 T7（L3 浏览器真实入口验收）
// 提供可复用的自动化底座。它断言的是**运行时行为**（DOM），而不是"类型检查通过"。
//
// 前置：
//   1) 后端在 127.0.0.1:8080（docker compose up -d），或经 VITE 代理可达；
//   2) 前端 Vite dev server：cd playground && npm run dev -- --port 5201 --strictPort
//      其 vite.config.ts 已把 /api 代理到 localhost:8080。
//
// 运行：
//   L3_BASE=http://127.0.0.1:5201/ L3_SHOTS=/tmp/l3-shots \
//     node scripts/browser-regression.mjs
//
// 退出码：0 = 全部断言通过；1 = 存在失败断言（详见 stdout 的 JSON）。

import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

// 注意：仓库路径含空格，必须用 fileURLToPath 解码（import.meta.url 的 pathname 会带 %20）
const PROJ = fileURLToPath(new URL('../playground/', import.meta.url))
const require = createRequire(PROJ + 'package.json')
const { chromium } = require('playwright-core')

const EXEC =
  process.env.L3_CHROME ||
  '/home/acme/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome'
const BASE = process.env.L3_BASE || 'http://127.0.0.1:5201/'
const SHOTS = process.env.L3_SHOTS || '/tmp/l3-shots'

const browser = await chromium.launch({
  executablePath: EXEC,
  headless: true,
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
})
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })

const consoleErrors = []
const badResponses = []
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(m.text())
})
page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + e.message))
page.on('response', async (r) => {
  if (r.status() >= 400) {
    let body = ''
    try {
      body = (await r.text()).slice(0, 150)
    } catch {}
    badResponses.push({ status: r.status(), method: r.request().method(), url: r.url(), body })
  }
})

const steps = []
const checks = []
const shot = (name) => page.screenshot({ path: `${SHOTS}/${name}.png` }).then(() => `${SHOTS}/${name}.png`)
const cards = () => page.locator('div.cursor-pointer').count()
const emptyState = () => page.getByText('暂无符合条件的资产').count()
const check = (name, ok, detail) => checks.push({ name, ok, detail })

// 1. 冷启动 → 列表必须渲染卡片（T6′ C2-1：修复前恒为空态）
await page.goto(BASE, { waitUntil: 'networkidle' })
await page.waitForSelector('h1:has-text("资产管理")', { timeout: 15000 })
await page.waitForTimeout(700)
const cardCount = await cards()
const headerText = await page.locator('h1:has-text("资产管理")').locator('..').innerText()
check('列表渲染非空（C2-1）', cardCount > 0, `cards=${cardCount}`)
check('无空态（C2-1）', (await emptyState()) === 0, `emptyState=${await emptyState()}`)
check('总数非 0（C2-4）', /共\s*[1-9]\d*\s*个资产/.test(headerText), headerText.replace(/\n/g, ' '))
steps.push({ step: '1-列表渲染', cardCount, headerText, pager: await page.getByText(/\d+–\d+ \/ \d+/).first().innerText().catch(() => '(none)'), shot: await shot('01-list') })

// 2. 搜索（防抖 150ms）
await page.getByPlaceholder('搜索资产名称、标签或描述...').fill('hero')
await page.waitForTimeout(1000)
const searchCount = await cards()
check('搜索命中收敛', searchCount > 0 && searchCount < cardCount, `cards=${searchCount}`)
steps.push({ step: '2-搜索 hero', cardCount: searchCount, shot: await shot('02-search-hero') })

// 3. 类型过滤
await page.getByPlaceholder('搜索资产名称、标签或描述...').fill('')
await page.waitForTimeout(700)
await page.getByRole('button', { name: '图片', exact: true }).click()
await page.waitForTimeout(1000)
const filtered = await cards()
check('resource_type=image 过滤生效', filtered > 0, `cards=${filtered}`)
steps.push({ step: '3-过滤[图片]', cardCount: filtered, pager: await page.getByText(/\d+–\d+ \/ \d+/).first().innerText().catch(() => '(none)'), shot: await shot('03-filter-image') })

// 4. 表格视图
await page.getByRole('button', { name: '表格', exact: true }).click()
await page.waitForTimeout(600)
const rows = await page.locator('table tbody tr').count()
check('表格视图渲染行', rows > 0, `rows=${rows}`)
steps.push({ step: '4-表格视图', rows, shot: await shot('04-table') })

// 5. 详情弹窗（C2-6：Meili 命中字段不全时不得渲染 Invalid Date，且弹窗不得被回源失败置空）
await page.locator('table tbody tr').first().click()
await page.waitForTimeout(900)
const dialogText = await page.locator('[role="dialog"]').innerText().catch(() => '')
check('详情弹窗保持打开', dialogText.includes('资产详情'), dialogText.slice(0, 40))
check('详情无 Invalid Date（C2-6）', !/Invalid Date/.test(dialogText), '')
steps.push({ step: '5-详情弹窗', dialogExcerpt: dialogText.split('\n').slice(0, 12).join(' | '), shot: await shot('05-dialog') })

// 6. 搜索场景下的 App 上传资产（Meili 命中路径）
await page.keyboard.press('Escape')
await page.waitForTimeout(400)
await page.getByRole('button', { name: '卡片网格', exact: true }).click()
await page.getByPlaceholder('搜索资产名称、标签或描述...').fill('hero')
await page.waitForTimeout(1000)
const heroCard = await page.locator('div.cursor-pointer').first().innerText()
check('App 上传资产卡片无 Invalid Date（C2-6）', !/Invalid Date/.test(heroCard), heroCard.replace(/\n/g, ' | '))
await page.locator('div.cursor-pointer').first().click()
await page.waitForTimeout(900)
const heroDialog = await page.locator('[role="dialog"]').innerText().catch(() => '')
check('App 上传资产详情回源到权威路径（C2-6）', /路径[\s\S]{0,10}[0-9a-f]{2}\//.test(heroDialog), heroDialog.split('\n').slice(0, 10).join(' | '))
steps.push({ step: '6-App资产详情', card: heroCard.replace(/\n/g, ' | '), shot: await shot('06-app-asset-detail') })

check('无 console 错误', consoleErrors.length === 0, JSON.stringify(consoleErrors))
check('无 4xx/5xx 请求', badResponses.length === 0, JSON.stringify(badResponses))

await browser.close()
console.log(JSON.stringify({ base: BASE, checks, consoleErrors, badResponses, steps }, null, 2))
process.exit(checks.every((c) => c.ok) ? 0 : 1)
