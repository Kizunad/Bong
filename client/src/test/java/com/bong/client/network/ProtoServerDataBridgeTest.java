package com.bong.client.network;

import bong.Common;
import bong.Envelope;
import com.bong.client.combat.DefenseWindowState;
import com.bong.client.combat.DefenseWindowStore;
import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.combat.store.FullPowerStateStore;
import com.bong.client.combat.store.WoundsStore;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.hud.PillBuffHudPlanner;
import com.bong.client.hud.PoisonTraitHudStateStore;
import com.bong.client.hud.BongToast;
import com.bong.client.social.NicheGuardianStore;
import com.bong.client.social.SocialStateStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.protobuf.Descriptors;
import com.google.protobuf.DynamicMessage;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.*;

class ProtoServerDataBridgeTest {

    @AfterEach
    void tearDown() {
        PillBuffHudPlanner.clear();
        TechniquesListPanel.resetForTests();
        BongToast.resetForTests();
        UnifiedEventStore.resetForTests();
        com.bong.client.coffin.TutorialCoffinPosStore.resetForTests();
        SocialStateStore.resetForTests();
        NicheGuardianStore.resetForTests();
        LootContainerStateStore.clear();
        DefenseWindowStore.resetForTests();
        DeathStateStore.resetForTests();
        FullPowerStateStore.resetForTests();
        WoundsStore.resetForTests();
        FalseSkinHudStateStore.resetForTests();
        PoisonTraitHudStateStore.clear();
        com.bong.client.craft.CraftStore.clear();
        com.bong.client.gathering.GatheringSessionStore.resetForTests();
        com.bong.client.insight.InsightOfferStore.resetForTests();
        com.bong.client.scroll.ScrollReadStore.resetForTests();
    }

    // ─── Happy path: Welcome ─────────────────────────────────────────
    @Test
    void bridgeWelcomeProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setWelcome(Envelope.Welcome.newBuilder()
                        .setMessage("Bong server connected"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for Welcome: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt(), "version should be 1");
        assertEquals("welcome", json.get("type").getAsString(), "type should be 'welcome'");
        assertEquals("Bong server connected", json.get("message").getAsString());
    }

    // ─── Happy path: Heartbeat ───────────────────────────────────────
    @Test
    void bridgeHeartbeatProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setHeartbeat(Envelope.Heartbeat.newBuilder()
                        .setMessage("mock agent tick"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt());
        assertEquals("heartbeat", json.get("type").getAsString());
        assertEquals("mock agent tick", json.get("message").getAsString());
    }

