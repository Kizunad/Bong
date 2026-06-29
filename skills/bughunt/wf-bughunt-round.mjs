export const meta = {
  name: 'bughunt-round',
  description: 'game-flow bug hunt round: find -> debate(real+reachable) -> fix(worktree) -> verify(opus, replaces Pi)',
  phases: [
    { title: 'Find', detail: '4 sonnet finders scan reachable gameplay bugs' },
    { title: 'Debate', detail: 'sonnet skeptics: real + reachable' },
    { title: 'Fix', detail: 'worktree-isolated implement + push branch' },
    { title: 'Verify', detail: 'opus adversarial verify fixed + no regression (replaces Pi)' },
  ],
}

// ---- round id (passed via args from the main loop; args may arrive as a JSON string) ----
const ROUND = (() => {
  let a = args
  if (typeof a === 'string') {
    try { a = JSON.parse(a) } catch { a = {} }
  }
  return (a && a.round) ? String(a.round) : 'adhoc'
})()

const FIND_SCHEMA = {
  type: 'object',
  required: ['findings'],
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'title', 'file_line', 'why_real', 'reachability', 'severity', 'fix_sketch'],
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          file_line: { type: 'string' },
          why_real: { type: 'string' },
          reachability: { type: 'string' },
          severity: { type: 'string', enum: ['critical', 'high', 'medium', 'low'] },
          fix_sketch: { type: 'string' },
        },
      },
    },
  },
}

const VERDICT_SCHEMA = {
  type: 'object',
  required: ['is_real', 'reachable', 'reason'],
  properties: {
    is_real: { type: 'boolean' },
    reachable: { type: 'boolean' },
    reason: { type: 'string' },
    severity_adjust: { type: 'string', enum: ['critical', 'high', 'medium', 'low', 'unchanged'] },
  },
}

const FIX_SCHEMA = {
  type: 'object',
  required: ['pushed', 'branch', 'files', 'diff_summary', 'status'],
  properties: {
    pushed: { type: 'boolean' },
    branch: { type: 'string' },
    files: { type: 'array', items: { type: 'string' } },
    diff_summary: { type: 'string' },
    test_added: { type: 'string' },
    status: { type: 'string', enum: ['done', 'partial', 'gave_up'] },
    notes: { type: 'string' },
  },
}

const VERIFY_SCHEMA = {
  type: 'object',
  required: ['fixed', 'regression_risk', 'reachable_consumer', 'verdict', 'reasoning'],
  properties: {
    fixed: { type: 'boolean' },
    regression_risk: { type: 'string', enum: ['none', 'low', 'medium', 'high'] },
    reachable_consumer: { type: 'boolean' },
    verdict: { type: 'string', enum: ['ship', 'needs_work', 'reject'] },
    reasoning: { type: 'string' },
  },
}

// ---- dedup: modules/themes already fixed or already documented as report-only plan-worthy.
//      APPEND newly-fixed modules here each round so finders stop re-reporting them. ----
const ALREADY_FIXED_QI = 'baomai_v3, zhenmai_v2, dandao, sword_path, jiemai(combat::resolve), burst_meridian, '
  + 'woliu(vortex_maintain), yidao(failed-repair), guangbo_ticao(body_conditioning), dugu, tuike_v2(false_skin_maintenance), '
  + 'full_power_strike(charge_tick), tribulation(aoe+juebi), zhenfa(ward/trap deploy), yidao(mass-repair), '
  + 'dugu_v2(cast_cost+shroud), dugu(infuse), zhenmai_v2+woliu(partial-saturation overflow-discard #696), '
  + 'woliu+dugu_v2-tick(negative-zone .max removal #696/#698), dugu_v2(Penetrate+Reverse target-drain #698), '
  + 'dugu_v2-tick(zone-unreachable->overflow routing #698), anqi_v2(5-skill cast-leak #701), '
  + 'sword_basics(SwordQiStore hit/excretion/expiry #701), carrier(charge qi-gate bypass #701), npc_skill.rs(#702)'

const PLAN_WORTHY_THEMES = 'modifier-orphan (InsightModifiers/ScarForged/baomai-v4 write-side-complete-consume-side-broken), '
  + 'distance-decay QI_DECAY_PER_BLOCK deviation from worldview §four, '
  + 'proto world_pos flat/array drift (server flat x/y/z vs client readDoubleTriple), '
  + 'dead-systems (baomai_v4 ResonanceMeter/ActiveScarCircuits/IronCocoonTracker, DandaoStyle, DaoZhangState never insert)'

