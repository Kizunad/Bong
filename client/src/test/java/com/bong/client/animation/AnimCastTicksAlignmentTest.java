package com.bong.client.animation;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-anim-fidelity-v1 P0 —— cast_ticks ↔ 动画时长对拍测试（精度标准 #2
 * 三套断言的机械化）+ 现状不达标 allowlist 棘轮 + spec manifest 断言框架。
 *
 * <p>快照 {@code bong/technique_cast_ticks_snapshot.json} 由 server
 * {@code TECHNIQUE_DEFINITIONS} 单向生成（重生成唯一入口
 * {@code cd server && BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot}），
 * 本测试只消费不维护——错误时长必须改动画对齐定义表，禁止「同步改快照混过关」
 * （plan §8.1 #4）。
 *
 * <p>三套断言（与精度标准 #2 严格同一时序模型）：
 * <ol>
 *   <li>普通非循环招（2 &lt; cast &lt; 40）：{@code endTick ∈ [cast+4, cast+8]}（recovery 红线）且非循环；</li>
 *   <li>瞬发招（cast ≤ 2）：总长 ∈ [6, 12] 且非循环（爆发帧 + 收势，不因 cast 短而砍收势）；</li>
 *   <li>长引导招（cast ≥ 40）：蓄力段动画 {@code isLoop} 且每个用到的轴在 endTick 有关键帧
 *       （防库坑 #1 单帧衰减的机械底线）；release 段存在性归 spec manifest（新交付时强制）。</li>
 * </ol>
 *
 * <p>现状不达标项进 {@link #CAST_ALIGNMENT_ALLOWLIST}（只缩不涨棘轮，冻结基线
 * {@link #P0_BASELINE} 永不追加）；<b>allowlist 清零 = P1-P4 完成的机械判据</b>。
 */
class AnimCastTicksAlignmentTest {
    private static final String SNAPSHOT_RESOURCE = "bong/technique_cast_ticks_snapshot.json";
    private static final String REGEN_HINT =
        "快照由 server TECHNIQUE_DEFINITIONS 单向生成、禁止手改；重生成："
            + "cd server && BONG_REGEN_CAST_TICKS_SNAPSHOT=1 cargo test technique_cast_ticks_snapshot";

    /**
     * skill_id → 发射的动画文件名（plan 附录 A 审计矩阵同源；借用招映射到同一动画）。
     * value = null 表示该招当前无任何动画发射（进 {@link #MISSING_ANIM_ALLOWLIST}）。
     * npc.* 三招不在本表——NPC mob 无 PlayAnim 通道（plan §8.1 #2），动画维度 N/A。
     */
    private static final Map<String, String> SKILL_ANIM = buildSkillAnimMap();

    private static Map<String, String> buildSkillAnimMap() {
        Map<String, String> m = new TreeMap<>();
        m.put("sword.cleave", "sword_cleave");
        m.put("sword.thrust", "sword_thrust");
        m.put("sword.parry", "sword_parry");
        m.put("sword.infuse", "sword_infuse");
        m.put("movement.dash", "dash_forward");
        m.put("shield_block", "shield_raise");
        m.put("burst_meridian.beng_quan", "beng_quan");
        m.put("burst_meridian.tie_shan_kao", "beng_quan");
        m.put("burst_meridian.xue_beng_bu", "beng_quan");
        m.put("burst_meridian.ni_mai_hu_ti", null);
        m.put("baomai.full_power_charge", "windup_charge");
        m.put("baomai.full_power_release", "release_burst");
        m.put("zhenmai.parry", "zhenmai_parry");
        m.put("zhenmai.neutralize", "zhenmai_neutralize");
        m.put("zhenmai.multipoint", "zhenmai_multipoint");
        m.put("zhenmai.harden", "zhenmai_harden");
        m.put("zhenmai.sever_chain", "zhenmai_sever_chain");
        m.put("woliu.vortex", null);
        m.put("woliu.hold", "vortex_palm_open");
        m.put("woliu.burst", "palm_strike");
        m.put("woliu.mouth", "palm_thrust");
        m.put("woliu.pull", "woliu_vacuum_lock");
        m.put("woliu.heart", "vortex_spiral_stance");
        m.put("woliu.vacuum_palm", "woliu_vacuum_palm");
        m.put("woliu.vortex_shield", "woliu_vortex_shield");
        m.put("woliu.vacuum_lock", "woliu_vacuum_lock");
        m.put("woliu.vortex_resonance", "woliu_vortex_resonance");
        m.put("woliu.turbulence_burst", "woliu_turbulence_burst");
        m.put("dugu.shoot_needle", "dugu_needle_throw");
        m.put("dugu.infuse_poison", "dugu_needle_throw");
        m.put("tuike.don", "tuike_don_skin");
        m.put("tuike.shed", "tuike_shed_burst");
        m.put("tuike.transfer_taint", "tuike_taint_transfer");
        m.put("anqi.charge_carrier", "windup_charge");
        m.put("anqi.single_snipe", "sword_stab");
        m.put("anqi.multi_shot", "release_burst");
        m.put("anqi.soul_inject", "cast_invoke");
        m.put("anqi.armor_pierce", "cast_invoke");
        m.put("anqi.echo_fractal", "release_burst");
        m.put("body.guangbo_ticao", "guangbo_ticao");
        m.put("sword_path.condense_edge", "sword_cleave");
        m.put("sword_path.qi_slash", "sword_thrust");
        m.put("sword_path.resonance", "sword_cleave");
        m.put("sword_path.manifest", "sword_manifest_cast");
        m.put("sword_path.heaven_gate", "sword_heaven_gate_charge");
        m.put("morph.yixing", "morph_cast");
        return m;
    }

    /** NPC mob 施放、无 PlayAnim 通道的招（快照有、映射无的唯一合法集合）。 */
    private static final Set<String> NPC_SKILLS =
        Set.of("npc.heal_basic", "npc.buff_speed", "npc.buff_defense");

    /**
     * 持续维持型例外：循环 + StopAnim 停止路径是**正当设计**（举盾按住持续），
     * 时长对拍三套断言不适用；专属用例断言其循环结构不退化。
     */
    private static final Set<String> SUSTAINED_LOOP_EXCEPTIONS = Set.of("shield_block");

    /**
     * 长演出例外：一次性完整长动画（cast 只是入口时长，动画刻意超长演出），
     * 时长对拍不适用；专属用例断言其非循环长度不倒退。
     */
    private static final Set<String> LONG_FORM_EXCEPTIONS = Set.of("body.guangbo_ticao");

    /**
     * 现状时长对拍不达标 allowlist（P0 落档，2026-07-18 离线全量核算产出）——
     * **只许缩小不许增长**：修好一招必须删对应条目；清零 = P1-P4 完成的机械判据。
     */
    private static final Set<String> CAST_ALIGNMENT_ALLOWLIST = Set.of(
        "sword.cleave", "sword.thrust", "sword.infuse", "movement.dash",
        "burst_meridian.beng_quan", "burst_meridian.tie_shan_kao", "burst_meridian.xue_beng_bu",
        "baomai.full_power_charge", "baomai.full_power_release",
        "zhenmai.parry", "zhenmai.neutralize", "zhenmai.multipoint", "zhenmai.harden",
        "zhenmai.sever_chain",
        "woliu.heart", "woliu.vacuum_palm", "woliu.vortex_shield", "woliu.vacuum_lock",
        "woliu.turbulence_burst",
        // vortex_resonance 时长模型达标（80t loop 对齐 cast=80），但 10 个手臂轴
        // endTick 无补帧（库坑 #1 单帧衰减）——**存量 bug 已登记归
        // plan-bughunt-woliu-resonance-loop-arm-decay-v1**（本 plan 防重声明明确
        // 排除，标准只防再犯），该 bugfix merge 后删本条目。
        "woliu.vortex_resonance",
        "anqi.single_snipe", "anqi.multi_shot", "anqi.soul_inject", "anqi.armor_pierce",
        "anqi.echo_fractal",
        "sword_path.qi_slash", "sword_path.resonance", "sword_path.manifest",
        "sword_path.heaven_gate", "morph.yixing");

    /** 无任何动画发射的招（D 级缺失，重制批次补动画后删条目）。 */
    private static final Set<String> MISSING_ANIM_ALLOWLIST =
        Set.of("burst_meridian.ni_mai_hu_ti", "woliu.vortex");

    /**
     * P0 冻结基线（机器可执行的"只缩不涨"棘轮）：两份 allowlist 必须是本集合的
     * 子集——删除条目（动画达标）自然通过，新增任何条目立刻判红。本基线**永不
     * 追加**：新招不达标的唯一出路是按精度标准做动画，不得经由扩大 allowlist 放行。
     */
    private static final Set<String> P0_BASELINE = Set.of(
        // P0 时刻（2026-07-18）字面量冻结——**不得由 allowlist 动态计算**（自引用
        // 会让「只缩不涨」断言恒真失效，CodeRabbit r1 修正），也永不追加。
        "sword.cleave", "sword.thrust", "sword.infuse", "movement.dash",
        "burst_meridian.beng_quan", "burst_meridian.tie_shan_kao", "burst_meridian.xue_beng_bu",
        "burst_meridian.ni_mai_hu_ti",
        "baomai.full_power_charge", "baomai.full_power_release",
        "zhenmai.parry", "zhenmai.neutralize", "zhenmai.multipoint", "zhenmai.harden",
        "zhenmai.sever_chain",
        "woliu.vortex", "woliu.heart", "woliu.vacuum_palm", "woliu.vortex_shield",
        "woliu.vacuum_lock", "woliu.vortex_resonance", "woliu.turbulence_burst",
        "anqi.single_snipe", "anqi.multi_shot", "anqi.soul_inject", "anqi.armor_pierce",
        "anqi.echo_fractal",
        "sword_path.qi_slash", "sword_path.resonance", "sword_path.manifest",
        "sword_path.heaven_gate", "morph.yixing");

    // ---- 快照 / 动画元数据读取 ----

    private static Map<String, Integer> loadSnapshot() throws IOException {
        try (InputStream input = AnimCastTicksAlignmentTest.class.getClassLoader()
                .getResourceAsStream(SNAPSHOT_RESOURCE)) {
            assertNotNull(input, "缺少 cast_ticks 快照 fixture: " + SNAPSHOT_RESOURCE
                + " —— " + REGEN_HINT);
            JsonObject root = JsonParser
                .parseReader(new InputStreamReader(input, StandardCharsets.UTF_8))
                .getAsJsonObject();
            Map<String, Integer> snapshot = new TreeMap<>();
            for (String key : root.keySet()) {
                snapshot.put(key, root.get(key).getAsInt());
            }
            assertFalse(snapshot.isEmpty(), "cast_ticks 快照不应为空 —— " + REGEN_HINT);
            return snapshot;
        }
    }

    /** 生产动画资产根（与 {@code AnimWiringManifestTest#animationAssetRoot()} 同语义）。 */
    private static Path animationAssetRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        for (Path candidate : List.of(
                cwd.resolve("src/main/resources/assets/bong/player_animation"),
                cwd.resolve("client/src/main/resources/assets/bong/player_animation"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException(
            "无法定位 client main resources 的 player_animation 根（user.dir=" + cwd
                + "）——对拍必须绑定 main source set");
    }

    private record AnimMeta(int endTick, boolean isLoop, JsonArray moves) {
    }

    private static AnimMeta readAnim(String animName) throws IOException {
        Path file = animationAssetRoot().resolve(animName + ".json");
        assertTrue(Files.isRegularFile(file),
            "映射表指向的动画资产不存在：" + file + "——映射表与磁盘漂移，先修映射再谈对拍");
        JsonObject emote = JsonParser.parseString(Files.readString(file))
            .getAsJsonObject().getAsJsonObject("emote");
        return new AnimMeta(
            emote.get("endTick").getAsInt(),
            emote.has("isLoop") && emote.get("isLoop").getAsBoolean(),
            emote.getAsJsonArray("moves"));
    }

    /** 动画用到的全部轴（part.axis）集合，及 endTick 帧上出现的轴集合。 */
    private static void collectAxes(AnimMeta meta, Set<String> allAxes, Set<String> endTickAxes) {
        for (JsonElement moveElement : meta.moves()) {
            JsonObject move = moveElement.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing") || part.equals("comment")) {
                    continue;
                }
                JsonElement axes = move.get(part);
                if (!axes.isJsonObject()) {
                    continue;
                }
                for (String axis : axes.getAsJsonObject().keySet()) {
                    if (axis.equals("comment")) {
                        continue;
                    }
                    String key = part + "." + axis;
                    allAxes.add(key);
                    if (tick == meta.endTick()) {
                        endTickAxes.add(key);
                    }
                }
            }
        }
    }

    /**
     * 轴 → (tick → 值) 全量解析（循环补帧**同值**断言用——只查轴存在挡不住
     * endTick 写默认值/错值造成的循环接缝，CodeRabbit r1 修正）。
     */
    private static Map<String, Map<Integer, Double>> collectAxisValues(AnimMeta meta) {
        Map<String, Map<Integer, Double>> values = new HashMap<>();
        for (JsonElement moveElement : meta.moves()) {
            JsonObject move = moveElement.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing") || part.equals("comment")) {
                    continue;
                }
                JsonElement axes = move.get(part);
                if (!axes.isJsonObject()) {
                    continue;
                }
                for (String axis : axes.getAsJsonObject().keySet()) {
                    if (axis.equals("comment")) {
                        continue;
                    }
                    values.computeIfAbsent(part + "." + axis, k -> new HashMap<>())
                        .put(tick, axes.getAsJsonObject().get(axis).getAsDouble());
                }
            }
        }
        return values;
    }

    /**
     * 循环动画补帧同值检查：每个用到的轴在 endTick 必须有帧，且值 == 循环起点
     * （最小 tick）帧值——库坑 #1 的完整语义（同值 keyframe，非仅存在）。
     * 返回违规轴描述列表（空=通过）。
     */
    private static List<String> loopSeamViolations(AnimMeta meta) {
        List<String> violations = new ArrayList<>();
        for (Map.Entry<String, Map<Integer, Double>> axisEntry : collectAxisValues(meta).entrySet()) {
            Map<Integer, Double> byTick = axisEntry.getValue();
            int firstTick = byTick.keySet().stream().min(Integer::compare).orElseThrow();
            Double endValue = byTick.get(meta.endTick());
            if (endValue == null) {
                violations.add(axisEntry.getKey() + "（endTick 无关键帧）");
            } else if (!endValue.equals(byTick.get(firstTick))) {
                violations.add(axisEntry.getKey() + "（endTick=" + endValue + " ≠ 起点帧="
                    + byTick.get(firstTick) + "，循环回绕跳变）");
            }
        }
        return violations;
    }

    /** 精度标准 #2 三套断言；返回 null=达标，非 null=不达标原因（供 allowlist 双向棘轮复用）。 */
    private static String alignmentFailure(int cast, AnimMeta meta) {
        if (cast <= 2) {
            if (meta.isLoop()) {
                return "瞬发招动画不应循环（isLoop=true）";
            }
            if (meta.endTick() < 6 || meta.endTick() > 12) {
                return "瞬发招总长 " + meta.endTick() + " ∉ [6,12]（爆发帧+收势）";
            }
            return null;
        }
        if (cast >= 40) {
            if (!meta.isLoop()) {
                return "长引导招蓄力段应为循环（isLoop=false）——须拆循环蓄力段+release 段两段";
            }
            List<String> seams = loopSeamViolations(meta);
            if (!seams.isEmpty()) {
                return "循环蓄力段 " + seams.size() + " 个轴违反 endTick 同值补帧（库坑 #1）：" + seams;
            }
            return null;
        }
        if (meta.isLoop()) {
            return "普通非循环招不应循环（isLoop=true）";
        }
        if (meta.endTick() < cast + 4 || meta.endTick() > cast + 8) {
            return "endTick " + meta.endTick() + " ∉ [" + (cast + 4) + "," + (cast + 8)
                + "]（cast 完成=发力顶点 + recovery 4-8 tick）";
        }
        return null;
    }

    // ---- 用例 ----

    /** 快照覆盖：映射表每招在快照有 cast_ticks；快照多出的键只能是 npc 三招。 */
    @Test
    void snapshotCoversEveryMappedSkillAndOnlyNpcSkillsAreUnmapped() throws IOException {
        Map<String, Integer> snapshot = loadSnapshot();
        for (String skillId : SKILL_ANIM.keySet()) {
            assertTrue(snapshot.containsKey(skillId),
                "映射表招式 `" + skillId + "` 不在 cast_ticks 快照内——server 定义表删招"
                    + "未同步映射表，或快照过期 —— " + REGEN_HINT);
        }
        Set<String> unmapped = new HashSet<>(snapshot.keySet());
        unmapped.removeAll(SKILL_ANIM.keySet());
        assertEquals(NPC_SKILLS, unmapped,
            "快照有而映射表无的招必须恰为 npc 三招（NPC mob 无 PlayAnim 通道，"
                + "plan §8.1 #2）——出现其他条目说明 server 新增招未入对拍映射表");
    }

    /** 主契约：非 allowlist / 非例外的招全部满足精度标准 #2 三套断言。 */
    @Test
    void castAlignmentContractHoldsForEverySkillOutsideAllowlist() throws IOException {
        Map<String, Integer> snapshot = loadSnapshot();
        List<String> violations = new ArrayList<>();
        for (Map.Entry<String, String> entry : SKILL_ANIM.entrySet()) {
            String skillId = entry.getKey();
            if (SUSTAINED_LOOP_EXCEPTIONS.contains(skillId)
                || LONG_FORM_EXCEPTIONS.contains(skillId)
                || CAST_ALIGNMENT_ALLOWLIST.contains(skillId)
                || MISSING_ANIM_ALLOWLIST.contains(skillId)) {
                continue;
            }
            assertNotNull(entry.getValue(),
                "招式 `" + skillId + "` 无动画映射却不在 MISSING_ANIM_ALLOWLIST——"
                    + "缺动画必须显式入 allowlist 留痕");
            String failure = alignmentFailure(snapshot.get(skillId), readAnim(entry.getValue()));
            if (failure != null) {
                violations.add(skillId + "（" + entry.getValue() + "）：" + failure);
            }
        }
        assertTrue(violations.isEmpty(),
            "以下招式违反精度标准 #2 时长对拍且不在 allowlist：" + violations
                + "——新招/重制动画必须直接达标，不得扩大 allowlist（冻结基线只缩不涨）");
    }

    /** 棘轮下界：allowlist 条目必须确实不达标——动画修好即强制删条目。 */
    @Test
    void allowlistEntriesActuallyFailAlignment() throws IOException {
        Map<String, Integer> snapshot = loadSnapshot();
        for (String skillId : CAST_ALIGNMENT_ALLOWLIST) {
            String anim = SKILL_ANIM.get(skillId);
            assertNotNull(anim,
                "allowlist 条目 `" + skillId + "` 无动画映射——应移入 MISSING_ANIM_ALLOWLIST");
            String failure = alignmentFailure(snapshot.get(skillId), readAnim(anim));
            assertNotNull(failure,
                "allowlist 条目 `" + skillId + "` 的动画 `" + anim + "` 已达标——"
                    + "必须立刻从 CAST_ALIGNMENT_ALLOWLIST 删除该条目（棘轮只缩不涨）");
        }
        for (String skillId : MISSING_ANIM_ALLOWLIST) {
            assertTrue(SKILL_ANIM.containsKey(skillId) && SKILL_ANIM.get(skillId) == null,
                "MISSING_ANIM_ALLOWLIST 条目 `" + skillId + "` 已有动画映射——"
                    + "必须删条目并让其走时长对拍主契约");
        }
    }

    /** 冻结基线棘轮：两份 allowlist ⊆ P0 基线（只缩不涨，永不追加）。 */
    @Test
    void allowlistsOnlyShrinkAgainstFrozenBaseline() {
        assertTrue(P0_BASELINE.containsAll(CAST_ALIGNMENT_ALLOWLIST),
            "CAST_ALIGNMENT_ALLOWLIST 出现 P0 冻结基线之外的条目："
                + CAST_ALIGNMENT_ALLOWLIST.stream()
                    .filter(id -> !P0_BASELINE.contains(id)).toList()
                + "——基线永不追加，新缺口唯一出路是按精度标准做动画");
        assertTrue(P0_BASELINE.containsAll(MISSING_ANIM_ALLOWLIST),
            "MISSING_ANIM_ALLOWLIST 出现 P0 冻结基线之外的条目");
        assertTrue(SKILL_ANIM.keySet().containsAll(P0_BASELINE),
            "基线含映射表之外的僵尸条目：" + P0_BASELINE.stream()
                .filter(id -> !SKILL_ANIM.containsKey(id)).toList());
    }

    /**
     * 持续维持型例外（shield_block）：循环结构 + 每轴 endTick 有帧不退化。
     * 注意此处刻意只查**帧存在**不查同值——shield_raise 是「举起后保持」语义，
     * 首尾帧值不同是既有资产的刻意设计（0→6 举起后循环维持末段姿态），与
     * 蓄力循环段的同值补帧红线（{@link #loopSeamViolations}）分属两种循环形态。
     */
    @Test
    void sustainedLoopExceptionKeepsLoopStructure() throws IOException {
        for (String skillId : SUSTAINED_LOOP_EXCEPTIONS) {
            AnimMeta meta = readAnim(SKILL_ANIM.get(skillId));
            assertTrue(meta.isLoop(),
                skillId + " 是持续维持型技能，动画应保持 isLoop=true（举盾按住持续 + "
                    + "StopAnim 停止路径）——改非循环需同步撤销本例外并过时长对拍");
            Set<String> all = new HashSet<>();
            Set<String> atEnd = new HashSet<>();
            collectAxes(meta, all, atEnd);
            assertEquals(all, atEnd,
                skillId + " 循环动画每个用到的轴必须在 endTick 有关键帧（库坑 #1 单帧衰减底线）");
        }
    }

    /** 长演出例外（guangbo_ticao）：非循环且长度不倒退（防退化成快闪）。 */
    @Test
    void longFormExceptionKeepsFullLengthNonLoop() throws IOException {
        for (String skillId : LONG_FORM_EXCEPTIONS) {
            AnimMeta meta = readAnim(SKILL_ANIM.get(skillId));
            assertFalse(meta.isLoop(), skillId + " 长演出动画应为非循环一次性播放");
            assertTrue(meta.endTick() >= 100,
                skillId + " 长演出动画 endTick=" + meta.endTick()
                    + " 低于 100——A 级基准资产不得退化（当前基准 150）");
        }
    }

    /**
     * spec manifest 断言框架（精度标准机械化，仅对本 plan 新交付动画强制）：
     * {@code bong/anim_spec_manifests/<anim>.json} 声明三段边界后逐项断言——
     * 三段各 ≥2 帧点、主轴相邻帧间隔 ≤4 tick、easing 显式且打击轴禁 linear、
     * leg.pitch ≤ 40°（0.698 rad）、循环每轴 endTick 补帧。目录随批次填充。
     */
    @Test
    void specManifestsEnforcePrecisionStandardMechanically() throws IOException {
        Path manifestDir = manifestRoot();
        assertTrue(Files.isDirectory(manifestDir),
            "spec manifest 目录缺失：" + manifestDir + "——P0 框架要求目录存在（随批次填充）");
        try (Stream<Path> files = Files.list(manifestDir)) {
            for (Path manifestFile : files
                    .filter(p -> p.getFileName().toString().endsWith(".json")).toList()) {
                assertManifestHolds(manifestFile);
            }
        }
    }

    private static Path manifestRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        for (Path candidate : List.of(
                cwd.resolve("src/test/resources/bong/anim_spec_manifests"),
                cwd.resolve("client/src/test/resources/bong/anim_spec_manifests"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException("无法定位 anim_spec_manifests 目录（user.dir=" + cwd + "）");
    }

    private static void assertManifestHolds(Path manifestFile) throws IOException {
        String animName = manifestFile.getFileName().toString().replace(".json", "");
        JsonObject manifest = JsonParser.parseString(Files.readString(manifestFile)).getAsJsonObject();
        AnimMeta meta = readAnim(animName);

        // 三段边界：anticipation/strike/recovery 的 [from, to]（tick，含端点）。
        Map<String, int[]> phases = new HashMap<>();
        for (String phase : List.of("anticipation", "strike", "recovery")) {
            assertTrue(manifest.has(phase),
                animName + " manifest 缺少 " + phase + " 段声明（三段式结构红线）");
            JsonArray range = manifest.getAsJsonArray(phase);
            phases.put(phase, new int[] {range.get(0).getAsInt(), range.get(1).getAsInt()});
        }
        // 三段区间结构校验：各段 from<to、有序不重叠（相邻可共享边界 tick=hold 衔接）、
        // 首段从 0 起、末段收在 endTick——防「三段都填 [0,endTick]」的空声明混过
        // （CodeRabbit r1 修正）。
        int[] ant = phases.get("anticipation");
        int[] strike = phases.get("strike");
        int[] recovery = phases.get("recovery");
        for (Map.Entry<String, int[]> phase : phases.entrySet()) {
            assertTrue(phase.getValue()[0] < phase.getValue()[1],
                animName + " " + phase.getKey() + " 段 [" + phase.getValue()[0] + ","
                    + phase.getValue()[1] + "] 必须 from < to");
        }
        assertEquals(0, ant[0], animName + " anticipation 必须从 tick 0 起");
        assertTrue(ant[1] <= strike[0],
            animName + " anticipation(" + ant[1] + ") 与 strike(" + strike[0]
                + ") 必须有序不重叠（≤，可共享边界 tick）");
        assertTrue(strike[1] <= recovery[0],
            animName + " strike(" + strike[1] + ") 与 recovery(" + recovery[0]
                + ") 必须有序不重叠");
        assertEquals(meta.endTick(), recovery[1],
            animName + " recovery 必须收在 endTick=" + meta.endTick() + "（收势到尾，"
                + "精度标准 #1/#2——声明短于动画会漏检尾段密度）");
        // 主打击轴清单（strike 段密度 + 禁 linear 的检查对象）。
        List<String> strikeAxes = new ArrayList<>();
        for (JsonElement axis : manifest.getAsJsonArray("strike_axes")) {
            strikeAxes.add(axis.getAsString());
        }
        assertFalse(strikeAxes.isEmpty(), animName + " manifest 必须声明至少一个主打击轴");

        Map<String, List<Integer>> axisTicks = new HashMap<>();
        Map<String, Set<String>> axisEasings = new HashMap<>();
        for (JsonElement moveElement : meta.moves()) {
            JsonObject move = moveElement.getAsJsonObject();
            int tick = move.get("tick").getAsInt();
            String easing = move.has("easing") ? move.get("easing").getAsString() : null;
            for (String part : move.keySet()) {
                if (part.equals("tick") || part.equals("easing") || part.equals("comment")) {
                    continue;
                }
                JsonElement axesElement = move.get(part);
                if (!axesElement.isJsonObject()) {
                    continue;
                }
                JsonObject axes = axesElement.getAsJsonObject();
                for (String axis : axes.keySet()) {
                    if (axis.equals("comment")) {
                        continue;
                    }
                    String key = part + "." + axis;
                    axisTicks.computeIfAbsent(key, k -> new ArrayList<>()).add(tick);
                    assertNotNull(easing,
                        animName + " tick " + tick + " 帧未显式声明 easing（精度标准 #3）");
                    axisEasings.computeIfAbsent(key, k -> new HashSet<>()).add(easing);
                    // leg.pitch ≤ 40°（弧度 0.698）——库坑 #2。
                    if ((part.contains("leg") || part.contains("Leg")) && axis.equals("pitch")) {
                        double value = Math.abs(axes.get(axis).getAsDouble());
                        assertTrue(value <= 0.699,
                            animName + " " + key + "@" + tick + " = " + value
                                + " rad 超过 40°（0.698 rad）——大幅度腿部动作由 bend 承担（库坑 #2）");
                    }
                }
            }
        }
        // 三段各 ≥2 帧点（按任意轴在段内的帧点计）。
        Set<Integer> allTicks = new HashSet<>();
        axisTicks.values().forEach(allTicks::addAll);
        for (Map.Entry<String, int[]> phase : phases.entrySet()) {
            long inPhase = allTicks.stream()
                .filter(t -> t >= phase.getValue()[0] && t <= phase.getValue()[1]).count();
            assertTrue(inPhase >= 2,
                animName + " " + phase.getKey() + " 段仅 " + inPhase + " 个帧点（应 ≥2，三段式结构红线）");
        }
        // 主打击轴：相邻帧间隔 ≤4 tick + 禁 linear。
        for (String axis : strikeAxes) {
            List<Integer> ticks = axisTicks.get(axis);
            assertNotNull(ticks, animName + " manifest 声明的主打击轴 `" + axis + "` 在动画中无关键帧");
            List<Integer> sorted = ticks.stream().sorted().distinct().toList();
            for (int i = 1; i < sorted.size(); i++) {
                assertTrue(sorted.get(i) - sorted.get(i - 1) <= 4,
                    animName + " 主打击轴 `" + axis + "` 帧点 " + sorted.get(i - 1) + "→"
                        + sorted.get(i) + " 间隔超过 4 tick（精度标准 #3 密度红线）");
            }
            Set<String> easings = axisEasings.getOrDefault(axis, Set.of());
            assertFalse(easings.stream().anyMatch(e -> e.equalsIgnoreCase("linear")),
                animName + " 主打击轴 `" + axis + "` 使用 linear easing（精度标准 #3 禁用）");
        }
        // 循环动画每轴 endTick 同值补帧（库坑 #1 完整语义：值相同，非仅存在）。
        if (meta.isLoop()) {
            List<String> seams = loopSeamViolations(meta);
            assertTrue(seams.isEmpty(),
                animName + " 循环动画违反 endTick 同值补帧（库坑 #1，循环回绕跳变）：" + seams);
        }
    }
}
