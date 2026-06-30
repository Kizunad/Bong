// 把 scratch 里各层的 cluster-*.json / feature-*.json 注入 module-map/index.html 数据块。
// 用法: node assemble.mjs
import { readFileSync, writeFileSync, readdirSync, existsSync } from 'node:fs'
import { join } from 'node:path'

const MM = '/tmp/claude-1002/-home-kiz-Code-Bong/56c67fa2-3435-4d4d-8600-1904202ca7f5/scratchpad/mm'
const HTML = '/home/kiz/Code/Bong/module-map/index.html'
const LAYERS = ['server', 'client', 'agent']

// 严重度归一化到 critical|warn|info(agent 直写 JSON 会跑偏成 medium/low/空)
const normSev = (s) => {
  const v = String(s || '').toLowerCase()
  if (/crit|high|severe|block/.test(v)) return 'critical'
  if (/warn|medium|moderate|major/.test(v)) return 'warn'
  return 'info'
}
const normOneGap = (g) => typeof g === 'string'
  ? { severity: 'info', note: g }
  : { severity: normSev(g && g.severity), note: (g && (g.note || g.message)) || JSON.stringify(g) }
const normGaps = (o) => {
  if (!o) return
  if (Array.isArray(o.gaps)) o.gaps = o.gaps.map(normOneGap)
  ;(o.components || o.aspects || []).forEach(c => { if (Array.isArray(c.gaps)) c.gaps = c.gaps.map(normOneGap) })
}

const REQUIRED = ['id', 'layer', 'name', 'path', 'title', 'summary', 'components']
const modules = []
const features = []
const problems = []

for (const layer of LAYERS) {
  const dir = join(MM, layer)
  if (!existsSync(dir)) continue
  for (const f of readdirSync(dir).sort()) {
    const p = join(dir, f)
    let data
    try { data = JSON.parse(readFileSync(p, 'utf8')) } catch (e) { problems.push(`${p}: JSON 解析失败 ${e.message}`); continue }
    if (f.startsWith('cluster-')) {
      const arr = Array.isArray(data) ? data : [data]
      for (const m of arr) {
        const miss = REQUIRED.filter(k => !(k in m))
        if (miss.length) { problems.push(`${p} 模块 ${m.id || '?'} 缺字段: ${miss.join(',')}`); }
        normGaps(m)
        modules.push(m)
      }
    } else if (f.startsWith('feature-')) {
      normGaps(data)
      features.push(data)
    }
  }
}

// 去重(按 id),后写覆盖先写
const dedupe = (arr) => { const m = new Map(); for (const x of arr) m.set(x.id, x); return [...m.values()] }
const mods = dedupe(modules)
const feats = dedupe(features)
mods.sort((a, b) => (a.layer + a.name).localeCompare(b.layer + b.name))

const html = readFileSync(HTML, 'utf8')
const START = '=== DATA:START ==='
const END = '/* === DATA:END === */'
const si = html.indexOf(START)
const commentEnd = html.indexOf('*/', si) + 2
const ei = html.indexOf(END)
if (si < 0 || ei < 0) { console.error('找不到数据标记'); process.exit(1) }

// 合并模式: 保留 html 里已注入、但本次 scratch 没覆盖的层(跨会话/容器重启 durable)
const between = html.slice(commentEnd, ei)
const grab = (re) => { const m = between.match(re); try { return m ? JSON.parse(m[1]) : [] } catch { return [] } }
const prevMods = grab(/const MODULES = (\[[\s\S]*?\]);\s*\nconst FEATURES/)
const prevFeats = grab(/const FEATURES = (\[[\s\S]*?\]);\s*$/)
const scratchModIds = new Set(modules.map(m => m.id))
const scratchFeatIds = new Set(features.map(f => f.id))
for (const m of prevMods) if (!scratchModIds.has(m.id)) modules.push(m)   // 旧层数据补回
for (const f of prevFeats) if (!scratchFeatIds.has(f.id)) features.push(f)

const block = `\nconst MODULES = ${JSON.stringify(mods, null, 2)};\n\nconst FEATURES = ${JSON.stringify(feats, null, 2)};\n`
const out = html.slice(0, commentEnd) + block + html.slice(ei)
writeFileSync(HTML, out)

const byLayer = mods.reduce((a, m) => (a[m.layer] = (a[m.layer] || 0) + 1, a), {})
const gapN = [...mods, ...feats].reduce((n, o) => {
  const cs = o.components || o.aspects || []
  return n + (o.gaps || []).filter(g => g.severity !== 'info').length
    + cs.reduce((k, c) => k + (c.gaps || []).filter(g => g.severity !== 'info').length, 0)
}, 0)
console.log(`✅ 注入 ${mods.length} 模块 (${JSON.stringify(byLayer)}) + ${feats.length} feature + ${gapN} 缺口`)
if (problems.length) { console.log('⚠️ 问题:'); problems.forEach(p => console.log('  - ' + p)) }