const DIMENSIONS = [
  { key: 'qi_sweep', focus: 'SYSTEMIC SWEEP of one conservation pattern: a skill cast/tick deducts Cultivation.qi_current (spend_qi/drain_qi/drain_*_qi, OR an inline `qi_current = (qi_current-cost).clamp(...)` that bypasses any helper) but does NOT credit the spent qi back to the caster zone via release_qi_amount_to_zone / qi_release_to_zone (no zone.spirit_qi mutation) -> qi evaporates, summarize_world_qi conservation broken. ALREADY FIXED, do NOT re-report: ' + ALREADY_FIXED_QI + '. Also do NOT re-report the known PLAN-WORTHY themes already documented in skeletons: ' + PLAN_WORTHY_THEMES + '. Grep ALL remaining server/src/combat/*.rs + server/src/{cultivation,forge,alchemy,gathering}/ skill modules for the SAME pattern. Report each offending module + exact qi_current deduct line + proof there is no zone credit nearby. Also flag any new "deduct then credit with REQUESTED amount instead of ACTUAL clamped amount" over-credit (creates qi), and any negative-zone `.max(0.0)` before *CAP.' },
  { key: 'skill_gate', focus: 'skill cast: meridian/realm/qi-cost precondition bypassed or mis-checked, proficiency/cooldown, effect actually applied to target (emit request but no consumer mutating state = island). server/src/skill + consumers' },
  { key: 'npc_combat', focus: 'NPC big-brain Utility AI: Scorer to Action ordering wrong (FirstToScore is registration-order not score-order -> high priority starved), spawn request with no consumer doing commands.spawn, Position/Transform sync, combat settlement conservation, Lifecycle gate missing (NearDeath/Despawned still acting). server/src/{npc,combat}' },
  { key: 'craft_econ', focus: 'craft/inventory/economy: recipe reachability, equip gate (canEquip vs server validation mismatch), item durability/qi wear, bone-coin economy, spirit-stone fuel, persistence flush on shutdown (AppExit hook missing -> data loss). server/src/{craft,forge,inventory,gathering,persistence} + client InventoryEquipRules' },
]

const SEV_RANK = { critical: 0, high: 1, medium: 2, low: 3 }
function severityOf(item) {
  const adj = item.verdict.severity_adjust
  const sev = (adj && adj !== 'unchanged') ? adj : item.finding.severity
  const r = SEV_RANK[sev]
  return (r === undefined) ? 2 : r
}