    // ─── Happy path: ZoneInfo ────────────────────────────────────────
    @Test
    void bridgeZoneInfoProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setZoneInfo(Envelope.ZoneInfo.newBuilder()
                        .setZone("blood_valley")
                        .setSpiritQi(-0.42)
                        .setDangerLevel(3))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt());
        assertEquals("zone_info", json.get("type").getAsString());
        assertEquals("blood_valley", json.get("zone").getAsString());
    }

    // ─── Happy path: EventAlert ──────────────────────────────────────
    @Test
    void bridgeEventAlertProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventAlert(Envelope.EventAlert.newBuilder()
                        .setEvent(Envelope.EventKind.EVENT_KIND_THUNDER_TRIBULATION)
                        .setMessage("watch out")
                        .setZone("spawn")
                        .setDurationTicks(100))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("event_alert", json.get("type").getAsString());
        assertEquals("watch out", json.get("message").getAsString());
    }

    // ─── Happy path: Narration ───────────────────────────────────────
    @Test
    void bridgeNarrationProducesLegacyJson() {
        Envelope.NarrationEntry narration = Envelope.NarrationEntry.newBuilder()
                .setText("The sky darkens")
                .setScope("broadcast")
                .setStyle("narration")
                .build();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setNarration(Envelope.NarrationBatch.newBuilder()
                        .addNarrations(narration))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("narration", json.get("type").getAsString());
        assertTrue(json.has("narrations"), "should have narrations field");
        assertEquals(1, json.getAsJsonArray("narrations").size());
    }

    // ─── Happy path: CombatHudState ──────────────────────────────────
    @Test
    void bridgeCombatHudStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCombatHudState(Envelope.CombatHudState.newBuilder()
                        .setHpPercent(0.75f)
                        .setQiPercent(0.5f)
                        .setStaminaPercent(1.0f))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("combat_hud_state", json.get("type").getAsString());
    }

    // ─── Happy path: CoffinState ─────────────────────────────────────
    @Test
    void bridgeCoffinStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCoffinState(Envelope.CoffinState.newBuilder()
                        .setInCoffin(true)
                        .setLifespanRateMultiplier(0.5))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("coffin_state", json.get("type").getAsString());
        assertTrue(json.get("in_coffin").getAsBoolean());
    }

    // ─── Happy path: UiOpen ──────────────────────────────────────────
    @Test
    void bridgeUiOpenProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setUiOpen(Envelope.UiOpen.newBuilder()
                        .setUi("cultivation_panel")
                        .setXml("<flow-layout/>"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("ui_open", json.get("type").getAsString());
        assertEquals("cultivation_panel", json.get("ui").getAsString());
    }

    // ─── Happy path: DeathScreen ─────────────────────────────────────
    @Test
    void bridgeDeathScreenProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDeathScreen(Envelope.DeathScreen.newBuilder()
                        .setStage(Envelope.DeathScreenStage.DEATH_SCREEN_STAGE_FORTUNE)
                        .setZoneKind(Envelope.DeathScreenZoneKind.DEATH_SCREEN_ZONE_KIND_ORDINARY)
                        .setCause("karma_backlash")
                        .setLuckRemaining(0.5))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("death_screen", json.get("type").getAsString());
        assertEquals("fortune", json.get("stage").getAsString(),
                "stage 必须从 DEATH_SCREEN_STAGE_FORTUNE 剥成 'fortune'（DeathScreen.phaseLabel 期望），"
                + "否则死亡界面阶段标签永远落 default '重生判定'");
        assertEquals("ordinary", json.get("zone_kind").getAsString(),
                "zone_kind 必须从 DEATH_SCREEN_ZONE_KIND_ORDINARY 剥成 'ordinary'（DeathScreen.zoneLabel 期望）");
    }

    // ─── death_screen: 顶层 stage/zone_kind + 嵌套 cinematic 枚举剥前缀 ──
    // 死亡界面阶段标签 + cinematic 过场推进的接收前提：proto3 JSON 把所有 death 枚举
    // 打成全名前缀，各消费方只认 serde 小写。锁住顶层与嵌套两层。
    @Test
    void bridgeDeathScreenStripsTopLevelAndCinematicEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDeathScreen(Envelope.DeathScreen.newBuilder()
                        .setVisible(true)
                        .setCause("pk")
                        .setStage(Envelope.DeathScreenStage.DEATH_SCREEN_STAGE_TRIBULATION)
                        .setZoneKind(Envelope.DeathScreenZoneKind.DEATH_SCREEN_ZONE_KIND_NEGATIVE)
                        .setCinematic(Envelope.DeathCinematicData.newBuilder()
                                .setV(1)
                                .setCharacterId("char-1")
                                .setPhase(Envelope.DeathCinematicPhase.DEATH_CINEMATIC_PHASE_ROLL)
                                .setZoneKind(Envelope.DeathCinematicZoneKind.DEATH_CINEMATIC_ZONE_KIND_DEATH)
                                .setRoll(Envelope.DeathCinematicRoll.newBuilder()
                                        .setResult(Envelope.DeathRollResult.DEATH_ROLL_RESULT_SURVIVE))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("death_screen", json.get("type").getAsString());
        assertEquals("tribulation", json.get("stage").getAsString(),
                "顶层 stage 剥成 'tribulation'（DeathScreen.phaseLabel）");
        assertEquals("negative", json.get("zone_kind").getAsString(),
                "顶层 zone_kind 剥成 'negative'（DeathScreen.zoneLabel）");
        JsonObject cinematic = json.getAsJsonObject("cinematic");
        assertEquals("roll", cinematic.get("phase").getAsString(),
                "cinematic.phase 剥成 'roll'（DeathCinematicState.Phase.fromWire），否则过场永远卡 PREDEATH");
        assertEquals("death", cinematic.get("zone_kind").getAsString(),
                "cinematic.zone_kind 一并归一化为 'death' 保持桥输出统一");
        assertEquals("survive", cinematic.getAsJsonObject("roll").get("result").getAsString(),
                "cinematic.roll.result 剥成 'survive'（DeathCinematicState.RollResult.fromWire）");
    }

    // ─── Happy path: TsyCollapseStarted maps to tsy_collapse_started_ipc ──
    @Test
    void bridgeTsyCollapseStartedMapsToCorrectTypeString() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTsyCollapseStarted(Envelope.TsyCollapseStartedIpc.newBuilder()
                        .setFamilyId("tsy_001"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("tsy_collapse_started_ipc", json.get("type").getAsString(),
                "TSY_COLLAPSE_STARTED should map to 'tsy_collapse_started_ipc'");
    }

    // ─── Error: empty envelope (PAYLOAD_NOT_SET) ─────────────────────
    @Test
    void bridgeEmptyEnvelopeReturnsError() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder().build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertFalse(result.isSuccess());
        assertTrue(result.errorMessage().contains("no payload set"),
                "error should mention missing payload: " + result.errorMessage());
    }

    // ─── Error: invalid bytes ────────────────────────────────────────
    @Test
    void bridgeGarbageBytesReturnsError() {
        byte[] garbage = new byte[]{(byte) 0xFF, (byte) 0xFE, 0x00, 0x01};

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(garbage);
        // Proto may or may not parse garbage as a valid message. Either error or
        // empty payload are acceptable.
        if (result.isSuccess()) {
            // If it somehow parsed, the JSON should at least have v and type
            JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
            assertEquals(1, json.get("v").getAsInt());
        }
    }

    // ─── Mapped case count matches expected ──────────────────────────
    @Test
    void mappedCaseCountMatchesExpectedVariants() {
        // We expect mappings for all S2C payload variants that have client handlers.
        // Excludes: PAYLOAD_NOT_SET and channel-specific variants (VFX_EVENT, AUDIO_*, etc.)
        int count = ProtoServerDataBridge.mappedCaseCount();
        assertTrue(count >= 100,
                "expected at least 100 mapped cases, got " + count +
                " — did you forget to add a new PayloadCase→type mapping?");
    }

    // ═══════════════════════════════════════════════════════════════════
    // F18: PayloadCase 穷尽保护（编译期/测试期）
    //
    // CASE_TO_TYPE（手写 m.put）与 extractInner switch 此前均未对
    // PayloadCase.values() 做穷尽校验——PayloadCase 是闭合 enum（proto3 oneof 无
    // UNRECOGNIZED），漏映射新 variant 会在运行期静默丢包（bridge() 返回
    // "unmapped PayloadCase: X" 或 "failed to extract inner message for X"，
    // 上层只 LOGGER.warn，玩家侧对应功能悄无声息地失效）。
    //
    // 本测试反射遍历 proto schema 里 ServerDataEnvelope.payload oneof 的每个字段，
    // 逐个构造"只设置该字段、内容为默认值"的最小 envelope，喂给 bridge()，断言
    // 结果不是那两种"漏映射"专属错误信息。之所以不要求每个 case 都 isSuccess()：
    // 少数 variant（CRAFT_OUTCOME / INVENTORY_EVENT）自身还嵌套一层 oneof，默认值
    // 下那层 oneof 未设置，会走 "has no oneof variant set" 分支报错——这是预期行为
    // （测的是"映射表是否穷尽"，不是"每个 variant 塞空数据也必须成功桥接"）。
    //
    // 排除清单（13 个）：
    //   - VFX_EVENT / AUDIO_PLAY_EVENT / AUDIO_STOP_EVENT / AMBIENT_ZONE_EVENT /
    //     ZONE_ENVIRONMENT_STATE：各自走独立 CustomPayload channel
    //     （bong:vfx_event / bong:audio/play / bong:audio/stop /
    //     bong:audio/ambient_zone / bong:zone_environment，见 BongNetworkHandler
    //     registerGlobalReceiver），从不经过 bong:server_data / ServerDataEnvelope。
    //   - MUTATION_STATE / MUTATION_EVENT / DANDAO_STYLE / TSY_ENTER_EVENT /
    //     TSY_EXIT_EVENT / TSY_NPC_SPAWNED / TSY_SENTINEL_PHASE_CHANGED：
    //     全仓（含 main 与 test）零 getter 引用，proto schema 预留字段，尚无 client
    //     消费方——接线时需同时补 CASE_TO_TYPE / extractInner 并从此排除清单移除。
    //   - FACTION_WAR_STATE（plan-wire-format-bridge-v1 P5，2026-07-03 摘除）：
    //     proto 消息仍在，但 #667「移除涌现冲突战事 HUD」已拆掉 client 侧 HUD
    //     handler/planner/store，server 侧 npc/war/settle.rs 也不再构造/广播该
    //     payload（生产路径零引用）。此前 CASE_TO_TYPE 里保留了映射，bridge()
    //     会成功转出一份没有任何 handler 消费的 JSON——本条从映射摘除，改走
    //     "unmapped PayloadCase" 错误路径，见 bridgeFactionWarStateIsIntentionallyUnmapped。
    // ═══════════════════════════════════════════════════════════════════

    private static final Set<Envelope.ServerDataEnvelope.PayloadCase> KNOWN_UNMAPPED_PAYLOAD_CASES =
            EnumSet.of(
                    Envelope.ServerDataEnvelope.PayloadCase.VFX_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.AUDIO_PLAY_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.AUDIO_STOP_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.AMBIENT_ZONE_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.ZONE_ENVIRONMENT_STATE,
                    Envelope.ServerDataEnvelope.PayloadCase.MUTATION_STATE,
                    Envelope.ServerDataEnvelope.PayloadCase.MUTATION_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.DANDAO_STYLE,
                    Envelope.ServerDataEnvelope.PayloadCase.TSY_ENTER_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.TSY_EXIT_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.TSY_NPC_SPAWNED,
                    Envelope.ServerDataEnvelope.PayloadCase.TSY_SENTINEL_PHASE_CHANGED,
                    Envelope.ServerDataEnvelope.PayloadCase.FACTION_WAR_STATE
            );

    @Test
    void everyPayloadCaseIsMapped_orIsOnTheExplicitExclusionList() {
        Descriptors.OneofDescriptor payloadOneof = payloadOneofDescriptor();
        assertNotNull(payloadOneof,
                "proto schema 里找不到 ServerDataEnvelope 的 'payload' oneof —— proto 生成物结构是否变了？");

        List<String> mappingGaps = new ArrayList<>();
        int exercised = 0;

        for (Descriptors.FieldDescriptor field : payloadOneof.getFields()) {
            Envelope.ServerDataEnvelope.PayloadCase payloadCase =
                    Envelope.ServerDataEnvelope.PayloadCase.forNumber(field.getNumber());
            assertNotNull(payloadCase,
                    "proto 字段 " + field.getFullName() + " (号=" + field.getNumber()
                            + ") 找不到对应 PayloadCase —— 生成代码与 .proto 是否不同步？");
            if (KNOWN_UNMAPPED_PAYLOAD_CASES.contains(payloadCase)) {
                continue;
            }
            exercised++;

            // 构造"该 oneof 字段=消息类型默认值，其余字段不设"的最小 envelope。
            Envelope.ServerDataEnvelope envelope = (Envelope.ServerDataEnvelope)
                    Envelope.ServerDataEnvelope.newBuilder()
                            .setField(field, DynamicMessage.getDefaultInstance(field.getMessageType()))
                            .build();

            ProtoServerDataBridge.BridgeResult result =
                    ProtoServerDataBridge.bridge(envelope.toByteArray());

            if (!result.isSuccess()) {
                String err = result.errorMessage() == null ? "" : result.errorMessage();
                boolean isMappingGap = err.startsWith("unmapped PayloadCase:")
                        || err.startsWith("failed to extract inner message for ");
                if (isMappingGap) {
                    mappingGaps.add(payloadCase.name() + " → " + err);
                }
                // 非映射缺口的失败（如嵌套 oneof 默认值未设变体）不是本测试关心的问题，忽略。
            }
        }

        assertTrue(exercised >= 100,
                "期望穷举到 >=100 个非排除 PayloadCase（proto schema 变小了，还是排除清单配错了？），实际="
                        + exercised);
        assertTrue(mappingGaps.isEmpty(),
                "以下 PayloadCase 在 ProtoServerDataBridge 里存在映射缺口"
                        + "（CASE_TO_TYPE 或 extractInner 漏收录该 variant，新数据会在运行期静默丢包，"
                        + "上层只 LOGGER.warn，对应功能悄无声息失效）。\n"
                        + "若这是刻意排除的 channel-specific / 尚未接线 variant，"
                        + "把它加进本测试的 KNOWN_UNMAPPED_PAYLOAD_CASES 并写明理由；"
                        + "否则请在 ProtoServerDataBridge.CASE_TO_TYPE 静态块和 extractInner switch 里补上映射：\n"
                        + String.join("\n", mappingGaps));
    }

    @Test
    void knownUnmappedPayloadCasesExclusionList_matchesActualGapCount() {
        // 回归 pin：排除清单大小必须精确等于"proto oneof 字段数 − CASE_TO_TYPE 映射数"，
        // 防止有人往排除清单里顺手多塞一个其实已经映射好的 case（掩盖真实的映射缺口）。
        Descriptors.OneofDescriptor payloadOneof = payloadOneofDescriptor();
        int totalFields = payloadOneof.getFields().size();
        int mapped = ProtoServerDataBridge.mappedCaseCount();
        assertEquals(totalFields - mapped, KNOWN_UNMAPPED_PAYLOAD_CASES.size(),
                "排除清单大小(" + KNOWN_UNMAPPED_PAYLOAD_CASES.size() + ")应精确等于 proto 字段总数("
                        + totalFields + ") − CASE_TO_TYPE 映射数(" + mapped
                        + ")。不一致说明排除清单本身配错了（多排或漏排）。");
    }

    // ═══════════════════════════════════════════════════════════════════
    // P5 round-trip 守卫（plan-wire-format-bridge-v1 §P5，长效防复发 pin）
    //
    // 上面的 everyPayloadCaseIsMapped_orIsOnTheExplicitExclusionList 只验证
    // "CASE_TO_TYPE / extractInner 是否穷尽覆盖所有 PayloadCase"，喂的是*默认值*
    // 消息，不触碰 handler 语义（很多 handler 在字段全默认时本就该 noOp，测不出
    // 问题）。本节补 handler 侧：用反射把每个已映射 payloadCase 的内部消息全部
    // 标量/枚举字段填成非默认值（递归子消息，跳过 map 字段），喂给 bridge() →
    // 断言 isSuccess()，再喂给 ServerDataRouter 默认路由 → 断言路由结果不是
    // noOp（除非在 NOOP_EXPECTED_WITH_NON_DEFAULT_DATA 里显式登记原因）。
    // 目标：未来 server 改 proto 形状 / 加新枚举 variant / handler 改判定逻辑
    // 若导致某条链路悄悄退化成 noOp，这里立刻撞红——这是本 plan 的长效价值。
    // ═══════════════════════════════════════════════════════════════════

    /**
     * 少数 payloadCase 即便塞入"每个标量字段都非默认值"的数据，其 handler 仍会
     * 合法地判定 noOp —— 原因不是桥接/映射缺陷，而是 handler 自身的业务前置
     * 条件（例如需要跨调用维护的会话态、多个字段间的一致性校验）。逐条登记，
     * 防止真回归混进来被当作"预期 noOp"放过。
     *
     * <p>本测试的通用 fuzz 填法（{@link #nonDefaultScalarOrMessage}）对每个
     * string/enum-like 字段一律填 {@code "rt_probe_" + fieldName}，不了解具体
     * 业务语义，故以下 18 条在真实数据下（P0–P4 逐条 fixture 已锁）是正常工作
     * 的，这里 noOp 纯属"通用假数据没通过业务校验"，不是 wire 契约缺陷：
     * <ul>
     *   <li>{@code INVENTORY_SNAPSHOT} / {@code SKILL_SNAPSHOT} / {@code BREAKTHROUGH_CINEMATIC} /
     *       {@code SPIRIT_TREASURE_STATE} / {@code SPIRIT_TREASURE_DIALOGUE} / {@code COMBAT_HUD_STATE}：
     *       要求内部字段满足业务不变式（已知技能 id 集合、槽位形状等），随机 fuzz 值不满足。</li>
     *   <li>{@code QUICK_SLOT_CONFIG} / {@code SKILL_BAR_CONFIG}：entry 数组要求定长 9（快捷栏槽位数），
     *       fuzz 只填了 1 个 repeated 元素。</li>
     *   <li>{@code TECHNIQUE_PROFICIENCY_UPDATE}：proficiency/gain 要求落在 [0,1]，fuzz 填 7.0f 越界。</li>
     *   <li>{@code SKILL_CONFIG_SNAPSHOT}：config 字段要求嵌套 JSON object 形状，fuzz 只填了标量字符串。</li>
     *   <li>{@code UI_OPEN}：dynamic XML 路径在测试环境被显式禁用（与本条数据内容无关的全局开关）。</li>
     *   <li>{@code INVENTORY_EVENT}：handler 要求先有权威 inventory snapshot 才接受增量事件，
     *       单测未先灌 snapshot。</li>
     *   <li>{@code DROPPED_LOOT_SYNC} / {@code LOOT_CONTAINER_CLOSE} / {@code ANQI_HUD} / {@code ZHENMAI_HUD}：
     *       字段是纯 string（非 proto enum，未走 stripEnumPrefix 桥层），
     *       handler 自己维护一份已知 id/kind/reason 白名单，fuzz 字符串命中不了。</li>
     *   <li>{@code MINERAL_PROBE_RESULT} / {@code WORKBENCH_OPEN}：handler 需要本地玩家实例
     *       （{@code MinecraftClient.player}），单测环境没有真实客户端会话。</li>
     * </ul>
     */
    private static final Set<Envelope.ServerDataEnvelope.PayloadCase> NOOP_EXPECTED_WITH_NON_DEFAULT_DATA =
            EnumSet.of(
                    Envelope.ServerDataEnvelope.PayloadCase.INVENTORY_SNAPSHOT,
                    Envelope.ServerDataEnvelope.PayloadCase.COMBAT_HUD_STATE,
                    Envelope.ServerDataEnvelope.PayloadCase.QUICK_SLOT_CONFIG,
                    Envelope.ServerDataEnvelope.PayloadCase.SKILL_BAR_CONFIG,
                    Envelope.ServerDataEnvelope.PayloadCase.TECHNIQUE_PROFICIENCY_UPDATE,
                    Envelope.ServerDataEnvelope.PayloadCase.SKILL_CONFIG_SNAPSHOT,
                    Envelope.ServerDataEnvelope.PayloadCase.BREAKTHROUGH_CINEMATIC,
                    Envelope.ServerDataEnvelope.PayloadCase.UI_OPEN,
                    Envelope.ServerDataEnvelope.PayloadCase.INVENTORY_EVENT,
                    Envelope.ServerDataEnvelope.PayloadCase.DROPPED_LOOT_SYNC,
                    Envelope.ServerDataEnvelope.PayloadCase.SKILL_SNAPSHOT,
                    Envelope.ServerDataEnvelope.PayloadCase.SPIRIT_TREASURE_STATE,
                    Envelope.ServerDataEnvelope.PayloadCase.SPIRIT_TREASURE_DIALOGUE,
                    Envelope.ServerDataEnvelope.PayloadCase.LOOT_CONTAINER_CLOSE,
                    Envelope.ServerDataEnvelope.PayloadCase.ANQI_HUD,
                    Envelope.ServerDataEnvelope.PayloadCase.MINERAL_PROBE_RESULT,
                    Envelope.ServerDataEnvelope.PayloadCase.WORKBENCH_OPEN,
                    Envelope.ServerDataEnvelope.PayloadCase.ZHENMAI_HUD
            );

    @Test
    void everyMappedPayloadCaseRoundTripsIntoNonNoOpHandlerDispatch() {
        Descriptors.OneofDescriptor payloadOneof = payloadOneofDescriptor();
        ServerDataRouter router = ServerDataRouter.createDefault();

        List<String> bridgeFailures = new ArrayList<>();
        List<String> noOpRegressions = new ArrayList<>();
        int exercised = 0;

        for (Descriptors.FieldDescriptor field : payloadOneof.getFields()) {
            Envelope.ServerDataEnvelope.PayloadCase payloadCase =
                    Envelope.ServerDataEnvelope.PayloadCase.forNumber(field.getNumber());
            if (KNOWN_UNMAPPED_PAYLOAD_CASES.contains(payloadCase)) {
                continue;
            }
            exercised++;

            DynamicMessage.Builder inner = DynamicMessage.newBuilder(field.getMessageType());
            populateNonDefault(inner, 0);

            Envelope.ServerDataEnvelope envelope = (Envelope.ServerDataEnvelope)
                    Envelope.ServerDataEnvelope.newBuilder()
                            .setField(field, inner.build())
                            .build();

            ProtoServerDataBridge.BridgeResult result =
                    ProtoServerDataBridge.bridge(envelope.toByteArray());

            if (!result.isSuccess()) {
                bridgeFailures.add(payloadCase.name() + " → bridge() failed: " + result.errorMessage());
                continue;
            }

            ServerDataRouter.RouteResult route = router.route(result.legacyJson(),
                    result.legacyJson().getBytes(StandardCharsets.UTF_8).length);

            if (route.isNoOp() && !NOOP_EXPECTED_WITH_NON_DEFAULT_DATA.contains(payloadCase)) {
                noOpRegressions.add(payloadCase.name() + " → " + route.logMessage());
            }
        }

        assertTrue(exercised >= 100,
                "期望穷举到 >=100 个已映射 PayloadCase，实际=" + exercised
                        + "（KNOWN_UNMAPPED_PAYLOAD_CASES 配错了？）");
        assertTrue(bridgeFailures.isEmpty(),
                "以下已映射 PayloadCase 喂入非默认值数据后 bridge() 转换失败"
                        + "（proto 形状/桥层 fixup 与 handler 期望脱节）：\n"
                        + String.join("\n", bridgeFailures));
        assertTrue(noOpRegressions.isEmpty(),
                "以下已映射 PayloadCase 喂入非默认值数据后路由到的 handler 仍判定 noOp"
                        + "（说明桥层枚举/坐标/字段名 fixup 没接上，或 handler 判定逻辑与 wire "
                        + "形状脱节；若这是 handler 本身合法的业务前置条件而非 bug，"
                        + "把它加进 NOOP_EXPECTED_WITH_NON_DEFAULT_DATA 并写明理由）：\n"
                        + String.join("\n", noOpRegressions));
    }

    /**
     * 递归把 {@code builder} 的每个字段（标量/枚举/子消息）填成非默认值，供
     * round-trip 守卫构造"内容饱满"的测试消息。跳过 map 字段（本仓库 wire
     * 契约里 map 用量很少，且 map 的 key/value 语义因字段而异，通用填法收益
     * 低于复杂度）；深度封顶防御万一出现自引用消息类型导致死循环。
     */
    private static void populateNonDefault(com.google.protobuf.Message.Builder builder, int depth) {
        if (depth > 6) {
            return;
        }
        for (Descriptors.FieldDescriptor field : builder.getDescriptorForType().getFields()) {
            if (field.isMapField()) {
                continue;
            }
            if (field.isRepeated()) {
                builder.addRepeatedField(field, nonDefaultScalarOrMessage(field, depth));
            } else {
                builder.setField(field, nonDefaultScalarOrMessage(field, depth));
            }
        }
    }

    private static Object nonDefaultScalarOrMessage(Descriptors.FieldDescriptor field, int depth) {
        switch (field.getJavaType()) {
            case INT:
                return 7;
            case LONG:
                return 7L;
            case FLOAT:
                return 1.5f;
            case DOUBLE:
                return 1.5d;
            case BOOLEAN:
                return true;
            case STRING:
                return "rt_probe_" + field.getName();
            case BYTE_STRING:
                return com.google.protobuf.ByteString.copyFromUtf8("rt_probe");
            case ENUM: {
                for (Descriptors.EnumValueDescriptor value : field.getEnumType().getValues()) {
                    if (value.getNumber() != 0) {
                        return value;
                    }
                }
                return field.getEnumType().getValues().get(0);
            }
            case MESSAGE: {
                DynamicMessage.Builder sub = DynamicMessage.newBuilder(field.getMessageType());
                populateNonDefault(sub, depth + 1);
                return sub.build();
            }
            default:
                throw new IllegalStateException("unhandled proto JavaType " + field.getJavaType()
                        + " for field " + field.getFullName());
        }
    }

    // ─── Faction war state: intentionally unmapped (plan-wire-format-bridge-v1 P5) ──
    @Test
    void bridgeFactionWarStateIsIntentionallyUnmapped() {
        // #667「移除涌现冲突战事 HUD」拆掉了 client 侧 HUD handler/planner/store，
        // server 侧 npc/war/settle.rs 也不再构造/广播这个 payload（生产路径零引用）。
        // FACTION_WAR_STATE 已从 CASE_TO_TYPE / extractInner 摘除并入
        // KNOWN_UNMAPPED_PAYLOAD_CASES —— bridge() 必须走 "unmapped PayloadCase"
        // 错误分支，而不是悄悄转出一份没人消费的 JSON。任何未来重新接线该 feature
        // 都必须同时补 CASE_TO_TYPE / extractInner 并把此 case 从排除清单移除，
        // 届时这条测试会先撞红提醒。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFactionWarState(Envelope.FactionWarState.newBuilder()
                        .setWarId(42)
                        .setZone("blood_valley")
                        .setRegionDescriptor("残灰谷")
                        .setPhase("skirmish")
                        .addGroups(1)
                        .addGroups(2)
                        .setEnlistCount(3)
                        .setMercenaryCount(4)
                        .setInterceptCount(5)
                        .setSpectateCount(6)
                        .setWinnerGroup(-1)
                        .setLoserGroup(-1))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertFalse(result.isSuccess(),
                "faction_war_state should no longer bridge — feature has no client consumer, expected 'unmapped PayloadCase' error");
        assertNotNull(result.errorMessage());
        assertTrue(result.errorMessage().startsWith("unmapped PayloadCase:"),
                "expected 'unmapped PayloadCase' error for faction_war_state, got: " + result.errorMessage());
    }

    // ─── FullPower variants map to correct type strings ──────────────
    @Test
    void fullPowerChargingMapsToChargingState() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFullPowerCharging(Envelope.FullPowerChargingState.newBuilder()
                        .setCasterUuid("offline:Kiz"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("full_power_charging_state", json.get("type").getAsString());
    }

    @Test
    void fullPowerExhaustedMapsToExhaustedState() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFullPowerExhausted(Envelope.FullPowerExhaustedState.newBuilder()
                        .setCasterUuid("offline:Kiz"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("full_power_exhausted_state", json.get("type").getAsString());
    }

    // ─── QuickSlotConfig maps to quickslot_config (not quick_slot_config) ──
    @Test
    void quickSlotConfigMapsToCorrectTypeString() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQuickSlotConfig(Envelope.QuickSlotConfig.newBuilder())
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("quickslot_config", json.get("type").getAsString(),
                "QUICK_SLOT_CONFIG should map to 'quickslot_config' (no underscore between quick and slot)");
    }

    // ─── SkillBarConfig maps to skillbar_config ──────────────────────
    @Test
    void skillBarConfigMapsToCorrectTypeString() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillBarConfig(Envelope.SkillBarConfig.newBuilder())
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("skillbar_config", json.get("type").getAsString(),
                "SKILL_BAR_CONFIG should map to 'skillbar_config'");
    }

    // ─── CombatEventFloater maps to combat_event ─────────────────────
    @Test
    void combatEventFloaterMapsToCombatEvent() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCombatEventFloater(Envelope.CombatEventFloater.newBuilder())
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("combat_event", json.get("type").getAsString(),
                "COMBAT_EVENT_FLOATER should map to 'combat_event'");
    }

    // ─── InventorySnapshot bridges correctly ─────────────────────────
    @Test
    void bridgeInventorySnapshotProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(42))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("inventory_snapshot", json.get("type").getAsString());
    }

    // ─── PlayerState bridges correctly ───────────────────────────────
    @Test
    void bridgePlayerStateProducesLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPlayerState(Envelope.PlayerState.newBuilder()
                        .setPlayer("offline:Steve")
                        .setRealm(Common.Realm.REALM_INDUCE)
                        .setSpiritQi(78.0)
                        .setKarma(0.2)
                        .setCompositePower(0.35)
                        .setZone("blood_valley"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("player_state", json.get("type").getAsString());
        assertEquals("offline:Steve", json.get("player").getAsString());
        assertEquals("blood_valley", json.get("zone").getAsString());
    }

    @Test
    void bridgeShieldBrokenProducesLegacyJsonAndRoutes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setShieldBroken(Envelope.ShieldBroken.newBuilder()
                        .setInstanceId(7)
                        .setTemplateId("wooden_shield"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for shield_broken: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("shield_broken", json.get("type").getAsString());
        assertEquals(7, json.get("instance_id").getAsLong());
        assertEquals("wooden_shield", json.get("template_id").getAsString());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault().route(result.legacyJson(), 0);
        assertTrue(route.isHandled(), "shield_broken proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "shield_broken proto bridge output must not become no-op");
    }

    @Test
    void bridgeShieldBlockHitProducesLegacyJsonAndRoutes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setShieldBlockHit(Envelope.ShieldBlockHit.newBuilder()
                        .setTemplateId("bone_shield"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for shield_block_hit: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("shield_block_hit", json.get("type").getAsString());
        assertEquals("bone_shield", json.get("template_id").getAsString());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault().route(result.legacyJson(), 0);
        assertTrue(route.isHandled(), "shield_block_hit proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "shield_block_hit proto bridge output must not become no-op");
    }

    @Test
    void bridgePillBuffStatusProducesLegacyJsonAndRoutesIntoHudPlanner() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPillBuffStatus(Envelope.PillBuffStatus.newBuilder()
                        .setBuffId("tie_bi_san")
                        .setRemainingTicks(1800)
                        .setEffectMultiplier(1.25))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for pill_buff_status: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("pill_buff_status", json.get("type").getAsString());
        assertEquals("tie_bi_san", json.get("buff_id").getAsString());
        assertEquals(1800, json.get("remaining_ticks").getAsInt());
        assertEquals(1.25, json.get("effect_multiplier").getAsDouble(), 1e-9);

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertTrue(route.isHandled(), "pill_buff_status proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "pill_buff_status proto bridge output must not become no-op");
        var buffs = PillBuffHudPlanner.activeBuffs();
        assertEquals(1, buffs.size());
        assertEquals("tie_bi_san", buffs.get(0).buffId());
        assertEquals(1800, buffs.get(0).remainingTicks());
        assertEquals(1.25, buffs.get(0).effectMultiplier(), 1e-9);
    }

    @Test
    void bridgeTechniqueProficiencyUpdateProducesLegacyJsonAndRoutesIntoStore() {
        TechniquesListPanel.replace(java.util.List.of(new TechniquesListPanel.Technique(
                "woliu.vortex",
                "绝灵涡流",
                java.util.List.of(),
                TechniquesListPanel.Grade.YELLOW,
                0.10f,
                "",
                true,
                "",
                "",
                "凝脉一层",
                java.util.List.of(),
                0.4f,
                8,
                60,
                4.0f)));
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTechniqueProficiencyUpdate(Envelope.TechniqueProficiencyUpdate.newBuilder()
                        .setTechniqueId("woliu.vortex")
                        .setProficiency(0.42f)
                        .setGain(0.02f))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for technique_proficiency_update: " + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("technique_proficiency_update", json.get("type").getAsString());
        assertEquals("woliu.vortex", json.get("technique_id").getAsString());
        assertEquals(0.42f, json.get("proficiency").getAsFloat(), 1e-6f);
        assertEquals(0.02f, json.get("gain").getAsFloat(), 1e-6f);

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertTrue(route.isHandled(), "technique_proficiency_update proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "technique_proficiency_update proto bridge output must not become no-op");
        var technique = TechniquesListPanel.snapshot().get(0);
        assertEquals("woliu.vortex", technique.id());
        assertEquals(0.42f, technique.proficiency(), 1e-6f);
    }

    // ═══════════════════════════════════════════════════════════════════
    // printAndNormalize: proto3 uint64 string → JSON number
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void uint64FieldsAreNormalizedToJsonNumbers() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(999)
                        .setBoneCoins(12345))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("revision").isJsonPrimitive(), "revision should be a primitive");
        assertTrue(json.get("revision").getAsJsonPrimitive().isNumber(),
                "revision should be a number (not string) after normalization");
        assertEquals(999, json.get("revision").getAsLong());
        assertEquals(12345, json.get("bone_coins").getAsLong());
    }

    @Test
    void zeroValueUint64IsPreservedByIncludingDefaultValueFields() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(0)
                        .setBoneCoins(0))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.has("revision"),
                "revision=0 must be present (includingDefaultValueFields)");
        assertEquals(0, json.get("revision").getAsLong());
        assertTrue(json.has("bone_coins"),
                "bone_coins=0 must be present (includingDefaultValueFields)");
        assertEquals(0, json.get("bone_coins").getAsLong());
    }

    @Test
    void negativeInt64NormalizedToNumber() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setZoneInfo(Envelope.ZoneInfo.newBuilder()
                        .setZone("test")
                        .setSpiritQi(-3.14)
                        .setDangerLevel(-1))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("danger_level").getAsJsonPrimitive().isNumber(),
                "negative int should be a JSON number");
        assertEquals(-1, json.get("danger_level").getAsInt());
    }

    @Test
    void largeUint64NearMaxLongNormalizedCorrectly() {
        long nearMax = Long.MAX_VALUE - 1;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(nearMax))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("revision").getAsJsonPrimitive().isNumber(),
                "near-max long should still be a JSON number");
        assertEquals(nearMax, json.get("revision").getAsLong());
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeOneofFlat: InventoryEvent — all 4 variants
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void inventoryEventMovedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setMoved(Envelope.InventoryEventMoved.newBuilder()
                                .setRevision(7)
                                .setInstanceId(42)
                                .setFrom(Envelope.InventoryLocation.newBuilder()
                                        .setContainer(Envelope.InventoryLocationContainer.newBuilder()
                                                .setContainerId("backpack").setRow(0).setCol(1)))
                                .setTo(Envelope.InventoryLocation.newBuilder()
                                        .setEquip(Envelope.InventoryLocationEquip.newBuilder()
                                                .setSlot(Envelope.EquipSlot.EQUIP_SLOT_CHEST)))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("inventory_event", json.get("type").getAsString());
        assertEquals("moved", json.get("kind").getAsString(),
                "oneof variant name should become 'kind' field");
        assertEquals(7, json.get("revision").getAsLong(),
                "inner fields should be flattened to top level");
        assertEquals(42, json.get("instance_id").getAsLong());
        assertTrue(json.has("from"), "from location should be present");
        assertTrue(json.has("to"), "to location should be present");
    }

    @Test
    void inventoryEventDroppedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setDropped(Envelope.InventoryEventDropped.newBuilder()
                                .setRevision(3)
                                .setInstanceId(10)
                                .setWorldPosX(100.0).setWorldPosY(64.0).setWorldPosZ(200.0)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("dropped", json.get("kind").getAsString());
        assertEquals(3, json.get("revision").getAsLong());
        assertEquals(100.0, json.get("world_pos_x").getAsDouble(), 0.01);
    }

    @Test
    void inventoryEventStackChangedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setStackChanged(Envelope.InventoryEventStackChanged.newBuilder()
                                .setRevision(5).setInstanceId(99).setStackCount(64)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("stack_changed", json.get("kind").getAsString());
        assertEquals(64, json.get("stack_count").getAsLong());
    }

    @Test
    void inventoryEventDurabilityChangedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setDurabilityChanged(Envelope.InventoryEventDurabilityChanged.newBuilder()
                                .setRevision(8).setInstanceId(50).setDurability(0.75)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("durability_changed", json.get("kind").getAsString());
        assertEquals(0.75, json.get("durability").getAsDouble(), 0.001);
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeOneofFlat: CraftOutcome — completed / failed
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void craftOutcomeCompletedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                        .setCompleted(Envelope.CraftOutcomeCompleted.newBuilder()
                                .setRecipeId("iron_sword")
                                .setOutputTemplate("iron_sword_basic")
                                .setOutputCount(1)
                                .setCompletedAtTick(1000)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("craft_outcome", json.get("type").getAsString());
        assertEquals("completed", json.get("kind").getAsString());
        assertEquals("iron_sword", json.get("recipe_id").getAsString());
        assertEquals(1, json.get("output_count").getAsInt());
        assertEquals(1000, json.get("completed_at_tick").getAsLong());
    }

    @Test
    void craftOutcomeFailedFlattensOneof() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                        .setFailed(Envelope.CraftOutcomeFailed.newBuilder()
                                .setRecipeId("spirit_pill")
                                .setMaterialReturned(2)
                                .setQiRefunded(50.5)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("failed", json.get("kind").getAsString());
        assertEquals("spirit_pill", json.get("recipe_id").getAsString());
        assertEquals(2, json.get("material_returned").getAsInt());
        assertEquals(50.5, json.get("qi_refunded").getAsDouble(), 0.01);
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeSlotConfig: QuickSlotConfig — wrapper unwrap + empty slots
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void quickSlotConfigUnwrapsEntryAndNullifiesEmpty() {
        Envelope.QuickSlotConfig.Builder qsc = Envelope.QuickSlotConfig.newBuilder();
        // slot 0: filled
        qsc.addSlots(Envelope.OptionalQuickSlotEntry.newBuilder()
                .setEntry(Envelope.QuickSlotEntry.newBuilder()
                        .setItemId("healing_pill")
                        .setDisplayName("灵息丸")
                        .setCastDurationMs(500)));
        // slots 1-8: empty
        for (int i = 1; i < 9; i++) {
            qsc.addSlots(Envelope.OptionalQuickSlotEntry.newBuilder());
        }
        for (int i = 0; i < 9; i++) {
            qsc.addCooldownUntilMs(0);
        }

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQuickSlotConfig(qsc).build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("quickslot_config", json.get("type").getAsString());

        JsonArray slots = json.getAsJsonArray("slots");
        assertEquals(9, slots.size(), "should have 9 slots");

        // slot 0: unwrapped from wrapper — should have item_id directly
        assertTrue(slots.get(0).isJsonObject(), "filled slot should be an object");
        JsonObject slot0 = slots.get(0).getAsJsonObject();
        assertEquals("healing_pill", slot0.get("item_id").getAsString(),
                "item_id should be at top level (not nested under 'entry')");
        assertFalse(slot0.has("entry"),
                "wrapper 'entry' field should be removed after unwrapping");

        // slots 1-8: empty wrapper {} → JsonNull
        for (int i = 1; i < 9; i++) {
            assertTrue(slots.get(i).isJsonNull(),
                    "empty slot " + i + " should be null (not empty object {})");
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeSlotConfig: SkillBarConfig — wrapper + oneof flatten
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void skillBarConfigUnwrapsAndFlattensOneof() {
        Envelope.SkillBarConfig.Builder sbc = Envelope.SkillBarConfig.newBuilder();
        // slot 0: item type
        sbc.addSlots(Envelope.OptionalSkillBarEntry.newBuilder()
                .setEntry(Envelope.SkillBarEntry.newBuilder()
                        .setItem(Envelope.SkillBarEntryItem.newBuilder()
                                .setTemplateId("iron_sword")
                                .setDisplayName("铁剑")
                                .setCastDurationMs(0)
                                .setCooldownMs(1000))));
        // slot 1: skill type
        sbc.addSlots(Envelope.OptionalSkillBarEntry.newBuilder()
                .setEntry(Envelope.SkillBarEntry.newBuilder()
                        .setSkill(Envelope.SkillBarEntrySkill.newBuilder()
                                .setSkillId("fireball")
                                .setDisplayName("火球术")
                                .setCastDurationMs(2000)
                                .setCooldownMs(5000))));
        // slots 2-8: empty
        for (int i = 2; i < 9; i++) {
            sbc.addSlots(Envelope.OptionalSkillBarEntry.newBuilder());
        }
        for (int i = 0; i < 9; i++) {
            sbc.addCooldownUntilMs(0);
        }

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillBarConfig(sbc).build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("skillbar_config", json.get("type").getAsString());

        JsonArray slots = json.getAsJsonArray("slots");

        // slot 0: item — kind should be "item", template_id at top level
        JsonObject slot0 = slots.get(0).getAsJsonObject();
        assertEquals("item", slot0.get("kind").getAsString(),
                "oneof variant 'item' should become kind='item'");
        assertEquals("iron_sword", slot0.get("template_id").getAsString(),
                "inner fields should be flattened to slot level");
        assertFalse(slot0.has("item"), "oneof variant key 'item' should be removed");
        assertFalse(slot0.has("entry"), "wrapper key 'entry' should be removed");

        // slot 1: skill
        JsonObject slot1 = slots.get(1).getAsJsonObject();
        assertEquals("skill", slot1.get("kind").getAsString());
        assertEquals("fireball", slot1.get("skill_id").getAsString());

        // slots 2-8: null
        for (int i = 2; i < 9; i++) {
            assertTrue(slots.get(i).isJsonNull(),
                    "empty slot " + i + " should be null");
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // bridgeForgeSession: step_state oneof flatten
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void forgeSessionInscriptionFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(1)
                        .setBlueprintId("iron_sword_bp")
                        .setBlueprintName("铁剑图纸")
                        .setActive(true)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_INSCRIPTION)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setInscription(Envelope.ForgeStepStateInscription.newBuilder()
                                        .setFilledSlots(3)
                                        .setMaxSlots(5)
                                        .setFailed(false))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("forge_session", json.get("type").getAsString());
        assertEquals(1, json.get("session_id").getAsLong());

        JsonObject stepState = json.getAsJsonObject("step_state");
        assertNotNull(stepState, "step_state should be present");
        assertEquals("inscription", stepState.get("step").getAsString(),
                "oneof variant should become step='inscription'");
        assertEquals(3, stepState.get("filled_slots").getAsInt(),
                "inner fields should be flattened into step_state");
        assertEquals(5, stepState.get("max_slots").getAsInt());
        assertFalse(stepState.has("inscription"),
                "oneof variant key 'inscription' should be removed after flatten");
    }

    @Test
    void forgeSessionTemperingFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(2)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_TEMPERING)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setTempering(Envelope.ForgeStepStateTempering.newBuilder()
                                        .setBeatCursor(4)
                                        .setHits(3)
                                        .setMisses(1))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("tempering", stepState.get("step").getAsString());
        assertEquals(4, stepState.get("beat_cursor").getAsInt());
        assertEquals(3, stepState.get("hits").getAsInt());
    }

    @Test
    void forgeSessionNoneStateFlattenedToStepNone() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(3)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_DONE)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setNoneState(true)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("none", stepState.get("step").getAsString(),
                "none_state=true should become step='none'");
        assertFalse(stepState.has("none_state"),
                "none_state field should be removed after conversion");
    }

    @Test
    void forgeSessionBilletFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(4)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_BILLET)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setBillet(Envelope.ForgeStepStateBillet.newBuilder()
                                        .setResolvedTierCap(3))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("billet", stepState.get("step").getAsString());
        assertEquals(3, stepState.get("resolved_tier_cap").getAsInt());
    }

    @Test
    void forgeSessionConsecrationFlattenedWithStepTag() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(5)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_CONSECRATION)
                        .setStepState(Envelope.ForgeStepState.newBuilder()
                                .setConsecration(Envelope.ForgeStepStateConsecration.newBuilder()
                                        .setQiInjected(50.0)
                                        .setQiRequired(100.0))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject stepState = json.getAsJsonObject("step_state");
        assertEquals("consecration", stepState.get("step").getAsString());
        assertEquals(50.0, stepState.get("qi_injected").getAsDouble(), 0.01);
    }

    // ═══════════════════════════════════════════════════════════════════
    // normalizeNumericStrings: nested objects and arrays
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void nestedUint64InSubObjectsAreNormalized() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(Envelope.InventoryEvent.newBuilder()
                        .setMoved(Envelope.InventoryEventMoved.newBuilder()
                                .setRevision(123456789)
                                .setInstanceId(987654321)
                                .setFrom(Envelope.InventoryLocation.newBuilder()
                                        .setContainer(Envelope.InventoryLocationContainer.newBuilder()
                                                .setContainerId("backpack")
                                                .setRow(5).setCol(3)))))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals(123456789, json.get("revision").getAsLong());
        assertEquals(987654321, json.get("instance_id").getAsLong());

        JsonObject from = json.getAsJsonObject("from");
        assertNotNull(from);
        JsonObject container = from.getAsJsonObject("container");
        assertNotNull(container);
        assertTrue(container.get("row").getAsJsonPrimitive().isNumber(),
                "nested uint64 'row' should be a JSON number after normalization");
        assertEquals(5, container.get("row").getAsLong());
    }

    @Test
    void uint64InRepeatedFieldsAreNormalized() {
        Envelope.QuickSlotConfig.Builder qsc = Envelope.QuickSlotConfig.newBuilder();
        for (int i = 0; i < 9; i++) {
            qsc.addSlots(Envelope.OptionalQuickSlotEntry.newBuilder());
        }
        qsc.addCooldownUntilMs(1000);
        qsc.addCooldownUntilMs(0);
        qsc.addCooldownUntilMs(5000);
        for (int i = 3; i < 9; i++) {
            qsc.addCooldownUntilMs(0);
        }

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQuickSlotConfig(qsc).build();

        JsonObject json = bridgeAndParse(envelope);
        JsonArray cooldowns = json.getAsJsonArray("cooldown_until_ms");
        assertEquals(9, cooldowns.size());
        assertTrue(cooldowns.get(0).getAsJsonPrimitive().isNumber(),
                "uint64 elements in repeated field should be JSON numbers");
        assertEquals(1000, cooldowns.get(0).getAsLong());
        assertEquals(5000, cooldowns.get(2).getAsLong());
    }

    // ═══════════════════════════════════════════════════════════════════
    // cast_sync: enum prefix normalization (plan-skill-warn-hud)
    //
    // proto3 canonical JSON 把枚举打成 CAST_PHASE_* / CAST_OUTCOME_*；CastSyncHandler
    // 期望 serde snake_case。bridgeCastSync 必须剥前缀转小写，否则整条 cast_sync 在
    // proto 线上静默 noOp（连既有 casting/meridian_gated 也收不到）。
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void bridgeCastSyncStripsPhaseAndOutcomeEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCastSync(Envelope.CastSync.newBuilder()
                        .setPhase(Envelope.CastPhase.CAST_PHASE_CASTING)
                        .setSlot(3)
                        .setDurationMs(1500)
                        .setStartedAtMs(1_700_000_000_000L)
                        .setOutcome(Envelope.CastOutcome.CAST_OUTCOME_NONE))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("cast_sync", json.get("type").getAsString());
        assertEquals("casting", json.get("phase").getAsString(),
                "phase 必须从 CAST_PHASE_CASTING 剥成 'casting'（CastSyncHandler switch 期望），"
                + "否则 proto 线上 cast bar 整条静默 noOp");
        assertEquals("none", json.get("outcome").getAsString(),
                "outcome 必须从 CAST_OUTCOME_NONE 剥成 'none'");
    }

    @Test
    void bridgeCastSyncMaps通用警示RejectOutcomesToSnakeCase() {
        // 每个 reject outcome 经 bridge 后必须落到 CastSyncHandler.parseOutcome 认得的 wire 串。
        record Case(Envelope.CastOutcome proto, String expectedWire) {}
        Case[] cases = new Case[] {
            new Case(Envelope.CastOutcome.CAST_OUTCOME_MERIDIAN_GATED, "meridian_gated"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_QI_INSUFFICIENT, "reject_qi_insufficient"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_ON_COOLDOWN, "reject_on_cooldown"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_INVALID_TARGET, "reject_invalid_target"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_IN_RECOVERY, "reject_in_recovery"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_REALM_TOO_LOW, "reject_realm_too_low"),
            new Case(Envelope.CastOutcome.CAST_OUTCOME_REJECT_NO_WEAPON, "reject_no_weapon"),
        };
        for (Case c : cases) {
            Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                    .setCastSync(Envelope.CastSync.newBuilder()
                            .setPhase(Envelope.CastPhase.CAST_PHASE_IDLE)
                            .setSlot(0)
                            .setDurationMs(0)
                            .setStartedAtMs(1L)
                            .setOutcome(c.proto()))
                    .build();
            JsonObject json = bridgeAndParse(envelope);
            assertEquals("idle", json.get("phase").getAsString(),
                    "施放前拒绝 phase 应为 idle for " + c.proto());
            assertEquals(c.expectedWire(), json.get("outcome").getAsString(),
                    "outcome " + c.proto() + " 应剥成 '" + c.expectedWire()
                    + "'（CastSyncHandler.parseOutcome 据此映射文案）");
        }
    }

    // ─── event_stream_push: 战斗事件流 HUD 接收前提 ──────────────────
    // proto3 JSON 把 EventChannel/EventPriority 打成枚举全名；EventStreamPushHandler
    // 只认 serde snake_case。bridge 必须剥前缀转小写，否则 channel/priority parse 为
    // null → handler noOp → HUD 事件流永远空白。锁住每个 channel/priority 变体的 wire 串。

    @Test
    void bridgeEventStreamPushStripsChannelAndPriorityPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                        .setChannel(Envelope.EventChannel.EVENT_CHANNEL_COMBAT)
                        .setPriority(Envelope.EventPriority.EVENT_PRIORITY_P1_IMPORTANT)
                        .setSourceTag("hit-Head-Slash")
                        .setText("命中 Head Slash -8")
                        .setColor(0)
                        .setCreatedAtMs(1_700_000_000_000L))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("event_stream_push", json.get("type").getAsString());
        assertEquals("combat", json.get("channel").getAsString(),
                "channel 必须从 EVENT_CHANNEL_COMBAT 剥成 'combat'（EventStreamPushHandler.parseChannel 期望），"
                + "否则 proto 线上战斗事件流整条静默 noOp，HUD 事件流区永远空白");
        assertEquals("p1_important", json.get("priority").getAsString(),
                "priority 必须从 EVENT_PRIORITY_P1_IMPORTANT 剥成 'p1_important'（parsePriority 期望）");
        assertEquals("命中 Head Slash -8", json.get("text").getAsString(), "text 应原样透传");
        assertEquals("hit-Head-Slash", json.get("source_tag").getAsString(), "source_tag 应原样透传");
    }

    @Test
    void bridgeEventStreamPushMapsAllChannelsToHandlerWire() {
        record Case(Envelope.EventChannel proto, String expectedWire) {}
        Case[] cases = new Case[] {
            new Case(Envelope.EventChannel.EVENT_CHANNEL_COMBAT, "combat"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_CULTIVATION, "cultivation"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_WORLD, "world"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_SOCIAL, "social"),
            new Case(Envelope.EventChannel.EVENT_CHANNEL_SYSTEM, "system"),
        };
        for (Case c : cases) {
            Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                    .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                            .setChannel(c.proto())
                            .setPriority(Envelope.EventPriority.EVENT_PRIORITY_P2_NORMAL)
                            .setText("x")
                            .setCreatedAtMs(1L))
                    .build();
            JsonObject json = bridgeAndParse(envelope);
            assertEquals(c.expectedWire(), json.get("channel").getAsString(),
                    "channel " + c.proto() + " 应剥成 '" + c.expectedWire()
                    + "'（EventStreamPushHandler.parseChannel 据此入 UnifiedEventStore）");
        }
    }

    @Test
    void bridgeEventStreamPushMapsAllPrioritiesToHandlerWire() {
        record Case(Envelope.EventPriority proto, String expectedWire) {}
        Case[] cases = new Case[] {
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P0_CRITICAL, "p0_critical"),
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P1_IMPORTANT, "p1_important"),
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P2_NORMAL, "p2_normal"),
            new Case(Envelope.EventPriority.EVENT_PRIORITY_P3_VERBOSE, "p3_verbose"),
        };
        for (Case c : cases) {
            Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                    .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                            .setChannel(Envelope.EventChannel.EVENT_CHANNEL_COMBAT)
                            .setPriority(c.proto())
                            .setText("x")
                            .setCreatedAtMs(1L))
                    .build();
            JsonObject json = bridgeAndParse(envelope);
            assertEquals(c.expectedWire(), json.get("priority").getAsString(),
                    "priority " + c.proto() + " 应剥成 '" + c.expectedWire()
                    + "'（parsePriority 据此；死亡事件用 P0_CRITICAL）");
        }
    }

    // ─── event_stream_push 端到端：bridge → router → handler → store ──
    // 缺陷本质在下游消费链：枚举未剥 → handler noOp → 事件永不入库。这条用例走完整
    // proto bytes → ProtoServerDataBridge → ServerDataRouter → EventStreamPushHandler
    // → UnifiedEventStore，断言可观测的入库结果，单元桥接断言无法替代。

    @Test
    void eventStreamPushRoutesEndToEndIntoUnifiedEventStore() {
        UnifiedEventStore.resetForTests();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                        .setChannel(Envelope.EventChannel.EVENT_CHANNEL_COMBAT)
                        .setPriority(Envelope.EventPriority.EVENT_PRIORITY_P1_IMPORTANT)
                        .setSourceTag("hit-Head-Slash")
                        .setText("命中 Head Slash -8")
                        .setColor(0)
                        .setCreatedAtMs(1_700_000_000_000L))
                .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(), "bridge should succeed: " + bridged.errorMessage());

        ServerDataRouter router = ServerDataRouter.createDefault();
        ServerDataRouter.RouteResult result = router.route(
                bridged.legacyJson(),
                bridged.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(),
                "event_stream_push 应被 handler 接受(非 noOp)；枚举未剥时这里会 noOp，事件流永远空白");
        List<UnifiedEvent> events = UnifiedEventStore.stream().snapshot();
        assertEquals(1, events.size(),
                "事件应入 UnifiedEventStore；修前因 channel/priority parse 为 null 而永不入库");
        UnifiedEvent ev = events.get(0);
        assertEquals(UnifiedEvent.Channel.COMBAT, ev.channel(),
                "channel 应解析为 COMBAT（来自剥前缀后的 'combat'）");
        assertEquals(UnifiedEvent.Priority.P1_IMPORTANT, ev.priority(),
                "priority 应解析为 P1_IMPORTANT（来自剥前缀后的 'p1_important'）");
        assertEquals("命中 Head Slash -8", ev.text(), "text 端到端原样透传");
    }

    @Test
    void eventStreamPushUnspecifiedEnumStaysOutOfStore() {
        // 错误分支：UNSPECIFIED 剥后为 'unspecified'，parseChannel/parsePriority 返回 null，
        // handler noOp，事件不入库。锁住"坏 channel/priority 不污染事件流"。
        UnifiedEventStore.resetForTests();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventStreamPush(Envelope.EventStreamPush.newBuilder()
                        .setChannel(Envelope.EventChannel.EVENT_CHANNEL_UNSPECIFIED)
                        .setPriority(Envelope.EventPriority.EVENT_PRIORITY_UNSPECIFIED)
                        .setText("x")
                        .setCreatedAtMs(1L))
                .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess());
        ServerDataRouter router = ServerDataRouter.createDefault();
        ServerDataRouter.RouteResult result = router.route(
                bridged.legacyJson(),
                bridged.legacyJson().getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isHandled(), "UNSPECIFIED channel/priority 应 noOp，不被当作有效事件");
        assertEquals(0, UnifiedEventStore.stream().snapshot().size(),
                "坏枚举不应入 UnifiedEventStore");
    }

    // ═══════════════════════════════════════════════════════════════════
    // F9 跨层修复：tutorial_coffin_pos proto bridge
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void bridgeTutorialCoffinPosProducesLegacyJsonAndRoutesIntoStore() {
        com.bong.client.coffin.TutorialCoffinPosStore.resetForTests();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTutorialCoffinPos(Envelope.TutorialCoffinPos.newBuilder()
                        .setX(12)
                        .setY(71)
                        .setZ(-33))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("tutorial_coffin_pos", json.get("type").getAsString());
        assertEquals(12, json.get("x").getAsInt());
        assertEquals(71, json.get("y").getAsInt());
        assertEquals(-33, json.get("z").getAsInt(),
                "negative z must survive the proto->JSON bridge untouched");

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(json.toString(), json.toString().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "tutorial_coffin_pos proto bridge output should route: " + route.logMessage());
        assertFalse(route.isNoOp(), "tutorial_coffin_pos proto bridge output must not become no-op");

        var stored = com.bong.client.coffin.TutorialCoffinPosStore.snapshot();
        assertTrue(stored.isPresent(), "store should hold the broadcast coffin pos after routing");
        assertEquals(new net.minecraft.util.math.BlockPos(12, 71, -33), stored.get());
    }

    @Test
    void bridgeTutorialCoffinPosAtOriginIsPreservedIncludingDefaultValueFields() {
        // x=0/y=0/z=0 must still survive the bridge (includingDefaultValueFields), otherwise
        // a coffin that legitimately sits at the world origin would look like "no field".
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTutorialCoffinPos(Envelope.TutorialCoffinPos.newBuilder()
                        .setX(0).setY(0).setZ(0))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("tutorial_coffin_pos", json.get("type").getAsString());
        assertTrue(json.has("x") && json.has("y") && json.has("z"),
                "x=0/y=0/z=0 fields must be present, not dropped as proto3 defaults");
        assertEquals(0, json.get("x").getAsInt());
        assertEquals(0, json.get("y").getAsInt());
        assertEquals(0, json.get("z").getAsInt());
    }

    // ─── plan-inventory-hint-panel-v1 P0/P1：InventoryMoveRejected proto round-trip ──
    @Test
    void bridgeInventoryMoveRejectedWithAllFieldsPreservesStructuredValues() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryMoveRejected(Envelope.InventoryMoveRejected.newBuilder()
                        .setReason("worn_cap_full")
                        .setSlot("chest")
                        .setCap(3))
                .build();

        JsonObject json = bridgeAndParse(envelope);

        assertEquals(1, json.get("v").getAsInt());
        assertEquals("inventory_move_rejected", json.get("type").getAsString());
        assertEquals("worn_cap_full", json.get("reason").getAsString());
        assertEquals("chest", json.get("slot").getAsString());
        assertEquals(3, json.get("cap").getAsInt(), "cap is uint32, must round-trip as a JSON number not string");
        assertFalse(json.has("required_realm"),
                "required_realm is proto3 optional and unset here — must be omitted, not null/empty string");
    }

    @Test
    void bridgeInventoryMoveRejectedRealmTooLowCarriesEnglishTagOnly() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryMoveRejected(Envelope.InventoryMoveRejected.newBuilder()
                        .setReason("realm_too_low")
                        .setRequiredRealm("Condense"))
                .build();

        JsonObject json = bridgeAndParse(envelope);

        assertEquals("realm_too_low", json.get("reason").getAsString());
        assertEquals("Condense", json.get("required_realm").getAsString(),
                "server must ship the English realm_to_string tag; Chinese conversion happens client-side");
        assertFalse(json.has("slot"));
        assertFalse(json.has("cap"));
    }

    @Test
    void bridgeInventoryMoveRejectedMinimalReasonOmitsOptionalFields() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryMoveRejected(Envelope.InventoryMoveRejected.newBuilder()
                        .setReason("hand_occupied"))
                .build();

        JsonObject json = bridgeAndParse(envelope);

        assertEquals("hand_occupied", json.get("reason").getAsString());
        assertFalse(json.has("required_realm"));
        assertFalse(json.has("slot"));
        assertFalse(json.has("cap"));
    }

    // ═══════════════════════════════════════════════════════════════════
    // P0 — RC1 uint64→JSON字符串 (docs/wire-format-bridge-audit-report.md RC1 节, 14 条)
    //
    // §8.1 收口决议 #2：printAndNormalize()/normalizeNumericStrings()
    // (ProtoServerDataBridge.java:1028-1073) 已在桥层对全部 124 payloadCase 无差别把
    // proto3-JSON 的 uint64 字符串转回 JSON number，是通用路径 + 全部专属 fixup 的公共
    // 前置步骤。以下 14 条逐一 fixture 实证：喂真实 proto3-JSON 形状（uint64 字段）→
    // bridge() → 断言对应 handler 非 noOp 且字段落地为正确数值（非 null / 非 fallback
    // 0 或 1）。全部预期 PASS（已被 normalizeNumericStrings 覆盖，非活 bug）——按 P0
    // 决议"仅对 fixture 真失败的才修"，本节全绿则不改 36 处 readLong reader。
    // ═══════════════════════════════════════════════════════════════════

    // ─── RC1 #1: social_pact.tick ────────────────────────────────────
    @Test
    void socialPactTickNormalizesToNumberAndAppliesRelationship() {
        long tick = 1_234_567_890L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSocialPact(Envelope.SocialPact.newBuilder()
                        .setLeft("player_a")
                        .setRight("player_b")
                        .setTerms("mutual_aid")
                        .setTick(tick)
                        .setBroken(false))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for social_pact: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "social_pact.tick (uint64) must parse as JSON number via printAndNormalize, "
                + "not noOp via readLong's isNumber() gate: " + route.logMessage());

        List<SocialStateStore.SocialRelationshipSignal> relationships = SocialStateStore.relationships();
        assertEquals(1, relationships.size());
        assertEquals(tick, relationships.get(0).tick(),
                "SocialRelationshipSignal.tick must carry the real server tick");
    }

    // ─── RC1 #2: sparring_invite.expires_at_ms ───────────────────────
    @Test
    void sparringInviteExpiresAtMsNormalizesToNumber() {
        long expiresAtMs = 1_719_999_999_999L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSparringInvite(Envelope.SparringInvite.newBuilder()
                        .setInviteId("invite-1")
                        .setInitiator("player_a")
                        .setTarget("player_b")
                        .setExpiresAtMs(expiresAtMs))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for sparring_invite: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "sparring_invite.expires_at_ms (uint64) must not noOp: " + route.logMessage());

        SocialStateStore.SparringInvite invite = SocialStateStore.sparringInvite();
        assertNotNull(invite, "sparring invite must be recorded");
        assertEquals(expiresAtMs, invite.expiresAtMs());
    }

    // ─── RC1 #3/#4: trade_offer.expires_at_ms / offered_item.instance_id ─
    private static Envelope.ServerDataEnvelope buildTradeOfferEnvelope(long expiresAtMs, long instanceId) {
        return Envelope.ServerDataEnvelope.newBuilder()
                .setTradeOffer(Envelope.TradeOffer.newBuilder()
                        .setOfferId("offer-1")
                        .setInitiator("player_a")
                        .setTarget("player_b")
                        .setOfferedItem(Envelope.TradeItemSummary.newBuilder()
                                .setInstanceId(instanceId)
                                .setItemId("spirit_stone")
                                .setDisplayName("灵石")
                                .setStackCount(1))
                        .addRequestedItems(Envelope.TradeItemSummary.newBuilder()
                                .setInstanceId(777L)
                                .setItemId("herb")
                                .setDisplayName("药草")
                                .setStackCount(2))
                        .setExpiresAtMs(expiresAtMs))
                .build();
    }

    @Test
    void tradeOfferExpiresAtMsNormalizesToNumber() {
        long expiresAtMs = 1_720_000_000_000L;
        Envelope.ServerDataEnvelope envelope = buildTradeOfferEnvelope(expiresAtMs, 555L);

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for trade_offer: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "trade_offer.expires_at_ms (uint64) must not noOp: " + route.logMessage());

        SocialStateStore.TradeOffer offer = SocialStateStore.tradeOffer();
        assertNotNull(offer, "trade offer must be recorded");
        assertEquals(expiresAtMs, offer.expiresAtMs());
    }

    @Test
    void tradeOfferOfferedItemInstanceIdNormalizesToNumber() {
        long instanceId = 9_876_543_210L; // > Integer.MAX_VALUE — proves genuine uint64, not int32
        Envelope.ServerDataEnvelope envelope = buildTradeOfferEnvelope(1_720_000_000_000L, instanceId);

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for trade_offer: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "trade_offer.offered_item.instance_id (uint64) must not noOp "
                + "(parseTradeItem returning null would independently noOp the whole trade offer): "
                + route.logMessage());

        SocialStateStore.TradeOffer offer = SocialStateStore.tradeOffer();
        assertNotNull(offer, "trade offer must be recorded");
        assertEquals(instanceId, offer.offeredItem().instanceId());
    }

    // ─── RC1 #5: poison_overdose_event.player_entity_id ──────────────
    @Test
    void poisonOverdoseEventPlayerEntityIdNormalizesToNumber() {
        long playerEntityId = 4_294_967_296L; // > uint32 max — proves genuine uint64
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPoisonOverdoseEvent(Envelope.PoisonOverdoseEvent.newBuilder()
                        .setPlayerEntityId(playerEntityId)
                        .setOverflow(0.2f)
                        .setLifespanPenaltyYears(3.5f)
                        .setMicroTearProbability(0.1f)
                        .setAtTick(100L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for poison_overdose_event: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "poison_overdose_event.player_entity_id (uint64) must not noOp via "
                + "readNonNegativeLong's isNumber() gate: " + route.logMessage());

        assertEquals(3.5f, PoisonTraitHudStateStore.snapshot().lifespanYearsLost(), 1e-6f,
                "the lifespan-loss warning must actually apply once player_entity_id parses");
    }

    // ─── RC1 #6: loot_container_update.session_id ────────────────────
    @Test
    void lootContainerUpdateSessionIdNormalizesToNumber() {
        long sessionId = 10_000_000_001L;
        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
                sessionId, "supply_coffin", "legendary", 3, 4, 120L, List.of()));

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setLootContainerUpdate(Envelope.LootContainerUpdate.newBuilder()
                        .setSessionId(sessionId))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for loot_container_update: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "loot_container_update.session_id (uint64) must not noOp: " + route.logMessage());

        LootContainerStateStore.Session current = LootContainerStateStore.current();
        assertTrue(current instanceof LootContainerStateStore.OpenSession,
                "session must remain open (not silently dropped) after update");
        assertEquals(sessionId, ((LootContainerStateStore.OpenSession) current).sessionId());
    }

    // ─── RC1 #7: defense_window.started_at_ms / expires_at_ms ────────
    @Test
    void defenseWindowStartedAndExpiresAtMsNormalizeToNumbers() {
        long startedAtMs = 1_720_000_000_000L;
        long expiresAtMs = startedAtMs + 600L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDefenseWindow(Envelope.DefenseWindow.newBuilder()
                        .setDurationMs(600)
                        .setStartedAtMs(startedAtMs)
                        .setExpiresAtMs(expiresAtMs))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for defense_window: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "defense_window.started_at_ms/expires_at_ms (uint64) must not noOp — otherwise "
                + "the 截脉弹反窗口 HUD red ring never renders: " + route.logMessage());

        DefenseWindowState snapshot = DefenseWindowStore.snapshot();
        assertTrue(snapshot.active());
        assertEquals(startedAtMs, snapshot.startedAtMs());
        assertEquals(expiresAtMs, snapshot.expiresAtMs());
    }

    // ─── RC1 #8: death_screen.cinematic.{phase_tick,phase_duration_ticks,
    //             total_elapsed_ticks,total_duration_ticks,rebirth_weakened_ticks} ─
    @Test
    void deathScreenCinematicUint64TimingFieldsNormalizeToNumbers() {
        long phaseTick = 1_234_567L;
        long phaseDurationTicks = 2_000_000L;
        long totalElapsedTicks = 5_555_555L;
        long totalDurationTicks = 9_999_999L;
        long rebirthWeakenedTicks = 42_000L;

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDeathScreen(Envelope.DeathScreen.newBuilder()
                        .setVisible(true)
                        .setCause("pk")
                        .setCinematic(Envelope.DeathCinematicData.newBuilder()
                                .setV(1)
                                .setCharacterId("char-1")
                                .setPhase(Envelope.DeathCinematicPhase.DEATH_CINEMATIC_PHASE_ROLL)
                                .setPhaseTick(phaseTick)
                                .setPhaseDurationTicks(phaseDurationTicks)
                                .setTotalElapsedTicks(totalElapsedTicks)
                                .setTotalDurationTicks(totalDurationTicks)
                                .setRebirthWeakenedTicks(rebirthWeakenedTicks)
                                .setRoll(Envelope.DeathCinematicRoll.newBuilder()
                                        .setResult(Envelope.DeathRollResult.DEATH_ROLL_RESULT_SURVIVE))))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for death_screen: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), route.logMessage());

        var cinematic = DeathStateStore.snapshot().cinematic();
        assertEquals(phaseTick, cinematic.phaseTick(),
                "phase_tick (uint64) must land as the real value, not the readLong fallback 0");
        assertEquals(phaseDurationTicks, cinematic.phaseDurationTicks(),
                "phase_duration_ticks (uint64) must land as the real value, not the readDurationTicks fallback 1");
        assertEquals(totalElapsedTicks, cinematic.totalElapsedTicks());
        assertEquals(totalDurationTicks, cinematic.totalDurationTicks());
        assertEquals(rebirthWeakenedTicks, cinematic.rebirthWeakenedTicks());
    }

    // ─── RC1 #9: social_renown_delta.tags_added[].last_seen_tick ─────
    @Test
    void socialRenownDeltaTagsAddedLastSeenTickNormalizesToNumber() {
        long lastSeenTick = 987_654_321L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSocialRenownDelta(Envelope.SocialRenownDelta.newBuilder()
                        .setCharId("char-1")
                        .setFameDelta(5)
                        .setNotorietyDelta(0)
                        .addTagsAdded(Envelope.RenownTag.newBuilder()
                                .setTag("savior")
                                .setWeight(1.0)
                                .setLastSeenTick(lastSeenTick)
                                .setPermanent(true))
                        .setTick(100L)
                        .setReason("quest"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for social_renown_delta: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "social_renown_delta must not noOp: " + route.logMessage());

        List<SocialStateStore.SocialRenownDelta> deltas = SocialStateStore.renownDeltas();
        assertEquals(1, deltas.size());
        assertEquals(1, deltas.get(0).tagsAdded().size(),
                "tags_added[].last_seen_tick (uint64) must not silently drop the whole tag "
                + "via parseRenownTags' per-tag readLong isNumber() gate");
        assertEquals(lastSeenTick, deltas.get(0).tagsAdded().get(0).lastSeenTick());
    }

    // ─── RC1 #10: niche_intrusion.items_taken (repeated uint64) ──────
    @Test
    void nicheIntrusionItemsTakenNormalizesRepeatedUint64ToNumbers() {
        long item1 = 1_000_000_001L;
        long item2 = 1_000_000_002L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setNicheIntrusion(Envelope.NicheIntrusion.newBuilder()
                        .setIntruderId("intruder-1")
                        .addItemsTaken(item1)
                        .addItemsTaken(item2)
                        .setTaintDelta(0.3f))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for niche_intrusion: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "niche_intrusion must not noOp: " + route.logMessage());

        List<NicheGuardianStore.NicheIntrusionAlert> alerts = NicheGuardianStore.intrusionAlerts();
        assertEquals(1, alerts.size());
        assertEquals(List.of(item1, item2), alerts.get(0).itemsTaken(),
                "repeated uint64 items_taken must land as real ids, not be filtered out by "
                + "readLongArray's per-element isNumber() gate");
    }

    // ─── RC1 #11: full_power_exhausted_state.started_tick / recovery_at_tick ─
    @Test
    void fullPowerExhaustedStateStartedAndRecoveryTickNormalizeToNumbers() {
        long startedTick = 100_000_000L;
        long recoveryAtTick = startedTick + 36_000L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFullPowerExhausted(Envelope.FullPowerExhaustedState.newBuilder()
                        .setCasterUuid("caster-1")
                        .setActive(true)
                        .setStartedTick(startedTick)
                        .setRecoveryAtTick(recoveryAtTick))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for full_power_exhausted_state: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "full_power_exhausted_state.started_tick/recovery_at_tick (uint64) must not noOp: "
                + route.logMessage());

        FullPowerStateStore.ExhaustedState exhausted = FullPowerStateStore.exhausted();
        assertTrue(exhausted.active());
        assertEquals(startedTick, exhausted.startedTick());
        assertEquals(recoveryAtTick, exhausted.recoveryAtTick());
    }

    // ─── RC1 #12: wounds_snapshot.wounds[].updated_at_ms ─────────────
    @Test
    void woundsSnapshotUpdatedAtMsNormalizesToNumber() {
        long updatedAtMs = 1_720_555_555_555L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setWoundsSnapshot(Envelope.WoundsSnapshot.newBuilder()
                        .addWounds(Envelope.WoundEntry.newBuilder()
                                .setPart("chest")
                                .setKind("cut")
                                .setSeverity(0.4f)
                                .setState("bleeding")
                                .setInfection(0.1f)
                                .setUpdatedAtMs(updatedAtMs)))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for wounds_snapshot: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), route.logMessage());

        WoundsStore.Wound wound = WoundsStore.get("chest");
        assertNotNull(wound, "wound entry for 'chest' must be present");
        assertEquals(updatedAtMs, wound.updatedAtMs(),
                "wounds[].updated_at_ms (uint64) must not silently fall back to 0 via "
                + "readDouble's isNumber() gate");
    }

    // ─── RC1 #13: false_skin_state.equipped_at_tick ──────────────────
    @Test
    void falseSkinStateEquippedAtTickNormalizesToNumber() {
        long equippedAtTick = 5_555_555L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFalseSkinState(Envelope.FalseSkinState.newBuilder()
                        .setTargetId("target-1")
                        .setLayersRemaining(2)
                        .setContamCapacityPerLayer(10.0)
                        .setAbsorbedContam(2.5)
                        .setEquippedAtTick(equippedAtTick))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for false_skin_state: " + result.errorMessage());

        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), route.logMessage());

        FalseSkinHudStateStore.State state = FalseSkinHudStateStore.snapshot();
        assertEquals(equippedAtTick, state.equippedAtTick(),
                "equipped_at_tick (uint64) must not silently fall back to 0 via readDouble's isNumber() gate");
    }

    // ─── RC1 #14: recipe_unlocked.unlocked_at_tick ───────────────────
    // NOTE: full router non-noOp assertion is not possible today — RecipeUnlockedHandler
    // also gates on `source.kind`, and the proto3-JSON shape of the `source` oneof
    // (UnlockEventSource) never produces a "kind" discriminator field (RC5,
    // docs/wire-format-bridge-audit-report.md RC5 节, P4 scope — out of this P0 pass).
    // So this pins the bridge-output (printAndNormalize) contract only: the raw JSON
    // field itself must be a number, independent of whether the handler ultimately
    // consumes it.
    @Test
    void recipeUnlockedUnlockedAtTickNormalizesToNumberAtBridgeLevel() {
        long unlockedAtTick = 4_242_424_242L;
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRecipeUnlocked(Envelope.RecipeUnlocked.newBuilder()
                        .setV(1)
                        .setRecipeId("recipe-1")
                        .setUnlockedAtTick(unlockedAtTick))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("unlocked_at_tick").getAsJsonPrimitive().isNumber(),
                "unlocked_at_tick must be a JSON number after printAndNormalize, "
                + "not a proto3-canonical string");
        assertEquals(unlockedAtTick, json.get("unlocked_at_tick").getAsLong());
    }

    // ─── RC1 residual gap (§8.1 #2): > Long.MAX_VALUE true uint64 ────
    // normalizeNumericStrings' Long.parseLong throws for real uint64 values above
    // Long.MAX_VALUE (represented on the wire via the negative-long unsigned trick) and
    // silently keeps the JSON string via the catch(NumberFormatException) branch. This
    // is a known, accepted residual gap (§8.1 #2 决议：游戏内 id/tick 罕见触达该量级，
    // 不修 reader，仅钉住行为防止未来误判为"已修好"）.
    @Test
    void uint64FieldAboveLongMaxValueStaysStringKnownResidualGap() {
        long unsignedMaxWireValue = -1L; // unsigned representation: 18446744073709551615
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(unsignedMaxWireValue))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertTrue(json.get("revision").getAsJsonPrimitive().isString(),
                "known residual gap: uint64 values beyond Long.MAX_VALUE stay JSON strings "
                + "after normalizeNumericStrings (NumberFormatException is caught silently); "
                + "if any RC1 handler's id/tick ever plausibly reaches this magnitude it would "
                + "need string-tolerant readLong, but none currently do");
        assertEquals("18446744073709551615", json.get("revision").getAsString());
    }

    // ═══════════════════════════════════════════════════════════════════
    // plan-wire-format-bridge-v1 P1／RC2 枚举前缀 fixup — crit 批（20 条）
    // 每条：喂真实 proto3-JSON 枚举全名 → bridge() → 断言输出剥成消费端期望的
    // wire 值（对应各 handler/store/switch 实际比较的字面量，详见
    // docs/wire-format-bridge-audit-report.md RC2 节）。
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void bridgePlayerStateNormalizesRealmEnum() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPlayerState(Envelope.PlayerState.newBuilder()
                        .setRealm(Common.Realm.REALM_CONDENSE)
                        .setZone("zone-1"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("player_state", json.get("type").getAsString());
        assertEquals("Condense", json.get("realm").getAsString(),
                "realm 必须从 REALM_CONDENSE 剥成 'Condense'（normalizeRealmField 产出格式，"
                + "HudRealmGate.tier() 小写后比对），否则所有境界门控 HUD 恒判醒灵");
    }

    @Test
    void bridgeSocialExposureStripsKindEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSocialExposure(Envelope.SocialExposure.newBuilder()
                        .setActor("player-1")
                        .setKind(Envelope.ExposureKind.EXPOSURE_KIND_DIVINE)
                        .setTick(10))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("divine", json.get("kind").getAsString(),
                "kind 必须从 EXPOSURE_KIND_DIVINE 剥成 'divine'（SocialServerDataHandler."
                + "EXPOSURE_KINDS 只认全小写裸词），否则 social_exposure 整条恒 noOp");
    }

    @Test
    void bridgeRiftPortalStateStripsDirectionAndKindEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRiftPortalState(Envelope.RiftPortalState.newBuilder()
                        .setEntityId(1)
                        .setKind(Envelope.RiftPortalKind.RIFT_PORTAL_KIND_MAIN_RIFT)
                        .setDirection(Envelope.RiftPortalDirection.RIFT_PORTAL_DIRECTION_EXIT)
                        .setFamilyId("family-1"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("exit", json.get("direction").getAsString(),
                "direction 必须剥成 'exit'（ExtractStateStore.nearestPortal() 字面量比对），"
                + "否则 Y 键撤离 / TSY 塌缩本族裂口面板永远匹配不到");
        assertEquals("main_rift", json.get("kind").getAsString(),
                "kind 必须剥成 'main_rift'（ExtractProgressHudPlanner.kindLabel switch）");
    }

    @Test
    void bridgeSearchAbortedStripsReasonEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSearchAborted(Envelope.SearchAborted.newBuilder()
                        .setPlayerId("player-1")
                        .setContainerEntityId(2)
                        .setReason(Envelope.SearchAbortReason.SEARCH_ABORT_REASON_COMBAT)
                        .setAtTick(5))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("combat", json.get("reason").getAsString(),
                "reason 必须剥成 'combat'（SearchHudStateStore.abortReason switch），"
                + "否则搜索中断 HUD 恒显示通用无原因态");
    }

    @Test
    void bridgeYidaoHudStateStripsActiveSkillEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setYidaoHudState(Envelope.YidaoHudState.newBuilder()
                        .setHealerId("healer-1")
                        .setActiveSkill(Envelope.YidaoSkillId.YIDAO_SKILL_ID_MERIDIAN_REPAIR))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("meridian_repair", json.get("active_skill").getAsString(),
                "active_skill 必须剥成 'meridian_repair'（YidaoHudPlanner.skillLabel switch），"
                + "否则医道 NPC 施法中的技能永远显示'待机'");
    }

    @Test
    void bridgeSkillXpGainStripsSkillEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillXpGain(Envelope.SkillXpGain.newBuilder()
                        .setCharId(1)
                        .setSkill(Common.SkillId.SKILL_ID_HERBALISM)
                        .setAmount(10))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("herbalism", json.get("skill").getAsString(),
                "skill 必须剥成 'herbalism'（SkillId.fromWire 精确 lowercase 匹配），"
                + "否则 skill_xp_gain 整条链路对所有技能恒 noOp");
    }

    @Test
    void bridgeSkillLvUpStripsSkillEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillLvUp(Envelope.SkillLvUp.newBuilder()
                        .setCharId(1)
                        .setSkill(Common.SkillId.SKILL_ID_COMBAT)
                        .setNewLv(3))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("combat", json.get("skill").getAsString(),
                "skill 必须剥成 'combat'，否则 skill_lv_up 恒 noOp");
    }

    @Test
    void bridgeSkillCapChangedStripsSkillEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillCapChanged(Envelope.SkillCapChanged.newBuilder()
                        .setCharId(1)
                        .setSkill(Common.SkillId.SKILL_ID_FORGING)
                        .setNewCap(2))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("forging", json.get("skill").getAsString(),
                "skill 必须剥成 'forging'，否则 skill_cap_changed 恒 noOp");
    }

    @Test
    void bridgeSkillScrollUsedStripsSkillEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSkillScrollUsed(Envelope.SkillScrollUsed.newBuilder()
                        .setCharId(1)
                        .setScrollId("scroll-1")
                        .setSkill(Common.SkillId.SKILL_ID_ALCHEMY)
                        .setXpGranted(5)
                        .setWasDuplicate(false))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("alchemy", json.get("skill").getAsString(),
                "skill 必须剥成 'alchemy'，否则 skill_scroll_used 恒 noOp（残卷顿悟永不落地）");
    }

    @Test
    void bridgeAlchemyOutcomeResolvedStripsBucketEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAlchemyOutcomeResolved(Envelope.AlchemyOutcomeResolved.newBuilder()
                        .setBucket(Envelope.AlchemyOutcomeBucket.ALCHEMY_OUTCOME_BUCKET_PERFECT))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("perfect", json.get("bucket").getAsString(),
                "bucket 必须剥成 'perfect'（AlchemyProgressHudPlanner/AlchemyScreen switch），"
                + "否则炼丹结果 HUD/试药史恒显示灰色默认'炼废'标签");
    }

    @Test
    void bridgeForgeSessionStripsTopLevelCurrentStepEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeSession(Envelope.ForgeSessionData.newBuilder()
                        .setSessionId(1)
                        .setBlueprintId("bp-1")
                        .setActive(true)
                        .setCurrentStep(Envelope.ForgeStep.FORGE_STEP_TEMPERING)
                        .setStepIndex(1)
                        .setAchievedTier(0))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("tempering", json.get("current_step").getAsString(),
                "顶层 current_step 必须剥成 'tempering'（此前 bridgeForgeSession 只剥了嵌套 "
                + "step_state 的 oneof tag，顶层字段从未被剥），否则 ForgeScreen 的三段式 UI "
                + "路由（tempering/inscription/consecration）永久卡死/空白");
    }

    @Test
    void bridgeAlchemyContaminationStripsPerLevelColorToPascalCase() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAlchemyContamination(Envelope.AlchemyContamination.newBuilder()
                        .addLevels(Envelope.AlchemyContaminationLevel.newBuilder()
                                .setColor(Common.ColorKind.COLOR_KIND_MELLOW)
                                .setCurrent(1.0)
                                .setMax(10.0)
                                .setOk(true))
                        .addLevels(Envelope.AlchemyContaminationLevel.newBuilder()
                                .setColor(Common.ColorKind.COLOR_KIND_VIOLENT)
                                .setCurrent(2.0)
                                .setMax(20.0)
                                .setOk(false)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonArray levels = json.getAsJsonArray("levels");
        assertEquals("Mellow", levels.get(0).getAsJsonObject().get("color").getAsString(),
                "level[0].color 必须剥成 'Mellow'（首字母大写单词，AlchemyContaminationHandler "
                + "用 String.equals(\"Mellow\") 精确比对，不是全小写），否则中毒警示恒 0/0/true");
        assertEquals("Violent", levels.get(1).getAsJsonObject().get("color").getAsString(),
                "level[1].color 必须剥成 'Violent'");
    }

    @Test
    void bridgeBotanyPlantV2RenderProfilesStripsModelOverlayEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setBotanyPlantV2RenderProfiles(Envelope.BotanyPlantV2RenderProfiles.newBuilder()
                        .addProfiles(Envelope.BotanyPlantV2RenderProfile.newBuilder()
                                .setPlantId("plant-1")
                                .setBaseMeshRef("mesh-1")
                                .setTintRgb(0xFFFFFF)
                                .setModelOverlay(Envelope.BotanyModelOverlay.BOTANY_MODEL_OVERLAY_DUAL_PHASE)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonArray profiles = json.getAsJsonArray("profiles");
        assertEquals("dual_phase", profiles.get(0).getAsJsonObject().get("model_overlay").getAsString(),
                "profiles[0].model_overlay 必须剥成 'dual_phase'（BotanyPlantRenderProfile."
                + "fromWireName switch 只认全小写裸词），否则发光/昼夜双相植物视觉恒回落 NONE");
    }

    @Test
    void bridgeGatheringSessionStripsTargetTypeAndQualityHintEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setGatheringSession(Envelope.GatheringSession.newBuilder()
                        .setSessionId("session-1")
                        .setTargetName("铁矿")
                        .setTargetType(Envelope.GatheringTargetType.GATHERING_TARGET_TYPE_ORE)
                        .setQualityHint(Envelope.GatheringQualityHint.GATHERING_QUALITY_HINT_PERFECT_POSSIBLE))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("ore", json.get("target_type").getAsString(),
                "target_type 必须剥成 'ore'（GatheringSessionViewModel.displayTargetName switch），"
                + "否则采集目标标签恒回落默认'草药'");
        assertEquals("perfect_possible", json.get("quality_hint").getAsString(),
                "quality_hint 必须剥成 'perfect_possible'（qualityLabel/hasPerfectQualityHint），"
                + "否则极品品质提示标签永不显示");
    }

    @Test
    void bridgeLingtianSessionStripsKindEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setLingtianSession(Envelope.LingtianSessionData.newBuilder()
                        .setActive(true)
                        .setKind(Envelope.LingtianSessionKind.LINGTIAN_SESSION_KIND_PLANTING)
                        .setPosX(1)
                        .setPosY(2)
                        .setPosZ(3))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("planting", json.get("kind").getAsString(),
                "kind 必须剥成 'planting'（LingtianSessionStore.Kind.fromWire switch），"
                + "否则灵田 HUD 恒标 '开垦' 无论实际种植/收获/翻新/补灵/吸灵");
    }

    @Test
    void bridgeCarrierStateStripsPhaseEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCarrierState(Envelope.CarrierState.newBuilder()
                        .setCarrier("carrier-1")
                        .setPhase(Envelope.CarrierChargePhase.CARRIER_CHARGE_PHASE_CHARGING)
                        .setProgress(0.5f))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("charging", json.get("phase").getAsString(),
                "phase 必须剥成 'charging'（CarrierStateHandler.readPhase switch），"
                + "否则载体充能相位恒回落 IDLE");
    }

    @Test
    void bridgeFalseSkinStateStripsTopLevelKindAndPerLayerTierEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setFalseSkinState(Envelope.FalseSkinState.newBuilder()
                        .setTargetId("target-1")
                        .setKind(Envelope.FalseSkinKind.FALSE_SKIN_KIND_ROTTEN_WOOD_ARMOR)
                        .setLayersRemaining(1)
                        .addLayers(Envelope.FalseSkinLayerState.newBuilder()
                                .setTier(Envelope.FalseSkinTier.FALSE_SKIN_TIER_HEAVY)
                                .setSpiritQuality(1.0)
                                .setDamageCapacity(1.0)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("rotten_wood_armor", json.get("kind").getAsString(),
                "顶层 kind 必须剥成 'rotten_wood_armor'（tierForLegacyKind 字面量比对）");
        JsonArray layers = json.getAsJsonArray("layers");
        assertEquals("heavy", layers.get(0).getAsJsonObject().get("tier").getAsString(),
                "layers[0].tier 必须剥成 'heavy'（sanitizeTier switch），否则每层伪装皮 tier "
                + "永久被兜底成最低档 'fan'");
    }

    @Test
    void bridgeQiColorObservedStripsMainAndSecondaryEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setQiColorObserved(Envelope.QiColorObserved.newBuilder()
                        .setObserver("observer-1")
                        .setObserved("observed-1")
                        .setMain(Common.ColorKind.COLOR_KIND_SHARP)
                        .setSecondary(Common.ColorKind.COLOR_KIND_HEAVY))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("sharp", json.get("main").getAsString(),
                "main 必须剥成 'sharp'（ColorKind.fromWire 大小写不敏感比对，standard 小写足够），"
                + "否则神识观色功能对每条消息恒 noOp");
        assertEquals("heavy", json.get("secondary").getAsString(),
                "secondary 必须同理剥成 'heavy'");
    }

    @Test
    void bridgeSpiritualSenseTargetsStripsPerEntryKindToPascalCase() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSpiritualSenseTargets(Envelope.SpiritualSenseTargets.newBuilder()
                        .addEntries(Envelope.SenseEntry.newBuilder()
                                .setKind(Envelope.SenseKind.SENSE_KIND_ZHENFA_WARD_ALERT)
                                .setX(1).setY(2).setZ(3).setIntensity(0.5))
                        .addEntries(Envelope.SenseEntry.newBuilder()
                                .setKind(Envelope.SenseKind.SENSE_KIND_DYING_ELDER_QI)
                                .setX(4).setY(5).setZ(6).setIntensity(0.9))
                        .setGeneration(1))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonArray entries = json.getAsJsonArray("entries");
        assertEquals("ZhenfaWardAlert", entries.get(0).getAsJsonObject().get("kind").getAsString(),
                "entries[0].kind 必须剥成多段 PascalCase 'ZhenfaWardAlert'（SenseKind.fromWire "
                + "精确匹配，不是全小写也不是单段首字母大写），否则阵法警示恒回落 LIVING_QI");
        assertEquals("DyingElderQi", entries.get(1).getAsJsonObject().get("kind").getAsString(),
                "entries[1].kind 必须剥成 'DyingElderQi'（垂死大能感知视觉差异化前提）");
    }

    @Test
    void bridgeEventAlertStripsEventEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventAlert(Envelope.EventAlert.newBuilder()
                        .setEvent(Envelope.EventKind.EVENT_KIND_REALM_COLLAPSE)
                        .setMessage("境界崩塌"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("realm_collapse", json.get("event").getAsString(),
                "event 必须剥成 'realm_collapse'（EventAlertHandler.parseRealmCollapseHudState "
                + "字面量比对），否则境界崩塌倒计时 HUD 永不激活、toast 标题永远泄漏裸 "
                + "'Event Kind Realm Collapse' 前缀");
    }

    // ═══════════════════════════════════════════════════════════════════
    // plan-wire-format-bridge-v1 P1／RC2 枚举前缀 fixup — warn/info 批（12 条）
    // ═══════════════════════════════════════════════════════════════════

    @Test
    void bridgeExtractStartedStripsPortalKindEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setExtractStarted(Envelope.ExtractStarted.newBuilder()
                        .setPlayerId("player-1")
                        .setPortalEntityId(1)
                        .setPortalKind(Envelope.RiftPortalKind.RIFT_PORTAL_KIND_DEEP_RIFT)
                        .setRequiredTicks(100)
                        .setAtTick(1))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("deep_rift", json.get("portal_kind").getAsString(),
                "portal_kind 必须剥成 'deep_rift'（ExtractProgressHudPlanner.kindLabel），"
                + "否则撤离进度条永远显示通用回落标签而非真实裂缝类型");
    }

    @Test
    void bridgeExtractAbortedStripsReasonEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setExtractAborted(Envelope.ExtractAborted.newBuilder()
                        .setPlayerId("player-1")
                        .setReason(Envelope.ExtractAbortedReason.EXTRACT_ABORTED_REASON_OUT_OF_RANGE))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("out_of_range", json.get("reason").getAsString(),
                "reason 必须剥成 'out_of_range'（ExtractStateStore.reasonLabel/"
                + "isRejectionReason switch），否则撤离中断原因永远显示未明，且拒绝类"
                + "原因错误触发红屏闪烁");
    }

    @Test
    void bridgeExtractFailedStripsReasonEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setExtractFailed(Envelope.ExtractFailed.newBuilder()
                        .setPlayerId("player-1")
                        .setReason(Envelope.ExtractFailedReason.EXTRACT_FAILED_REASON_SPIRIT_QI_DRAINED))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("spirit_qi_drained", json.get("reason").getAsString(),
                "reason 必须剥成 'spirit_qi_drained'（ExtractStateStore.reasonLabel），"
                + "否则撤离失败横幅永远显示'未明'而非'真元耗尽'");
    }

    @Test
    void bridgeForgeOutcomeStripsBucketAndColorEnumPrefixes() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setForgeOutcome(Envelope.ForgeOutcome.newBuilder()
                        .setSessionId(1)
                        .setBlueprintId("bp-1")
                        .setBucket(Envelope.ForgeOutcomeBucket.FORGE_OUTCOME_BUCKET_PERFECT)
                        .setQuality(1.0f)
                        .setColor(Common.ColorKind.COLOR_KIND_SHARP)
                        .setAchievedTier(3))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("perfect", json.get("bucket").getAsString(),
                "bucket 必须剥成 'perfect'（ForgeScreen '上次结果' 展示行），否则永远显示裸 "
                + "proto 常量 'FORGE_OUTCOME_BUCKET_PERFECT'");
        assertEquals("sharp", json.get("color").getAsString(),
                "color 必须剥成 'sharp'（ForgeScreen 色=xxx 展示行）");
    }

    @Test
    void bridgeRealmVisionParamsStripsFogShapeEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRealmVisionParams(Envelope.RealmVisionParams.newBuilder()
                        .setFogStart(10.0)
                        .setFogEnd(50.0)
                        .setFogShape(Envelope.FogShape.FOG_SHAPE_SPHERE))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("sphere", json.get("fog_shape").getAsString(),
                "fog_shape 必须剥成 'sphere'（FogShape.fromWire 接受 'Sphere'/'sphere'，剥前缀"
                + "转小写落在接受集合内），否则高境界球形雾效永远退化成默认 CYLINDER");
    }

    @Test
    void bridgeSpiritTreasureDialogueStripsNestedToneEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSpiritTreasureDialogue(Envelope.SpiritTreasureDialogueProto.newBuilder()
                        .setDialogue(Envelope.SpiritTreasureDialogueData.newBuilder()
                                .setRequestId("req-1")
                                .setCharacterId("char-1")
                                .setTreasureId("treasure-1")
                                .setText("……")
                                .setTone(Envelope.SpiritTreasureDialogueTone.SPIRIT_TREASURE_DIALOGUE_TONE_COLD)
                                .setAffinityDelta(0.1))
                        .setDisplayName("某灵宝")
                        .setZone("zone-1"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject dialogue = json.getAsJsonObject("dialogue");
        assertEquals("cold", dialogue.get("tone").getAsString(),
                "嵌套 dialogue.tone 必须剥成 'cold'（JiZhaoJingTabPanel 直接把它拼进玩家可见"
                + "文案），否则永久污染显示裸 proto 常量 'SPIRIT_TREASURE_DIALOGUE_TONE_COLD'");
    }

    @Test
    void bridgeCraftOutcomeFailedStripsReasonEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                        .setFailed(Envelope.CraftOutcomeFailed.newBuilder()
                                .setV(1)
                                .setPlayerId("player-1")
                                .setRecipeId("recipe-1")
                                .setReason(Envelope.CraftFailureReason.CRAFT_FAILURE_REASON_PLAYER_CANCELLED)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("failed", json.get("kind").getAsString(),
                "craft_outcome oneof 判别 kind 必须仍是 'failed'（bridgeOneofFlat 摊平逻辑"
                + "不能因新增 reason 剥离而破坏）");
        assertEquals("player_cancelled", json.get("reason").getAsString(),
                "reason 必须剥成 'player_cancelled'（当前虽是 dead field，仍需与其余枚举字段"
                + "同规格，防止未来接线时静默复发）");
    }

    @Test
    void bridgeCraftOutcomeCompletedStillWorksAfterReasonFixup() {
        // 回归防线：给 craft_outcome 加 reason 剥离不能破坏既有 completed 变体路径。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftOutcome(Envelope.CraftOutcome.newBuilder()
                        .setCompleted(Envelope.CraftOutcomeCompleted.newBuilder()
                                .setV(1)
                                .setPlayerId("player-1")
                                .setRecipeId("recipe-1")
                                .setOutputTemplate("output-1")
                                .setOutputCount(2)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals("completed", json.get("kind").getAsString());
        assertEquals("output-1", json.get("output_template").getAsString());
        assertEquals(2, json.get("output_count").getAsInt());
    }

    @Test
    void bridgeInventorySnapshotStripsForgeColorAcrossHotbarPlacedAndEquipped() {
        Envelope.InventoryItemView.Builder hotbarItem = Envelope.InventoryItemView.newBuilder()
                .setInstanceId(1)
                .setItemId("sword-1")
                .setDisplayName("锐剑")
                .setForgeColor(Common.ColorKind.COLOR_KIND_SHARP);
        Envelope.InventoryItemView.Builder placedItem = Envelope.InventoryItemView.newBuilder()
                .setInstanceId(2)
                .setItemId("armor-1")
                .setDisplayName("厚甲")
                .setForgeColor(Common.ColorKind.COLOR_KIND_HEAVY);
        Envelope.InventoryItemView.Builder wornItem = Envelope.InventoryItemView.newBuilder()
                .setInstanceId(3)
                .setItemId("helm-1")
                .setDisplayName("醇盔")
                .setForgeColor(Common.ColorKind.COLOR_KIND_MELLOW);

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventorySnapshot(Envelope.InventorySnapshot.newBuilder()
                        .setRevision(1)
                        .addPlacedItems(Envelope.PlacedInventoryItem.newBuilder()
                                .setContainerId("bag-1")
                                .setRow(0).setCol(0)
                                .setItem(placedItem))
                        .setEquipped(Envelope.EquippedInventorySnapshot.newBuilder()
                                .addHeadWorn(wornItem))
                        .addHotbar(Envelope.HotbarSlot.newBuilder().setItem(hotbarItem)))
                .build();

        JsonObject json = bridgeAndParse(envelope);

        JsonObject hotbarSlot0 = json.getAsJsonArray("hotbar").get(0).getAsJsonObject();
        assertEquals("Sharp", hotbarSlot0.get("forge_color").getAsString(),
                "hotbar[0].forge_color 必须剥成 'Sharp'（ItemTooltipPanel.forgeColorLabel "
                + "精确 switch），否则染色武器 tooltip 永远显示裸 proto 常量");

        JsonObject placed0 = json.getAsJsonArray("placed_items").get(0).getAsJsonObject()
                .getAsJsonObject("item");
        assertEquals("Heavy", placed0.get("forge_color").getAsString(),
                "placed_items[0].item.forge_color 必须剥成 'Heavy'");

        JsonObject headWorn0 = json.getAsJsonObject("equipped")
                .getAsJsonArray("head_worn").get(0).getAsJsonObject();
        assertEquals("Mellow", headWorn0.get("forge_color").getAsString(),
                "equipped.head_worn[0].forge_color 必须剥成 'Mellow'");
    }

    // ═══════════════════════════════════════════════════════════════════
    // plan-wire-format-bridge-v1 P3／RC6 字段名漂移 / proto 里不存在
    // (docs/wire-format-bridge-audit-report.md RC6 节, 11 条 + 从 P2 移入 2 条)
    // ═══════════════════════════════════════════════════════════════════

    // ─── RC6 #1 crit: craft_recipe_list.recipes[].station（proto 补字段） ───
    @Test
    void craftRecipeListCarriesStationSoWorkbenchAndHandcraftScreensDoNotCrossLeak() {
        Envelope.CraftRecipeEntry workbenchRecipe = Envelope.CraftRecipeEntry.newBuilder()
                .setId("craft.example.iron_helm")
                .setCategory(Envelope.CraftCategory.CRAFT_CATEGORY_ARMOR_CRAFT)
                .setDisplayName("铁盔")
                .addMaterials(Envelope.CraftMaterialPair.newBuilder()
                        .setTemplateId("iron_ingot").setCount(3))
                .setQiCost(0.0)
                .setTimeTicks(1200)
                .setOutput(Envelope.CraftOutputPair.newBuilder()
                        .setTemplateId("iron_helm").setCount(1))
                .setRequirements(Envelope.CraftRequirements.newBuilder())
                .setUnlocked(true)
                .setStation("workbench")
                .build();
        Envelope.CraftRecipeEntry handcraftRecipe = Envelope.CraftRecipeEntry.newBuilder()
                .setId("craft.example.herb_knife")
                .setCategory(Envelope.CraftCategory.CRAFT_CATEGORY_TOOL)
                .setDisplayName("采药刀")
                .setQiCost(0.0)
                .setTimeTicks(600)
                .setOutput(Envelope.CraftOutputPair.newBuilder()
                        .setTemplateId("herb_knife").setCount(1))
                .setRequirements(Envelope.CraftRequirements.newBuilder())
                .setUnlocked(true)
                // 无 setStation：手搓配方，缺省(未设置)=手搓。
                .build();
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftRecipeList(Envelope.CraftRecipeList.newBuilder()
                        .setV(1)
                        .setPlayerId("offline:Alice")
                        .addRecipes(workbenchRecipe)
                        .addRecipes(handcraftRecipe)
                        .setTs(1))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed for craft_recipe_list: " + result.errorMessage());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "craft_recipe_list must not noOp: " + route.logMessage());

        List<com.bong.client.craft.CraftRecipe> recipes = com.bong.client.craft.CraftStore.recipes();
        assertEquals(2, recipes.size());
        com.bong.client.craft.CraftRecipe workbench = recipes.stream()
                .filter(r -> r.id().equals("craft.example.iron_helm")).findFirst().orElseThrow();
        assertEquals("workbench", workbench.station(),
                "station 此前从未在 proto 里发送，恒 null → 制作台屏空 → 补字段后必须读到 'workbench'");
        assertTrue(workbench.isWorkbenchRecipe(), "带 station='workbench' 的配方必须只在制作台屏出现");
        assertFalse(workbench.isHandcraft(), "workbench 配方不能泄漏进手搓台过滤");

        com.bong.client.craft.CraftRecipe handcraft = recipes.stream()
                .filter(r -> r.id().equals("craft.example.herb_knife")).findFirst().orElseThrow();
        assertNull(handcraft.station(), "未设置 station 的配方 station 必须为 null（手搓）");
        assertTrue(handcraft.isHandcraft(), "手搓配方必须只在手搓台出现");
        assertFalse(handcraft.isWorkbenchRecipe(), "手搓配方不能被制作台屏误收");
    }

    // ─── RC6 #2 crit: event_alert.severity（无 server 数据源, 恒 WARNING 默认） ───
    @Test
    void eventAlertSeverityAlwaysFallsBackToWarningBecauseNoWireFieldExists() {
        // event_alert.severity proto 里从未存在过(只有 event/message/zone/duration_ticks)，
        // Severity.fromWireName(null) 恒返回 WARNING —— 这是已知优雅降级，非本 plan 修复范围
        // (无法从 EventKind 派生正确 severity 属游戏设计判断，非纯 wire 对齐)。此测试钉死
        // 该降级行为，防止未来有人"半修"引入不一致。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventAlert(Envelope.EventAlert.newBuilder()
                        .setEvent(Envelope.EventKind.EVENT_KIND_THUNDER_TRIBULATION)
                        .setMessage("天劫将至"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertFalse(json.has("severity"), "severity 字段在 proto EventAlert 里从未存在过");

        String legacyJson = json.toString();
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "event_alert 仍应正常路由（severity 缺失不导致 noOp）: "
                + route.logMessage());
        ServerDataDispatch.ToastSpec toastSpec = route.dispatch().alertToast().orElseThrow();
        assertEquals(EventAlertHandler.WARNING_COLOR, toastSpec.color(),
                "缺失 severity 恒 fallback 到 WARNING 色（Severity.fromWireName(null)）");
    }

    // ─── RC6 #3 crit: ui_open.template_id（server 从未populate过，整条路径恒死） ───
    @Test
    void uiOpenRawXmlPathStillWorksWhileTemplateIdRemainsUnpopulatedByServer() {
        // ui_open.template_id: server 端 UiOpen 从未有过 template 概念（proto_convert.rs
        // 只有 ui/xml 两字段），resolveTemplateOpenState 分支永远拿不到非空 template_id。
        // 唯一功能路径(raw ui+xml)不受影响 —— 钉死这条仍然工作。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setUiOpen(Envelope.UiOpen.newBuilder()
                        .setUi("cultivation_panel")
                        .setXml("<flow-layout><label text=\"修仙面板\"/></flow-layout>"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertFalse(json.has("template_id"), "template_id 从未在 proto UiOpen 里存在过");
        assertEquals("cultivation_panel", json.get("ui").getAsString());
    }

    // ─── RC6 #4 warn: techniques_snapshot.aliases（proto 无此字段, 恒空列表但优雅降级） ───
    @Test
    void techniquesSnapshotAliasesAlwaysEmptyButIdAndDisplayNameSearchStillWork() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setTechniquesSnapshot(Envelope.TechniquesSnapshot.newBuilder()
                        .addEntries(Envelope.TechniqueEntry.newBuilder()
                                .setId("technique.flying_sword")
                                .setDisplayName("御剑术")
                                .setGrade("earth")))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "techniques_snapshot must not noOp: " + route.logMessage());

        List<TechniquesListPanel.Technique> snapshot = TechniquesListPanel.snapshot();
        assertEquals(1, snapshot.size());
        assertTrue(snapshot.get(0).aliases().isEmpty(),
                "aliases 在 proto TechniqueEntry 里从未存在过，恒空列表（无源数据可补，属"
                + "已知优雅降级——alias 搜索永远不命中，但 id/display_name 搜索仍可用）");
    }

    // ─── RC6 #5 warn: combat_event enrichment 簇（无 server 数据源, kind-based 降级） ───
    @Test
    void combatEventFloaterDegradesGracefullyToKindBasedDefaultsWithoutEnrichmentFields() {
        // school/tier/attacker_uuid/target_uuid/direction/rare_drop/kill/perfect 等富化字段
        // 在 CombatEventFloaterEntry proto 里从未存在过(只有 kind/amount/text/x/y/z)，且服务端
        // 内部结构体同样从未携带过这些数据——属新功能范畴（需要 uuid 追踪/完美招架判定等全新
        // 服务端逻辑），非本 plan 的"wire 形状对齐"范围。此测试钉死 kind 字符串驱动的基础路径
        // 仍然工作（不因富化字段缺失而 noOp）。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCombatEventFloater(Envelope.CombatEventFloater.newBuilder()
                        .addEvents(Envelope.CombatEventFloaterEntry.newBuilder()
                                .setKind("damage")
                                .setAmount(12.5f)
                                .setText("12.5")
                                .setX(1.0).setY(2.0).setZ(3.0)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject event = json.getAsJsonArray("events").get(0).getAsJsonObject();
        assertEquals("damage", event.get("kind").getAsString());
        assertFalse(event.has("school"), "school 从未在 proto CombatEventFloaterEntry 里存在过");
        assertFalse(event.has("attacker_uuid"), "attacker_uuid 同理，从未存在过");
        assertFalse(event.has("perfect"), "perfect 同理，从未存在过");
    }

    // ─── RC6 #6 warn: heart_demon_offer.choices[].alignment/cost_summary/cost_flavor ───
    @Test
    void heartDemonOfferChoiceUsesNeutralAlignmentAndGenericCostFallbackWithoutWireData() {
        // alignment/cost_summary/cost_flavor 在 HeartDemonOfferChoice proto 里从未存在过
        // (只有 choice_id/category/title/effect_summary/flavor/style_hint 6 字段)。
        // HeartDemonOfferHandler 已对这三字段做 fallback()，行为等价于 InsightChoice 的
        // 6 参数便捷构造器（NEUTRAL alignment + "代价待结算"/"心魔会索取对应代价。"）——
        // 钉死该降级契约。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setHeartDemonOffer(Envelope.HeartDemonOffer.newBuilder()
                        .setTriggerId("hd-1")
                        .addChoices(Envelope.HeartDemonOfferChoice.newBuilder()
                                .setChoiceId("choice-1")
                                .setCategory("Composure")
                                .setTitle("忍耐")
                                .setEffectSummary("composure +10%")
                                .setFlavor("心魔低语。")
                                .setStyleHint("忍")))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "heart_demon_offer must not noOp: " + route.logMessage());

        com.bong.client.insight.InsightOfferViewModel offer =
                com.bong.client.insight.InsightOfferStore.snapshot();
        com.bong.client.insight.InsightChoice choice = offer.choices().get(0);
        assertEquals(com.bong.client.insight.InsightAlignment.NEUTRAL, choice.alignment(),
                "alignment 无源数据，恒 NEUTRAL（无法从 wire 判断阵营倾向）");
        assertEquals("代价待结算", choice.costSummary(),
                "cost_summary 无源数据，恒通用兜底文案");
        assertEquals("心魔会索取对应代价。", choice.costFlavor(),
                "cost_flavor 无源数据，恒通用兜底文案");
    }

    // ─── RC6 #7 warn: event_alert.effect（proto 无此字段, VisualEffectState.none() 降级） ───
    @Test
    void eventAlertEffectAlwaysEmptyBecauseNoEffectSubmessageExistsOnWire() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventAlert(Envelope.EventAlert.newBuilder()
                        .setEvent(Envelope.EventKind.EVENT_KIND_BEAST_TIDE)
                        .setMessage("兽潮将至"))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertFalse(json.has("effect"), "effect 子消息在 proto EventAlert 里从未存在过");
    }

    // ─── RC6 #8 info: zone_info.display_name（Zone 服务端类型无展示名概念） ───
    @Test
    void zoneInfoDisplayNameNeverExistsOnWireBecauseServerZoneHasNoDisplayNameConcept() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setZoneInfo(Envelope.ZoneInfo.newBuilder()
                        .setZone("qingyun_peaks")
                        .setSpiritQi(0.8)
                        .setDangerLevel(1))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertFalse(json.has("display_name"),
                "display_name 从未在 proto ZoneInfo 里存在过（server Zone 类型只有 name 字段，"
                + "无独立展示名概念——非纯 wire 对齐可修，需先立 zone 展示名 plan）");
    }

    // ─── RC6 #9 info: craft_session_state.error（proto 无此字段, 恒空字符串） ───
    @Test
    void craftSessionStateErrorAlwaysEmptyBecauseNoErrorFieldExistsOnWire() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCraftSessionState(Envelope.CraftSessionState.newBuilder()
                        .setV(1)
                        .setPlayerId("offline:Alice")
                        .setActive(true)
                        .setRecipeId("craft.example.foo")
                        .setElapsedTicks(30)
                        .setTotalTicks(100)
                        .setCompletedCount(1)
                        .setTotalCount(3)
                        .setTs(1))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertFalse(json.has("error"), "error 从未在 proto CraftSessionState 里存在过");
    }

    // ─── RC6 #10 info: combat_event.color（proto 无此字段, 恒 kind 派生默认色） ───
    @Test
    void combatEventColorAlwaysAbsentSoFloaterUsesKindDerivedDefaultColor() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setCombatEventFloater(Envelope.CombatEventFloater.newBuilder()
                        .addEvents(Envelope.CombatEventFloaterEntry.newBuilder()
                                .setKind("heal")
                                .setAmount(5.0f)
                                .setText("+5")
                                .setX(0).setY(0).setZ(0)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        JsonObject event = json.getAsJsonArray("events").get(0).getAsJsonObject();
        assertFalse(event.has("color"), "color 从未在 proto CombatEventFloaterEntry 里存在过");
    }

    // ─── RC6 #11 info: event_alert.duration_ms（改读真实 duration_ticks, tick→ms 换算） ───
    @Test
    void eventAlertDurationMsDerivesFromRealDurationTicksInsteadOfDeadField() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setEventAlert(Envelope.EventAlert.newBuilder()
                        .setEvent(Envelope.EventKind.EVENT_KIND_POISON_MIASMA)
                        .setMessage("毒瘴弥漫")
                        .setDurationTicks(200)) // 200 tick * 50ms/tick = 10_000ms
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "event_alert must not noOp: " + route.logMessage());
        ServerDataDispatch.ToastSpec toastSpec = route.dispatch().alertToast().orElseThrow();
        assertEquals(10_000L, toastSpec.durationMillis(),
                "duration_ms 此前读一个从未存在的字段恒 fallback 到 severity 默认时长；"
                + "改读真实 duration_ticks(uint64, proto 确有此字段)并按 1 tick=50ms 换算后，"
                + "服务端应可精确控制 toast 展示时长");
    }

    // ─── RC6 #12 warn(从 P2 移入): player_state.zone_label（无 server 数据源，恒 null） ───
    @Test
    void playerStateZoneLabelAlwaysNullBecauseServerZoneHasNoDisplayNameConcept() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPlayerState(Envelope.PlayerState.newBuilder()
                        .setRealm(Common.Realm.REALM_CONDENSE)
                        .setSpiritQi(78.0)
                        .setSpiritQiMax(100.0)
                        .setKarma(0.2)
                        .setCompositePower(0.35)
                        .setZone("blood_valley")
                        .setBreakdown(Envelope.PlayerPowerBreakdown.newBuilder()
                                .setCombat(0.2).setWealth(0.4).setSocial(0.65)
                                .setKarma(0.2).setTerritory(0.1)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertFalse(json.has("zone_label"), "zone_label 从未在 proto PlayerState 里存在过");
    }

    // ─── RC6 #13 warn(从 P2 移入): player_state.zone_spirit_qi（proto 已补字段, 真实读取） ───
    @Test
    void playerStateZoneSpiritQiReadsRealValueOnceProtoFieldIsPopulated() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setPlayerState(Envelope.PlayerState.newBuilder()
                        .setRealm(Common.Realm.REALM_CONDENSE)
                        .setSpiritQi(78.0)
                        .setSpiritQiMax(100.0)
                        .setKarma(0.2)
                        .setCompositePower(0.35)
                        .setZone("blood_valley")
                        .setZoneSpiritQi(0.42)
                        .setBreakdown(Envelope.PlayerPowerBreakdown.newBuilder()
                                .setCombat(0.2).setWealth(0.4).setSocial(0.65)
                                .setKarma(0.2).setTerritory(0.1)))
                .build();

        JsonObject json = bridgeAndParse(envelope);
        assertEquals(0.42, json.get("zone_spirit_qi").getAsDouble(), 1e-9,
                "zone_spirit_qi 此前在 proto 里根本不存在(PlayerStateViewModel."
                + "zoneSpiritQiNormalized() 恒 NaN 归一化默认)——补字段后必须原样落地");
    }

    // ─── RC6 #14 crit(从 P2 移入): mining_progress.mineral_id/display_name（proto 补字段） ───
    @Test
    void miningProgressCarriesMineralIdAndDisplayNameInsteadOfGenericFallbackLabel() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setMiningProgress(Envelope.MiningProgress.newBuilder()
                        .setSessionId("mining:1:64:2:cu_tie")
                        .setOrePosX(1).setOrePosY(64).setOrePosZ(2)
                        .setProgress(0.3)
                        .setInterrupted(false)
                        .setCompleted(false)
                        .setMineralId("cu_tie")
                        .setDisplayName("粗铁矿脉"))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "mining_progress must not noOp: " + route.logMessage());

        com.bong.client.gathering.GatheringSessionViewModel session =
                com.bong.client.gathering.GatheringSessionStore.snapshot();
        assertEquals("粗铁矿脉", session.targetName(),
                "此前 mineral_id/display_name 从未在 proto 里存在过，GatheringProgressPayloadReader"
                + ".firstNonBlank 恒 fallback 到通用 '矿脉'；补字段后必须显示真实矿种展示名");
    }

    // ═══════════════════════════════════════════════════════════════════
    // plan-wire-format-bridge-v1 P4 — RC4/RC5：内嵌 JSON 字符串 + 其他形状不符（2 条）
    // 每条：喂真实 server 侧编码形状（proto string 字段/oneof）→ bridge() → 断言
    // handler 非 noOp + 字段落地为期望值（不是原始转义 JSON 文本/恒 default）。
    // ═══════════════════════════════════════════════════════════════════

    // ─── RC4 crit: loot_container_open.source_kind（内嵌 JSON 字符串二次解码） ───
    //
    // server proto_convert.rs: `source_kind: serde_json::to_string(&o.source_kind)`——
    // LootContainerSourceKindV1 是 serde 外部标签枚举(rename_all=snake_case)，先被
    // serde_json 序列化成 JSON，再整体塞进 proto `string` 字段。JsonFormat 对这个
    // string 字段只会原样打印被转义的 JSON 文本，handler 此前直接把这段转义文本当
    // kind 用、grade 恒 "common"。修复后 LootContainerHandler.parseSourceKind 二次
    // JsonParser.parseString 解码，必须还原出真实 kind/grade。

    @Test
    void bridgeLootContainerOpenDecodesEmbeddedSupplyCoffinSourceKind() {
        // server 侧对 SupplyCoffin{grade:"legendary"} 的实际编码：
        // serde_json::to_string(&SupplyCoffin{grade:"legendary"})
        //   == "{\"supply_coffin\":{\"grade\":\"legendary\"}}"
        String serverEncodedSourceKind = "{\"supply_coffin\":{\"grade\":\"legendary\"}}";
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setLootContainerOpen(Envelope.LootContainerOpen.newBuilder()
                        .setSessionId(101L)
                        .setSourceKind(serverEncodedSourceKind)
                        .setRows(3)
                        .setCols(4)
                        .setTimeoutWallSecs(1_800L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed: " + result.errorMessage());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "loot_container_open must not noOp: " + route.logMessage());

        LootContainerStateStore.OpenSession session =
                (LootContainerStateStore.OpenSession) LootContainerStateStore.current();
        assertEquals("supply_coffin", session.sourceKind(),
                "kind 此前恒为原始转义 JSON 文本 '{\"supply_coffin\":...}'（isJsonPrimitive 分支"
                + "短路了 isJsonObject/variant-key 解析）；二次解码后必须还原成 'supply_coffin'");
        assertEquals("legendary", session.grade(),
                "grade 此前恒 hardcode 'common'（丢失真实稀有度）；二次解码后必须还原成 'legendary'");
    }

    @Test
    void bridgeLootContainerOpenDecodesEmbeddedStorageCrateSourceKind() {
        // server 侧对 StorageCrate{is_herb:true} 的实际编码。
        String serverEncodedSourceKind = "{\"storage_crate\":{\"is_herb\":true}}";
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setLootContainerOpen(Envelope.LootContainerOpen.newBuilder()
                        .setSessionId(102L)
                        .setSourceKind(serverEncodedSourceKind)
                        .setRows(4)
                        .setCols(4)
                        .setTimeoutWallSecs(0L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "loot_container_open must not noOp: " + route.logMessage());

        LootContainerStateStore.OpenSession session =
                (LootContainerStateStore.OpenSession) LootContainerStateStore.current();
        assertEquals("storage_crate", session.sourceKind());
        assertEquals("herb", session.grade(),
                "is_herb=true 内嵌在转义 JSON 字符串里，二次解码后必须还原成 herb 分类");
    }

    @Test
    void bridgeLootContainerOpenDecodesEmbeddedDeadDropSourceKind() {
        // server 侧对 unit variant DeadDrop 的实际编码：外部标签枚举把它序列化成裸
        // JSON 字符串 "dead_drop"，再整体 serde_json::to_string 一次 → proto string
        // 字段的原始内容是 "\"dead_drop\""（带字面双引号）。
        String serverEncodedSourceKind = "\"dead_drop\"";
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setLootContainerOpen(Envelope.LootContainerOpen.newBuilder()
                        .setSessionId(103L)
                        .setSourceKind(serverEncodedSourceKind)
                        .setRows(3)
                        .setCols(3)
                        .setTimeoutWallSecs(0L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "loot_container_open must not noOp: " + route.logMessage());

        LootContainerStateStore.OpenSession session =
                (LootContainerStateStore.OpenSession) LootContainerStateStore.current();
        assertEquals("dead_drop", session.sourceKind(),
                "此前 kind 会带着字面双引号变成 '\"dead_drop\"'（原始转义文本），"
                + "二次解码后必须去掉多余引号还原成 'dead_drop'");
        assertEquals("common", session.grade());
    }

    // ─── RC5 crit: recipe_unlocked.source（oneof 无判别字段，重塑三分支） ───
    //
    // UnlockEventSource proto3-JSON 对 set 的 oneof 成员直接在顶层用其自身字段名打印
    // （无 "kind" 判别字段），RecipeUnlockedHandler 却读 sourceObj.get("kind") 恒 null
    // → 整条残卷/师承/顿悟解锁通知链路此前静默永久丢弃（CraftStore.recordUnlock 从
    // 未被调用）。bridgeRecipeUnlocked 把三种 oneof 分支重塑成 {kind,...} 判别式对象。

    @Test
    void bridgeRecipeUnlockedScrollSourceRoutesToCraftStore() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRecipeUnlocked(Envelope.RecipeUnlocked.newBuilder()
                        .setV(1)
                        .setPlayerId("player-1")
                        .setRecipeId("pill_qi_gathering")
                        .setSource(Envelope.UnlockEventSource.newBuilder()
                                .setScrollItemTemplate("scroll_meridian_intro"))
                        .setUnlockedAtTick(1000L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed: " + result.errorMessage());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(),
                "recipe_unlocked(scroll) must not noOp — 此前 source.kind 恒 null: "
                + route.logMessage());

        com.bong.client.craft.CraftStore.RecipeUnlockedEvent event =
                com.bong.client.craft.CraftStore.lastUnlocked().orElseThrow(
                        () -> new AssertionError("CraftStore.recordUnlock 应被调用"));
        assertEquals("pill_qi_gathering", event.recipeId());
        assertEquals(1000L, event.unlockedAtTick());
        com.bong.client.craft.CraftStore.RecipeUnlockedEvent.Scroll scroll =
                assertInstanceOf(com.bong.client.craft.CraftStore.RecipeUnlockedEvent.Scroll.class,
                        event.source());
        assertEquals("scroll_meridian_intro", scroll.itemTemplate());
    }

    @Test
    void bridgeRecipeUnlockedMentorSourceRoutesToCraftStore() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRecipeUnlocked(Envelope.RecipeUnlocked.newBuilder()
                        .setV(1)
                        .setPlayerId("player-1")
                        .setRecipeId("armor_iron_plate")
                        .setSource(Envelope.UnlockEventSource.newBuilder()
                                .setMentorNpcArchetype("blacksmith_elder"))
                        .setUnlockedAtTick(2000L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "recipe_unlocked(mentor) must not noOp: " + route.logMessage());

        com.bong.client.craft.CraftStore.RecipeUnlockedEvent event =
                com.bong.client.craft.CraftStore.lastUnlocked().orElseThrow();
        assertEquals("armor_iron_plate", event.recipeId());
        com.bong.client.craft.CraftStore.RecipeUnlockedEvent.Mentor mentor =
                assertInstanceOf(com.bong.client.craft.CraftStore.RecipeUnlockedEvent.Mentor.class,
                        event.source());
        assertEquals("blacksmith_elder", mentor.npcArchetype());
    }

    @Test
    void bridgeRecipeUnlockedInsightSourceStripsTriggerEnumPrefix() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRecipeUnlocked(Envelope.RecipeUnlocked.newBuilder()
                        .setV(1)
                        .setPlayerId("player-1")
                        .setRecipeId("technique_breakthrough_insight")
                        .setSource(Envelope.UnlockEventSource.newBuilder()
                                .setInsightTrigger(Envelope.InsightTrigger.INSIGHT_TRIGGER_NEAR_DEATH))
                        .setUnlockedAtTick(3000L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isHandled(), "recipe_unlocked(insight) must not noOp: " + route.logMessage());

        com.bong.client.craft.CraftStore.RecipeUnlockedEvent event =
                com.bong.client.craft.CraftStore.lastUnlocked().orElseThrow();
        com.bong.client.craft.CraftStore.RecipeUnlockedEvent.Insight insight =
                assertInstanceOf(com.bong.client.craft.CraftStore.RecipeUnlockedEvent.Insight.class,
                        event.source());
        assertEquals("near_death", insight.trigger(),
                "trigger 必须从 proto 全名 INSIGHT_TRIGGER_NEAR_DEATH 剥成 'near_death'"
                + "（RecipeUnlockedHandler switch 只认 breakthrough/near_death/defeat_stronger）");
    }

    @Test
    void bridgeRecipeUnlockedWithoutSourceOneofSetIsNoOp() {
        // oneof 未设置(SOURCE_NOT_SET)：proto3 printer 完全省略 "source" 字段，
        // handler 应保持既有"缺失 source 即 noOp"契约（不是本 fixup 的回归目标）。
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRecipeUnlocked(Envelope.RecipeUnlocked.newBuilder()
                        .setV(1)
                        .setPlayerId("player-1")
                        .setRecipeId("recipe-no-source")
                        .setUnlockedAtTick(4000L))
                .build();

        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess());
        ServerDataRouter.RouteResult route = ServerDataRouter.createDefault()
                .route(result.legacyJson(), result.legacyJson().getBytes(StandardCharsets.UTF_8).length);
        assertTrue(route.isNoOp(),
                "source oneof 未设置时应保持 noOp 契约(缺字段本就不应解锁配方): "
                + route.logMessage());
        assertTrue(com.bong.client.craft.CraftStore.lastUnlocked().isEmpty());
    }

    // ═══════════════════════════════════════════════════════════════════
    // Helper
    // ═══════════════════════════════════════════════════════════════════

    private static JsonObject bridgeAndParse(Envelope.ServerDataEnvelope envelope) {
        ProtoServerDataBridge.BridgeResult result = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(result.isSuccess(), "bridge should succeed: " + result.errorMessage());
        return JsonParser.parseString(result.legacyJson()).getAsJsonObject();
    }

    /**
     * 定位 {@code ServerDataEnvelope.payload} oneof 的 descriptor。protobuf-java 4.28.3 的
     * {@code Descriptor} 没有 {@code findOneofByName}（旧版本 API 里有，这个版本没有），
     * 只能遍历 {@link Descriptors.Descriptor#getOneofs()} 按名字找。
     */
    private static Descriptors.OneofDescriptor payloadOneofDescriptor() {
        for (Descriptors.OneofDescriptor oneof :
                Envelope.ServerDataEnvelope.getDescriptor().getOneofs()) {
            if ("payload".equals(oneof.getName())) {
                return oneof;
            }
        }
        return null;
    }
}