phase('Find')
log('round ' + ROUND + ': 4 sonnet finders (1 systemic qi-leak sweep + 3 other classes) off origin/main HEAD')
const finderResults = await parallel(DIMENSIONS.map((d) => () =>
  agent(
    'You are a bug finder for Bong (xianxia MC sandbox: Rust/Valence server + Fabric client + TS agent).'
    + ' Hunt bugs ONLY in dimension [' + d.key + ']: ' + d.focus + '\n\n'
    + 'Hard requirements:\n'
    + '1. Must read code to confirm a REAL bug (logic error / conservation break / island broken-link / state-machine missing branch). Do NOT report style/naming/refactor.\n'
    + '2. Must confirm REACHABILITY: a player reaches this path in normal play (blueprint/recipe/default loadout/skill chain/NPC behavior). If only dev /give triggers it, say so and downgrade. This is a hard gate; the worst outcome is fixing a never-triggered island.\n'
    + '3. Each finding: file:line break point + read-evidence + fix sketch.\n'
    + '4. Quality over quantity: at most 3 high-quality findings; empty array if no real bug.\n'
    + 'You are reading a FRESH origin/main worktree (the ROOT of this run). Already-fixed (do NOT re-report): held-item/fauna-anim/armor/spirit_niche; NPC-meridian-opened-gate (dugu-poison); proto enum-prefix bridge (event_stream/death_screen); the qi-conservation modules and plan-worthy themes listed in your dimension focus. '
    + 'ALREADY REPORTED in round 20260630-r1 (do NOT re-report — in-flight PR or known report-only): HeartDemon Obsession qi_current drain not credited to zone (tribulation.rs resolve_heart_demon_choice); rogue/scattered_cultivator thinker FirstToScore ordering — NpcDefenseScorer starved by ChaseTargetScorer AND PlayerProximityScorer/FleeAction starved by ChaseTargetScorer (rogue.rs); zhenmai.sever_chain TechniqueDefinition qi_cost=50 vs resolver sever_chain_profile(realm) HUD-cost drift; dugu_v2 apply_penetrate missing mark.caster==caster check (unreachable); chase/flee actions missing Lifecycle guard (one-tick ghost-nav, low). '
    + 'ALSO ALREADY REPORTED in round 20260630-r2 (do NOT re-report — in-flight PR or known): Daoxiang/TSY NPC qi_invest(25)>qi_max(10) → anti-cheat gate silences all NPC melee (added AttackSource::NpcMelee); the BROADER npc-melee qi_invest>qi_max / no-zone-credit family (relic.rs:372 qi_invest=12, territory.rs:710 qi_invest=8, skull_fiend.rs:550) — known follow-up cluster, do not re-report each site; DuoShe qi_max clip evaporates qi_current excess without zone credit (possession.rs); SkillBarBind/Cast skip KnownTechniques ownership — any player casts unlearned techniques (client_request_handler player_knows_technique gate); guangbo skill silent qi-fail. '
    + 'ALSO ALREADY REPORTED in round 20260630-r3 (do NOT re-report): Heaven Gate aftermath zeroes qi_current without zone credit (sword_path/skill_register.rs heaven_gate_cast/phase_system); HeartDemon Steadfast outcome mints qi_current from nothing without zone debit (tribulation.rs resolve_heart_demon_choice); baomai_v3 full_power_charge/release missing realm gate (Awaken regress exploit, baomai_v3/skills.rs); player_proximity/flee scorer missing Lifecycle gate (one-tick ghost-nav, low). '
    + 'Find DIFFERENT bugs (or, for qi_sweep, the REMAINING unfixed modules with the same leak pattern — but NOT the npc-melee qi_invest cluster or the heaven-gate/heart-demon/baomai sites above).',
    { label: 'find:' + d.key, phase: 'Find', model: 'sonnet', schema: FIND_SCHEMA },
  ),
))
const found = finderResults.filter(Boolean).flatMap((r) => (r && r.findings) ? r.findings : [])
log('find done: ' + found.length + ' candidates')

const candidates = found.slice(0, 6)

phase('Debate')
const debated = await parallel(candidates.map((f) => () =>
  agent(
    'You are an adversarial skeptic. For the bug finding below, try hard to refute it: is it a real bug? Does surrounding code already handle it? Can a player truly reach it (not dev-only)?\n'
    + 'Default to skepticism — if evidence is weak or unreachable, judge is_real=false / reachable=false. Read the relevant files before ruling.\n\n'
    + 'finding: ' + JSON.stringify(f),
    { label: 'debate:' + f.id, phase: 'Debate', model: 'sonnet', schema: VERDICT_SCHEMA },
  ).then((v) => (v ? { finding: f, verdict: v } : null)),
))
const confirmed = debated.filter(Boolean).filter((x) => x.verdict.is_real && x.verdict.reachable)
log('debate done: ' + confirmed.length + '/' + candidates.length + ' confirmed real+reachable')

if (confirmed.length === 0) {
  return { round: ROUND, found: found.length, confirmed: 0, fixes: [], note: 'no confirmed real+reachable bug this round' }
}

const toFix = confirmed.slice().sort((a, b) => severityOf(a) - severityOf(b)).slice(0, 3)
log('fix targets: ' + toFix.map((x) => x.finding.id).join(', '))

phase('Fix')
const fixes = await parallel(toFix.map((x, i) => () =>
  agent(
    'You are in an isolated git worktree (based on origin/main). Fix this confirmed bug.\n\n'
    + 'bug: ' + JSON.stringify(x.finding) + '\n'
    + 'debate verdict: ' + JSON.stringify(x.verdict) + '\n\n'
    + 'Requirements:\n'
    + '1. Implement the MINIMAL fix at the file:line break point; wire to a real consumer, do not create a new island. If a new Bevy system is needed, REGISTER it in the relevant mod.rs (an unregistered system is dead code; do not rely on a test add_systems to mask it).\n'
    + '2. Add a test locking the behavior (happy + key boundary/error branch). Rust uses #[cfg(test)], client uses JUnit.\n'
    + '   QI-ZONE-CREDIT TEST PITFALLS (your tests MUST account for these or CI goes red):\n'
    + '     a. ZoneRegistry::fallback() spawn zone starts near-full (spirit_qi≈0.9, only ~5 raw room); if your spent cost > room it SPLITS into a zone transfer + discarded overflow, so a `total==cost` assertion fails. EITHER empty it first (`app.world_mut().resource_mut::<ZoneRegistry>().find_zone_mut("spawn").unwrap().spirit_qi = 0.0;`) for a clean full-credit assertion, OR assert the split.\n'
    + '     b. The test caster/defender entity needs a CurrentDimension component (real players get it at spawn; test helpers often dont) — without it find_zone returns None and the credit routes to an Overflow account, not the zone.\n'
    + '     c. Credit the ACTUAL clamped deduction (before-after, or amount.min(qi_current.max(0))), NOT the requested amount — over-crediting creates qi (CodeRabbit flags this Critical).\n'
    + '     d. You CANNOT run the tests (rule 3 forbids the build) — so get these RIGHT BY CONSTRUCTION; a test that compiles can still fail at runtime, and the orchestrator will bounce it back. Avoid float equality at clamp boundaries (use the real computed value, not a hardcoded integer).\n'
    + '3. Do NOT run cargo build / gradle build (full worktree compile blows up disk). At most `cargo check` one crate or read-review.\n'
    + '4. When done: git checkout -b bughunt-' + ROUND + '-' + i + '-' + x.finding.id + ' && git add <changes> && git commit && git push -u origin HEAD. Return the branch name.\n'
    + '5. Only touch files needed for THIS bug; no opportunistic refactor. `git add` ONLY your changed .rs/.java files by path (never `git add -A` — it sweeps untracked junk: wf-*.mjs/snapshots/zips).\n'
    + 'If it turns out not to be a real bug or the fix needs a design decision / spans many modules, set status=gave_up and explain (better unfixed than wrongly changed).',
    { label: 'fix:' + x.finding.id, phase: 'Fix', model: 'sonnet', isolation: 'worktree', schema: FIX_SCHEMA },
  ).then((fx) => (fx ? { finding: x.finding, verdict: x.verdict, fix: fx } : null)),
))
const realFixes = fixes.filter(Boolean).filter((x) => x.fix.status === 'done' && x.fix.pushed)
log('fix done: ' + realFixes.length + ' done+pushed')

phase('Verify')
// opus verify — at most 3 (toFix capped at 3), satisfies the opus<=3 concurrency rule.
const verified = await parallel(realFixes.map((x) => () =>
  agent(
    'You are an opus adversarial verifier (replacing Pi review). Read-only diff review — do NOT build or run cargo/gradle (the orchestrator runs fmt/clippy/test in the main loop; disk is tight). Verify:\n'
    + '1. Does the change ACTUALLY fix the original bug — read the real diff via `git diff origin/main..' + x.fix.branch + '` + surrounding code (read files, no build).\n'
    + '2. Any REGRESSION risk (breaks other paths / conservation / state machine).\n'
    + '3. Does the fix reach a REAL reachable consumer (not an island — emitting with no consumer != fixed; a new system must be registered in mod.rs).\n'
    + '4. Does the test truly lock the behavior (read the test code; watch for vacuous test-copies of production logic, and float-equality at clamp boundaries).\n'
    + 'Be strictly skeptical. verdict: ship=PR-ready / needs_work=flawed / reject=not fixed or creates island.\n\n'
    + 'original bug: ' + JSON.stringify(x.finding) + '\nfix: ' + JSON.stringify(x.fix),
    { label: 'verify:' + x.finding.id, phase: 'Verify', model: 'opus', schema: VERIFY_SCHEMA },
  ).then((v) => (v ? Object.assign({}, x, { verify: v }) : null)),
))

return {
  round: ROUND,
  found: found.length,
  confirmed: confirmed.length,
  fixes: verified.filter(Boolean).map((x) => ({
    id: x.finding.id,
    title: x.finding.title,
    severity: x.finding.severity,
    branch: x.fix.branch,
    files: x.fix.files,
    diff_summary: x.fix.diff_summary,
    test_added: x.fix.test_added,
    verify: x.verify,
  })),
  skipped: confirmed.filter((c) => toFix.indexOf(c) === -1).map((c) => c.finding.id),
}
