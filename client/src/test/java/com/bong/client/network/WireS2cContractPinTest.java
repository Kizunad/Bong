package com.bong.client.network;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.sun.source.tree.BlockTree;
import com.sun.source.tree.CatchTree;
import com.sun.source.tree.ClassTree;
import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.DoWhileLoopTree;
import com.sun.source.tree.EnhancedForLoopTree;
import com.sun.source.tree.ExpressionStatementTree;
import com.sun.source.tree.ExpressionTree;
import com.sun.source.tree.ForLoopTree;
import com.sun.source.tree.IdentifierTree;
import com.sun.source.tree.IfTree;
import com.sun.source.tree.LambdaExpressionTree;
import com.sun.source.tree.LiteralTree;
import com.sun.source.tree.MemberReferenceTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.tree.MethodTree;
import com.sun.source.tree.NewArrayTree;
import com.sun.source.tree.NewClassTree;
import com.sun.source.tree.ParenthesizedTree;
import com.sun.source.tree.ReturnTree;
import com.sun.source.tree.SwitchExpressionTree;
import com.sun.source.tree.ThrowTree;

import com.sun.source.tree.SwitchTree;
import com.sun.source.tree.TryTree;
import com.sun.source.tree.Tree;
import com.sun.source.tree.VariableTree;
import com.sun.source.tree.WhileLoopTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePath;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.TreeScanner;
import com.sun.source.util.Trees;
import org.junit.jupiter.api.Test;

import javax.lang.model.element.Element;
import javax.lang.model.element.ExecutableElement;
import javax.lang.model.element.Modifier;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

final class WireS2cContractPinTest {
    private static final String RECEIVER_OWNER =
        "net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking";
    private static final String IDENTIFIER_OWNER = "net.minecraft.util.Identifier";
    private static final String CLIENT_ENTRYPOINT = "com.bong.client.BongClient";
    private static final String BONG_BLOCK_GENERATOR_CLASS = "com/bong/client/block/BongBlockIds.java";
    private static final Set<Path> SOURCE_ROOTS = Set.of(
        Path.of("src/main/java"),
        Path.of("build/generated/sources/bongBlocks/java")
    );
    private static final Set<Path> ATTRIBUTED_SOURCES = Set.of(
        Path.of("src/main/java/com/bong/client/BongClient.java"),
        Path.of("src/main/java/com/bong/client/BongNetworkHandler.java"),
        Path.of("src/main/java/com/bong/client/daozhan/DaoZhanDisguiseHandler.java"),
        Path.of("src/main/java/com/bong/client/dying_elder/DyingElderEncounterHandler.java"),
        Path.of("src/main/java/com/bong/client/fauna/HallucinationLayerHandler.java"),
        Path.of("src/main/java/com/bong/client/fauna/RatQiTierHandler.java"),
        Path.of("src/main/java/com/bong/client/iris/IrisBootstrap.java"),
        Path.of("src/main/java/com/bong/client/iris/ShaderStateHandler.java"),
        Path.of("src/main/java/com/bong/client/network/AgentUiPayloadHandler.java"),
        Path.of("src/main/java/com/bong/client/network/InventoryEventHandler.java"),
        Path.of("src/main/java/com/bong/client/network/ProtoServerDataBridge.java"),
        Path.of("src/main/java/com/bong/client/npc/NpcBubbleHandler.java"),
        Path.of("src/main/java/com/bong/client/npc/NpcLodHandler.java"),
        Path.of("src/main/java/com/bong/client/npc/NpcMetadataHandler.java"),
        Path.of("src/main/java/com/bong/client/npc/NpcMoodHandler.java"),
        Path.of("src/main/java/com/bong/client/spider/SpiderDisguiseHandler.java"),
        Path.of("src/main/java/com/bong/client/tsy/TsyBossHealthHandler.java"),
        Path.of("src/main/java/com/bong/client/tsy/TsyDeathVfxHandler.java"),
        Path.of("src/main/java/com/bong/client/visual/particle/QiAttritionVfxPlayer.java")
    );

    private enum NormalizationMode {
        SNAKE_LOWER,
        CAPITALIZED,
        PASCAL_CASE,
        SNAKE_LOWER_OMIT_UNSPECIFIED
    }

    private record NormalizationSite(String field, String prefix, NormalizationMode mode) {}

    private record PrefixLiteralSite(Path relativePath, long line) {}

    private static final Map<NormalizationSite, Integer> BRIDGE_NORMALIZATIONS =
        bridgeNormalizations();

    private static final Map<String, Set<NormalizationSite>> BRIDGE_DISPATCH_NORMALIZATIONS =
        bridgeDispatchNormalizations();

    private enum MigrationDecision {
        MIGRATE,
        EXEMPT
    }

    private static final Map<String, MigrationDecision> SIDE_CHANNEL_DECISIONS =
        migrationDecisions();

    private static final List<Set<String>> ATOMIC_MIGRATION_GROUPS = List.of(
        Set.of("bong:audio/play", "bong:audio/stop"),
        Set.of("bong:resonance_lock", "bong:resonance_lock_end"),
        Set.of("bong:spider_disguise_enter", "bong:spider_ambush_trigger"),
        Set.of("bong:daozhan_disguise_enter", "bong:daozhan_reveal")
    );

    private static final Set<String> ENUM_PREFIXES = Set.of(
        "ALCHEMY_OUTCOME_BUCKET_",
        "BOTANY_MODEL_OVERLAY_",
        "CARRIER_CHARGE_PHASE_",
        "CAST_OUTCOME_",
        "CAST_PHASE_",
        "COLOR_KIND_",
        "CONTAINER_KIND_",
        "CRAFT_CATEGORY_",
        "CRAFT_FAILURE_REASON_",
        "DEATH_CINEMATIC_PHASE_",
        "DEATH_CINEMATIC_ZONE_KIND_",
        "DEATH_ROLL_RESULT_",
        "DEATH_SCREEN_STAGE_",
        "DEATH_SCREEN_ZONE_KIND_",
        "EVENT_CHANNEL_",
        "EVENT_KIND_",
        "EVENT_PRIORITY_",
        "EQUIP_SLOT_",
        "EQUIP_STATE_",
        "EXPOSURE_KIND_",
        "EXTRACT_ABORTED_REASON_",
        "EXTRACT_FAILED_REASON_",
        "FALSE_SKIN_KIND_",
        "FALSE_SKIN_TIER_",
        "FOG_SHAPE_",
        "FORGE_OUTCOME_BUCKET_",
        "FORGE_STEP_",
        "GATHERING_QUALITY_HINT_",
        "GATHERING_TARGET_TYPE_",
        "GUARDIAN_KIND_",
        "INSIGHT_TRIGGER_",
        "KEY_KIND_",
        "LINGTIAN_SESSION_KIND_",
        "MOVEMENT_ACTION_",
        "MOVEMENT_ACTION_REQUEST_KIND_",
        "MOVEMENT_ZONE_KIND_",
        "REALM_",
        "RIFT_PORTAL_DIRECTION_",
        "RIFT_PORTAL_KIND_",
        "SEARCH_ABORT_REASON_",
        "SEASON_",
        "SENSE_KIND_",
        "SKILL_ID_",
        "SPIRIT_TREASURE_DIALOGUE_TONE_",
        "YIDAO_SKILL_ID_"
    );

    private static final Map<String, Integer> BRIDGE_PREFIX_LITERAL_COUNTS =
        bridgePrefixLiteralCounts();

    @Test
    void everyClientS2cReceiverIsCountedAndEveryBypassHasAMigrationDecision() throws IOException {
        Path clientRoot = clientRoot();
        JavaSourceModel sourceModel = sourceModel();
        List<ReceiverSite> receiverSites = sourceModel.receiverSites();

        assertEquals(32, receiverSites.size(),
            "P0 基线为 32 个真实 Java receiver 调用（server_data 1 + side channels 31）");
        assertEquals(
            List.of(
                Path.of("com/bong/client/BongNetworkHandler.java"),
                Path.of("com/bong/client/iris/IrisBootstrap.java")
            ),
            receiverSites.stream()
                .map(ReceiverSite::relativePath)
                .distinct()
                .sorted(Comparator.comparing(Path::toString))
                .toList(),
            "新增 receiver 文件必须进入 R6 旁路普查，而不是绕过既有 bootstrap"
        );

        Map<String, List<ReceiverSite>> sitesByChannel = new LinkedHashMap<>();
        for (ReceiverSite site : receiverSites) {
            sitesByChannel.computeIfAbsent(site.channel(), ignored -> new ArrayList<>()).add(site);
        }
        List<String> duplicates = sitesByChannel.entrySet().stream()
            .filter(entry -> entry.getValue().size() > 1)
            .map(entry -> entry.getKey() + " -> " + entry.getValue())
            .toList();
        assertEquals(List.of(), duplicates,
            "每个 production receiver channel 必须唯一；重复项需同时报告 call site");
        assertEquals(32, sitesByChannel.size(), "32 个 receiver 必须解析成 32 个唯一 channel ID");
        assertEquals(1, sitesByChannel.getOrDefault("bong:server_data", List.of()).size(),
            "目标轨 bong:server_data 必须且只能注册一次");

        Set<String> actualSideChannels = new TreeSet<>(sitesByChannel.keySet());
        assertTrue(actualSideChannels.remove("bong:server_data"));
        assertEquals(
            SIDE_CHANNEL_DECISIONS.keySet(),
            actualSideChannels,
            "实际解析出的 31 个旁路必须由唯一 migration decision 账本完整覆盖"
        );
        assertEquals(31, SIDE_CHANNEL_DECISIONS.size());
        assertEquals(28, decisionCount(MigrationDecision.MIGRATE));
        assertEquals(3, decisionCount(MigrationDecision.EXEMPT));
        assertEquals(
            Set.of("bong:agent_ui_request", "bong:agent_ui_close", "bong:shader_state"),
            channelsWithDecision(MigrationDecision.EXEMPT),
            "豁免必须是实际旁路的精确子集，不能靠 31-3 的算术掩盖 stale exemption"
        );

        for (Set<String> group : ATOMIC_MIGRATION_GROUPS) {
            Set<MigrationDecision> decisions = group.stream()
                .map(SIDE_CHANNEL_DECISIONS::get)
                .collect(java.util.stream.Collectors.toSet());
            assertTrue(!decisions.contains(null),
                () -> "atomic migration group contains an unregistered channel: " + group);
            assertEquals(1, decisions.size(),
                () -> "atomic migration group must share one decision: " + group);
        }

        sourceModel.assertReceiverBootstrapsAreReachable();
    }

    @Test
    void protoEnumPrefixInventoryAndNormalizationModesStayFrozen() throws IOException {
        Path clientRoot = clientRoot();
        JavaSourceModel sourceModel = sourceModel();

        assertEquals(
            BRIDGE_NORMALIZATIONS,
            sourceModel.bridgeNormalizations(),
            "ProtoServerDataBridge 的 prefix→field→mode 多重集漂移时必须更新 R6 normalization 账本"
        );
        assertEquals(
            BRIDGE_DISPATCH_NORMALIZATIONS,
            sourceModel.bridgeDispatchNormalizations(),
            "每个 protobuf payload getter 必须绑定到自身 converter 的 field/prefix normalization，不得只靠全局 multiset"
        );
        int bridgeReferences = BRIDGE_NORMALIZATIONS.values().stream()
            .mapToInt(Integer::intValue)
            .sum();
        assertEquals(43, BRIDGE_NORMALIZATIONS.keySet().stream()
            .map(NormalizationSite::prefix)
            .collect(java.util.stream.Collectors.toSet()).size());
        assertEquals(58, bridgeReferences,
            "P0 semantic bridge normalization ledger must cover every reachable field operation");
        assertEquals(
            BRIDGE_PREFIX_LITERAL_COUNTS,
            sourceModel.bridgePrefixLiteralCounts(),
            "P0 bridge source must retain the exact 43-prefix/57-literal lexical multiset"
        );
        assertEquals(
            57,
            BRIDGE_PREFIX_LITERAL_COUNTS.values().stream().mapToInt(Integer::intValue).sum(),
            "P0 bridge lexical baseline remains 57 prefix literals"
        );

        assertEquals(
            List.of(
                new NormalizationSite("slot", "EQUIP_SLOT_", NormalizationMode.SNAKE_LOWER),
                new NormalizationSite("state", "EQUIP_STATE_", NormalizationMode.SNAKE_LOWER)
            ),
            sourceModel.inventoryNormalizations(),
            "P0 inventory equip location must normalize both slot and required state"
        );

        sourceModel.assertProductionPrefixLiteralInventory();
        assertEquals(45, sourceModel.productionPrefixLiteralCount(),
            "完整 production receive path 基线为 45 个 enum 前缀 literal");
        assertEquals(60, bridgeReferences + sourceModel.inventoryNormalizations().size(),
            "semantic normalization ledger includes reachable helper reuse, array-element normalization, and inventory exceptions");
    }

    @Test
    void directCallScannerRejectsConditionalEarlyExit() throws IOException {
        assertEquals(
            List.of("always", "nested"),
            JavaSourceModel.directCallsInSyntheticSource("""
                final class Wiring {
                    static void early(boolean enabled) {
                        if (!enabled) {
                            return;
                        }
                        register();
                    }

                    static void thrown(boolean enabled) {
                        if (!enabled) {
                            throw new IllegalStateException();
                        }
                        register();
                    }

                    static void nested() {
                        Runnable deferred = () -> { return; };
                        register();
                    }

                    static void always() {
                        register();
                    }

                    static void register() {}
                }
                """),
            "conditional early return must make following wiring non-unconditional without treating lambda returns as method exits"
        );
    }

    private static JavaSourceModel sourceModel() throws IOException {
        return JavaSourceModelHolder.INSTANCE;
    }

    private static final class JavaSourceModelHolder {
        private static final JavaSourceModel INSTANCE = parseModel();

        private static JavaSourceModel parseModel() {
            try {
                return JavaSourceModel.parse(clientRoot());
            } catch (IOException exception) {
                throw new ExceptionInInitializerError(exception);
            }
        }
    }
    private static Map<String, Integer> bridgePrefixLiteralCounts() {
        Map<String, Integer> counts = new HashMap<>();
        for (NormalizationSite site : BRIDGE_NORMALIZATIONS.keySet()) {
            counts.putIfAbsent(site.prefix(), 0);
        }
        for (String prefix : counts.keySet()) {
            counts.put(prefix, switch (prefix) {
                case "COLOR_KIND_" -> 10;
                case "SKILL_ID_" -> 4;
                case "GUARDIAN_KIND_", "RIFT_PORTAL_KIND_" -> 2;
                default -> 1;
            });
        }
        return Map.copyOf(counts);
    }

    private static Map<NormalizationSite, Integer> bridgeNormalizations() {
        Map<NormalizationSite, Integer> normalizations = new LinkedHashMap<>();
        addNormalizations(normalizations, NormalizationMode.SNAKE_LOWER,
            new String[][] {
                {"kind", "EXPOSURE_KIND_"},
                {"direction", "RIFT_PORTAL_DIRECTION_"},
                {"kind", "RIFT_PORTAL_KIND_"},
                {"reason", "SEARCH_ABORT_REASON_"},
                {"active_skill", "YIDAO_SKILL_ID_"},
                {"skill", "SKILL_ID_"}, {"skill", "SKILL_ID_"},
                {"skill", "SKILL_ID_"}, {"skill", "SKILL_ID_"},
                {"bucket", "ALCHEMY_OUTCOME_BUCKET_"},
                {"target_type", "GATHERING_TARGET_TYPE_"},
                {"quality_hint", "GATHERING_QUALITY_HINT_"},
                {"kind", "LINGTIAN_SESSION_KIND_"},
                {"phase", "CARRIER_CHARGE_PHASE_"},
                {"main", "COLOR_KIND_"}, {"secondary", "COLOR_KIND_"},
                {"event", "EVENT_KIND_"},
                {"portal_kind", "RIFT_PORTAL_KIND_"},
                {"reason", "EXTRACT_ABORTED_REASON_"},
                {"reason", "EXTRACT_FAILED_REASON_"},
                {"bucket", "FORGE_OUTCOME_BUCKET_"},
                {"color", "COLOR_KIND_"},
                {"fog_shape", "FOG_SHAPE_"},
                {"current_step", "FORGE_STEP_"},
                {"movement_action", "MOVEMENT_ACTION_"},
                {"zone_kind", "MOVEMENT_ZONE_KIND_"},
                {"rejected_action", "MOVEMENT_ACTION_REQUEST_KIND_"},
                {"phase", "CAST_PHASE_"}, {"outcome", "CAST_OUTCOME_"},
                {"kind", "CONTAINER_KIND_"}, {"locked", "KEY_KIND_"},
                {"channel", "EVENT_CHANNEL_"}, {"priority", "EVENT_PRIORITY_"},
                {"stage", "DEATH_SCREEN_STAGE_"},
                {"zone_kind", "DEATH_SCREEN_ZONE_KIND_"},
                {"phase", "DEATH_CINEMATIC_PHASE_"},
                {"zone_kind", "DEATH_CINEMATIC_ZONE_KIND_"},
                {"result", "DEATH_ROLL_RESULT_"},
                {"qi_color_main", "COLOR_KIND_"},
                {"qi_color_secondary", "COLOR_KIND_"},
                {"color", "COLOR_KIND_"}, {"category", "CRAFT_CATEGORY_"},
                {"qi_color_min[0]", "COLOR_KIND_"},
                {"season", "SEASON_"},
                {"model_overlay", "BOTANY_MODEL_OVERLAY_"},
                {"kind", "FALSE_SKIN_KIND_"}, {"tier", "FALSE_SKIN_TIER_"},
                {"tone", "SPIRIT_TREASURE_DIALOGUE_TONE_"},
                {"trigger", "INSIGHT_TRIGGER_"},
                {"reason", "CRAFT_FAILURE_REASON_"}
            }
        );
        addNormalizations(normalizations, NormalizationMode.CAPITALIZED,
            new String[][] {
                {"forge_color", "COLOR_KIND_"},
                {"realm", "REALM_"}, {"realm", "REALM_"},
                {"realm_min", "REALM_"},
                {"color", "COLOR_KIND_"}
            }
        );
        addNormalizations(normalizations, NormalizationMode.PASCAL_CASE,
            new String[][] {{"kind", "SENSE_KIND_"}}
        );
        addNormalizations(normalizations, NormalizationMode.SNAKE_LOWER_OMIT_UNSPECIFIED,
            new String[][] {
                {"guardian_kind", "GUARDIAN_KIND_"},
                {"guardian_kind", "GUARDIAN_KIND_"}
            }
        );
        return Map.copyOf(normalizations);
    }

    private static void addNormalizations(
        Map<NormalizationSite, Integer> normalizations,
        NormalizationMode mode,
        String[][] pairs
    ) {
        for (String[] pair : pairs) {
            normalizations.merge(
                new NormalizationSite(pair[0], pair[1], mode),
                1,
                Integer::sum
            );
        }
    }

    private static Map<String, Set<NormalizationSite>> bridgeDispatchNormalizations() {
        Map<String, Set<NormalizationSite>> normalizations = new LinkedHashMap<>();
        normalizations.put("getSocialExposure", Set.of(site("kind", "EXPOSURE_KIND_")));
        normalizations.put("getRiftPortalState", Set.of(
            site("direction", "RIFT_PORTAL_DIRECTION_"),
            site("kind", "RIFT_PORTAL_KIND_")
        ));
        normalizations.put("getSearchAborted", Set.of(site("reason", "SEARCH_ABORT_REASON_")));
        normalizations.put("getYidaoHudState", Set.of(site("active_skill", "YIDAO_SKILL_ID_")));
        normalizations.put("getSkillXpGain", Set.of(site("skill", "SKILL_ID_")));
        normalizations.put("getSkillLvUp", Set.of(site("skill", "SKILL_ID_")));
        normalizations.put("getSkillCapChanged", Set.of(site("skill", "SKILL_ID_")));
        normalizations.put("getSkillScrollUsed", Set.of(site("skill", "SKILL_ID_")));
        normalizations.put("getAlchemyOutcomeResolved", Set.of(site("bucket", "ALCHEMY_OUTCOME_BUCKET_")));
        normalizations.put("getGatheringSession", Set.of(
            site("target_type", "GATHERING_TARGET_TYPE_"),
            site("quality_hint", "GATHERING_QUALITY_HINT_")
        ));
        normalizations.put("getLingtianSession", Set.of(site("kind", "LINGTIAN_SESSION_KIND_")));
        normalizations.put("getCarrierState", Set.of(site("phase", "CARRIER_CHARGE_PHASE_")));
        normalizations.put("getQiColorObserved", Set.of(
            site("main", "COLOR_KIND_"),
            site("secondary", "COLOR_KIND_")
        ));
        normalizations.put("getEventAlert", Set.of(site("event", "EVENT_KIND_")));
        normalizations.put("getExtractStarted", Set.of(site("portal_kind", "RIFT_PORTAL_KIND_")));
        normalizations.put("getExtractAborted", Set.of(site("reason", "EXTRACT_ABORTED_REASON_")));
        normalizations.put("getExtractFailed", Set.of(site("reason", "EXTRACT_FAILED_REASON_")));
        normalizations.put("getForgeOutcome", Set.of(
            site("bucket", "FORGE_OUTCOME_BUCKET_"),
            site("color", "COLOR_KIND_")
        ));
        normalizations.put("getRealmVisionParams", Set.of(site("fog_shape", "FOG_SHAPE_")));
        normalizations.put("getNicheGuardianFatigue", Set.of(
            site("guardian_kind", "GUARDIAN_KIND_", NormalizationMode.SNAKE_LOWER_OMIT_UNSPECIFIED)
        ));
        normalizations.put("getNicheGuardianBroken", Set.of(
            site("guardian_kind", "GUARDIAN_KIND_", NormalizationMode.SNAKE_LOWER_OMIT_UNSPECIFIED)
        ));
        return Map.copyOf(normalizations);
    }

    private static NormalizationSite site(String field, String prefix) {
        return site(field, prefix, NormalizationMode.SNAKE_LOWER);
    }

    private static NormalizationSite site(String field, String prefix, NormalizationMode mode) {
        return new NormalizationSite(field, prefix, mode);
    }
    private static Map<String, MigrationDecision> migrationDecisions() {
        Map<String, MigrationDecision> decisions = new LinkedHashMap<>();
        for (String channel : List.of(
            "bong:npc_metadata",
            "bong:npc_lod",
            "bong:npc_bubble",
            "bong:npc_mood",
            "bong:tsy_boss_health",
            "bong:tsy_death_vfx",
            "bong:locust_swarm_warning",
            "bong:vfx_event",
            "bong:vfx/qi_attrition",
            "bong:audio/play",
            "bong:audio/stop",
            "bong:tiandao_presence",
            "bong:audio/ambient_zone",
            "bong:zone_environment",
            "bong:mutation_visual",
            "bong:crack_reading",
            "bong:resonance_lock",
            "bong:resonance_lock_end",
            "bong:void_erosion_visual",
            "bong:spider_disguise_enter",
            "bong:spider_ambush_trigger",
            "bong:rat_qi_tier",
            "bong:daozhan_disguise_enter",
            "bong:daozhan_reveal",
            "bong:core_absorption_hallucination",
            "bong:elder_encounter",
            "bong:era_ambiance",
            "bong:halfstep_rechallenge"
        )) {
            decisions.put(channel, MigrationDecision.MIGRATE);
        }
        decisions.put("bong:agent_ui_request", MigrationDecision.EXEMPT);
        decisions.put("bong:agent_ui_close", MigrationDecision.EXEMPT);
        decisions.put("bong:shader_state", MigrationDecision.EXEMPT);
        return Map.copyOf(decisions);
    }

    private static long decisionCount(MigrationDecision decision) {
        return SIDE_CHANNEL_DECISIONS.values().stream().filter(decision::equals).count();
    }

    private static Set<String> channelsWithDecision(MigrationDecision decision) {
        return SIDE_CHANNEL_DECISIONS.entrySet().stream()
            .filter(entry -> entry.getValue() == decision)
            .map(Map.Entry::getKey)
            .collect(java.util.stream.Collectors.toUnmodifiableSet());
    }

    private record ReceiverSite(
        Path relativePath,
        long line,
        String channel,
        ExecutableElement enclosingMethod
    ) {
        @Override
        public String toString() {
            return relativePath + ":" + line;
        }
    }

    private static final class JavaSourceModel {
        private final Path clientRoot;
        private final List<Path> sourceRoots;
        private final Trees trees;
        private final List<ReceiverSite> receiverSites = new ArrayList<>();
        private final List<String> forbiddenReferences = new ArrayList<>();
        private final Map<ExecutableElement, List<ExecutableElement>> calls = new HashMap<>();
        private final Map<ExecutableElement, List<ExecutableElement>> directCalls = new HashMap<>();
        private final Map<String, ExecutableElement> sourceMethods = new HashMap<>();
        private final Set<ExecutableElement> sourceMethodElements = new HashSet<>();
        private final Map<ExecutableElement, List<NormalizationSite>> bridgeNormalizationsByMethod =
            new HashMap<>();
        private final Map<String, List<NormalizationSite>> bridgeNormalizationsByGetter =
            new HashMap<>();
        private final Map<String, List<ExecutableElement>> bridgeNormalizationOwnersByGetter =
            new HashMap<>();
        private final Map<String, List<PrefixLiteralSite>> prefixLiteralsByValue = new HashMap<>();
        private final List<NormalizationSite> inventoryNormalizations = new ArrayList<>();
        private boolean inventoryStartsWithPrefix;
        private boolean inventoryLowercasesSlot;
        private boolean inventoryStateStartsWithPrefix;
        private boolean inventoryLowercasesState;
        private ExecutableElement bridgeMethod;
        private ExecutableElement inventoryParseLocationMethod;

        private JavaSourceModel(Path clientRoot, List<Path> sourceRoots, Trees trees) {
            this.clientRoot = clientRoot;
            this.sourceRoots = sourceRoots;
            this.trees = trees;
        }

        static List<String> directCallsInSyntheticSource(String source) throws IOException {
            JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
            JavaFileObject sourceFile = new SimpleJavaFileObject(
                URI.create("string:///Wiring.java"),
                JavaFileObject.Kind.SOURCE
            ) {
                @Override
                public CharSequence getCharContent(boolean ignoreEncodingErrors) {
                    return source;
                }
            };
            JavacTask task = (JavacTask) compiler.getTask(
                null,
                null,
                null,
                List.of("-proc:none", "--release", "17"),
                null,
                List.of(sourceFile)
            );
            List<CompilationUnitTree> units = new ArrayList<>();
            task.parse().forEach(units::add);
            List<String> callers = new ArrayList<>();
            for (CompilationUnitTree unit : units) {
                new TreePathScanner<Void, String>() {
                    @Override
                    public Void visitMethod(MethodTree node, String ignored) {
                        return super.visitMethod(node, node.getName().toString());
                    }

                    @Override
                    public Void visitMethodInvocation(MethodInvocationTree node, String caller) {
                        if (node.getMethodSelect().toString().equals("register")
                            && isDirectExpressionStatement(getCurrentPath())) {
                            callers.add(caller);
                        }
                        return super.visitMethodInvocation(node, caller);
                    }
                }.scan(unit, null);
            }
            callers.sort(String::compareTo);
            return List.copyOf(callers);
        }

        static JavaSourceModel parse(Path clientRoot) throws IOException {
            JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
            assertTrue(compiler != null, "R6 source pin requires a full Java 17 JDK, not a JRE");
            List<Path> sourceRoots = SOURCE_ROOTS.stream()
                .map(clientRoot::resolve)
                .filter(Files::isDirectory)
                .sorted()
                .toList();
            assertEquals(SOURCE_ROOTS.size(), sourceRoots.size(),
                "R6 receiver census requires every production Java source root to exist");
            List<Path> sourceFiles = new ArrayList<>();
            for (Path sourceRoot : sourceRoots) {
                try (Stream<Path> files = Files.walk(sourceRoot)) {
                    sourceFiles.addAll(files
                        .filter(path -> path.toString().endsWith(".java"))
                        .toList());
                }
            }
            sourceFiles.sort(Comparator.naturalOrder());
            assertGeneratedSourceContract(sourceRoots, sourceFiles);
            List<Path> attributedSources = ATTRIBUTED_SOURCES.stream()
                .map(clientRoot::resolve)
                .sorted()
                .toList();
            assertTrue(attributedSources.stream().allMatch(Files::isRegularFile),
                "R6 attributed ownership source set must remain present");

            DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
            try (StandardJavaFileManager fileManager =
                     compiler.getStandardFileManager(diagnostics, null, null)) {
                Iterable<? extends JavaFileObject> javaFiles =
                    fileManager.getJavaFileObjectsFromPaths(attributedSources);
                List<String> options = List.of(
                    "-proc:none",
                    "--release", "17",
                    "-classpath", System.getProperty("java.class.path")
                );
                JavacTask task = (JavacTask) compiler.getTask(
                    null, fileManager, diagnostics, options, null, javaFiles
                );
                List<CompilationUnitTree> units = new ArrayList<>();
                task.parse().forEach(units::add);
                task.analyze();

                List<String> errors = diagnostics.getDiagnostics().stream()
                    .filter(diagnostic -> diagnostic.getKind() == Diagnostic.Kind.ERROR)
                    .filter(diagnostic -> !isKnownMissingNullableDiagnostic(diagnostic))
                    .map(JavaSourceModel::formatDiagnostic)
                    .toList();
                assertEquals(List.of(), errors,
                    "production Java AST attribution must succeed before receiver census can be trusted");

                JavaSourceModel model = new JavaSourceModel(clientRoot, sourceRoots, Trees.instance(task));
                for (CompilationUnitTree unit : units) {
                    model.scan(unit);
                }

                for (Path sourceFile : sourceFiles) {
                    model.scanPrefixLiterals(sourceFile);
                }
                assertEquals(List.of(), model.forbiddenReferences,
                    "registerGlobalReceiver method references are dynamic registration forms; R6 fails closed");
                return model;
            }
        }

        List<ReceiverSite> receiverSites() {
            return List.copyOf(receiverSites);
        }

        Map<String, Set<NormalizationSite>> bridgeDispatchNormalizations() {
            Map<String, Set<NormalizationSite>> actual = new HashMap<>();
            Set<ExecutableElement> reachable = reachableFrom(Set.of(bridgeMethod));
            for (Map.Entry<String, List<ExecutableElement>> entry : bridgeNormalizationOwnersByGetter.entrySet()) {
                if (entry.getValue().stream().noneMatch(reachable::contains)) {
                    continue;
                }
                List<NormalizationSite> sites = bridgeNormalizationsByGetter.getOrDefault(entry.getKey(), List.of());
                actual.put(entry.getKey(), Set.copyOf(sites));
            }
            return Map.copyOf(actual);
        }

        Map<NormalizationSite, Integer> bridgeNormalizations() {
            assertTrue(bridgeMethod != null, "ProtoServerDataBridge.bridge production root disappeared");
            Set<ExecutableElement> reachable = reachableFrom(Set.of(bridgeMethod));
            Map<NormalizationSite, Integer> normalizations = new HashMap<>();
            for (ExecutableElement method : reachable) {
                for (NormalizationSite site : bridgeNormalizationsByMethod
                    .getOrDefault(method, List.of())) {
                    normalizations.merge(site, 1, Integer::sum);
                }
            }
            return Map.copyOf(normalizations);
        }

        Map<String, Integer> bridgePrefixLiteralCounts() {
            Path bridgePath = Path.of("com/bong/client/network/ProtoServerDataBridge.java");
            Map<String, Integer> counts = new HashMap<>();
            for (Map.Entry<String, List<PrefixLiteralSite>> entry : prefixLiteralsByValue.entrySet()) {
                int count = (int) entry.getValue().stream()
                    .filter(site -> site.relativePath().equals(bridgePath))
                    .count();
                if (count > 0) {
                    counts.put(entry.getKey(), count);
                }
            }
            return Map.copyOf(counts);
        }

        int productionPrefixLiteralCount() {
            return prefixLiteralsByValue.size();
        }

        void assertProductionPrefixLiteralInventory() {
            Set<String> expected = BRIDGE_NORMALIZATIONS.keySet().stream()
                .map(NormalizationSite::prefix)
                .collect(java.util.stream.Collectors.toCollection(TreeSet::new));
            expected.add("EQUIP_SLOT_");
            expected.add("EQUIP_STATE_");
            assertEquals(expected, new TreeSet<>(prefixLiteralsByValue.keySet()),
                "production enum-prefix literals must match the frozen R6 ledger");

            Path bridgePath = Path.of("com/bong/client/network/ProtoServerDataBridge.java");
            Path inventoryPath = Path.of("com/bong/client/network/InventoryEventHandler.java");
            List<String> escaped = prefixLiteralsByValue.entrySet().stream()
                .flatMap(entry -> entry.getValue().stream().map(site -> Map.entry(entry.getKey(), site)))
                .filter(entry -> {
                    Path path = entry.getValue().relativePath();
                    return !path.equals(bridgePath) && !path.equals(inventoryPath);
                })
                .map(entry -> entry.getKey() + " -> " + entry.getValue())
                .sorted()
                .toList();
            assertEquals(List.of(), escaped,
                "enum-prefix literals may only occur in ProtoServerDataBridge or the pinned inventory exception");

            for (String prefix : List.of("EQUIP_SLOT_", "EQUIP_STATE_")) {
                List<PrefixLiteralSite> inventorySites =
                    prefixLiteralsByValue.getOrDefault(prefix, List.of());
                assertEquals(1, inventorySites.size(),
                    "InventoryEventHandler must retain exactly one " + prefix + " literal declaration");
                assertEquals(inventoryPath, inventorySites.get(0).relativePath(),
                    prefix + " may only occur in InventoryEventHandler");
            }
        }

        List<NormalizationSite> inventoryNormalizations() {
            assertTrue(inventoryParseLocationMethod != null,
                "InventoryEventHandler.parseLocation production root disappeared");
            assertTrue(inventoryStartsWithPrefix,
                "InventoryEventHandler.parseLocation 必须先以 EQUIP_SLOT_ 做 startsWith gate");
            assertTrue(inventoryLowercasesSlot,
                "InventoryEventHandler.parseLocation 必须把 EQUIP_SLOT_* 后缀转为 ROOT lowercase");
            assertTrue(inventoryStateStartsWithPrefix,
                "InventoryEventHandler.parseLocation 必须先以 EQUIP_STATE_ 做 startsWith gate");
            assertTrue(inventoryLowercasesState,
                "InventoryEventHandler.parseLocation 必须把 EQUIP_STATE_* 后缀转为 ROOT lowercase");
            return List.copyOf(inventoryNormalizations);
        }

        void assertReceiverBootstrapsAreReachable() throws IOException {
            assertClientEntrypoint();
            ExecutableElement initializer = requireSourceMethod(
                CLIENT_ENTRYPOINT, "onInitializeClient", 0
            );
            ExecutableElement networkRegister = requireSourceMethod(
                "com.bong.client.BongNetworkHandler", "register", 0
            );
            ExecutableElement irisRegister = requireSourceMethod(
                "com.bong.client.iris.IrisBootstrap", "register", 0
            );

            assertEquals(1, directCallCount(initializer, networkRegister),
                "BongClient.onInitializeClient 必须以无条件 expression statement 恰好调用一次 BongNetworkHandler.register");
            assertEquals(1, directCallCount(initializer, irisRegister),
                "BongClient.onInitializeClient 必须以无条件 expression statement 恰好调用一次 IrisBootstrap.register");

            Set<ExecutableElement> reachable = reachableFromDirectCalls(
                Set.of(networkRegister, irisRegister)
            );
            List<ReceiverSite> unreachable = receiverSites.stream()
                .filter(site -> !reachable.contains(site.enclosingMethod()))
                .toList();
            assertEquals(List.of(), unreachable,
                "每个 receiver 调用都必须经无条件 direct-call 链从两个 production register bootstrap 可达");
        }

        private void scan(CompilationUnitTree unit) {
            new TreePathScanner<Void, ExecutableElement>() {
                @Override
                public Void visitMethod(MethodTree node, ExecutableElement ignored) {
                    Element element = trees.getElement(getCurrentPath());
                    ExecutableElement method = element instanceof ExecutableElement executable
                        ? executable : null;
                    if (method != null) {
                        sourceMethods.put(methodKey(method), method);
                        sourceMethodElements.add(method);
                        String key = methodKey(method);
                        if (key.equals(
                            "com.bong.client.network.ProtoServerDataBridge#bridge/1"
                        )) {
                            bridgeMethod = method;
                        }
                        if (key.equals(
                            "com.bong.client.network.InventoryEventHandler#parseLocation/1"
                        )) {
                            inventoryParseLocationMethod = method;
                        }
                    }
                    return super.visitMethod(node, method);
                }

                @Override
                public Void visitMethodInvocation(
                    MethodInvocationTree node,
                    ExecutableElement enclosingMethod
                ) {
                    Element element = trees.getElement(
                        new TreePath(getCurrentPath(), node.getMethodSelect())
                    );
                    boolean receiverSyntax = isReceiverSyntax(node.getMethodSelect());
                    assertTrue(!receiverSyntax || element instanceof ExecutableElement,
                        () -> "registerGlobalReceiver syntax could not be attributed at " + site(unit, node));
                    assertTrue(!receiverSyntax || isReceiverMethod((ExecutableElement) element),
                        () -> "registerGlobalReceiver must resolve to Fabric's exact owner at "
                            + site(unit, node));
                    if (enclosingMethod != null && element instanceof ExecutableElement called) {
                        calls.computeIfAbsent(enclosingMethod, ignored -> new ArrayList<>()).add(called);
                        if (isDirectExpressionStatement(getCurrentPath())) {
                            directCalls.computeIfAbsent(enclosingMethod, ignored -> new ArrayList<>())
                                .add(called);
                        }
                    }
                    if (element instanceof ExecutableElement called && isReceiverMethod(called)) {
                        assertTrue(enclosingMethod != null,
                            "receiver invocation must be enclosed by a production method");
                        assertTrue(isDirectExpressionStatement(getCurrentPath()),
                            () -> "receiver registration must be an unconditional expression statement at "
                                + site(unit, node));
                        assertTrue(!node.getArguments().isEmpty(),
                            "registerGlobalReceiver must have an Identifier first argument");
                        TreePath argumentPath = new TreePath(getCurrentPath(), node.getArguments().get(0));
                        String channel = resolveIdentifier(argumentPath, new HashSet<>());
                        assertTrue(channel != null,
                            () -> "receiver channel must be statically resolvable at " + site(unit, node));
                        receiverSites.add(new ReceiverSite(
                            relativePath(unit),
                            line(unit, node),
                            channel,
                            enclosingMethod
                        ));
                    }
                    collectBridgeNormalization(unit, node, enclosingMethod, element);
                    collectInventoryNormalization(unit, node, enclosingMethod, element);
                    return super.visitMethodInvocation(node, enclosingMethod);
                }

                @Override
                public Void visitMemberReference(
                    MemberReferenceTree node,
                    ExecutableElement enclosingMethod
                ) {
                    if (node.getName().contentEquals("registerGlobalReceiver")) {
                        Element element = trees.getElement(getCurrentPath());
                        String owner = element instanceof ExecutableElement called
                            ? called.getEnclosingElement().toString()
                            : "<unattributed>";
                        forbiddenReferences.add(site(unit, node) + " owner=" + owner);
                    }
                    return super.visitMemberReference(node, enclosingMethod);
                }
            }.scan(unit, null);
        }

        private void scanPrefixLiterals(Path sourceFile) throws IOException {
            String source = Files.readString(sourceFile);
            int index = 0;
            long line = 1;
            while (index < source.length()) {
                char current = source.charAt(index);
                if (current == '\n') {
                    line++;
                    index++;
                    continue;
                }
                if (current == '/' && index + 1 < source.length()) {
                    char next = source.charAt(index + 1);
                    if (next == '/') {
                        index += 2;
                        while (index < source.length() && source.charAt(index) != '\n') {
                            index++;
                        }
                        continue;
                    }
                    if (next == '*') {
                        index += 2;
                        while (index + 1 < source.length()
                            && !(source.charAt(index) == '*' && source.charAt(index + 1) == '/')) {
                            if (source.charAt(index) == '\n') {
                                line++;
                            }
                            index++;
                        }
                        index = Math.min(source.length(), index + 2);
                        continue;
                    }
                }
                if (current == '\'') {
                    index = skipJavaCharacterLiteral(source, index);
                    continue;
                }
                if (current != '"') {
                    index++;
                    continue;
                }
                if (source.startsWith("\"\"\"", index)) {
                    index += 3;
                    while (index + 2 < source.length() && !source.startsWith("\"\"\"", index)) {
                        if (source.charAt(index) == '\n') {
                            line++;
                        }
                        index++;
                    }
                    index = Math.min(source.length(), index + 3);
                    continue;
                }

                long literalLine = line;
                StringBuilder value = new StringBuilder();
                index++;
                while (index < source.length()) {
                    char character = source.charAt(index++);
                    if (character == '"') {
                        break;
                    }
                    if (character == '\\' && index < source.length()) {
                        char escaped = source.charAt(index++);
                        value.append(switch (escaped) {
                            case 'b' -> '\b';
                            case 't' -> '\t';
                            case 'n' -> '\n';
                            case 'f' -> '\f';
                            case 'r' -> '\r';
                            case '"' -> '"';
                            case '\'' -> '\'';
                            case '\\' -> '\\';
                            default -> escaped;
                        });
                        continue;
                    }
                    if (character == '\n') {
                        line++;
                    }
                    value.append(character);
                }
                String literal = value.toString();
                PrefixLiteralSite site = new PrefixLiteralSite(relativePath(sourceFile), literalLine);
                if (ENUM_PREFIXES.contains(literal)) {
                    prefixLiteralsByValue
                        .computeIfAbsent(literal, unused -> new ArrayList<>())
                        .add(site);
                } else if (literal.matches("[A-Z][A-Z0-9_]*_")) {
                    fail("unregistered enum-prefix literal at " + site + ": " + literal);
                }
            }
        }

        private int skipJavaCharacterLiteral(String source, int index) {
            index++;
            while (index < source.length()) {
                char character = source.charAt(index++);
                if (character == '\\' && index < source.length()) {
                    index++;
                } else if (character == '\'') {
                    break;
                }
            }
            return index;
        }


        private String getterName(ExpressionTree expression) {
            if (expression instanceof MethodInvocationTree invocation
                && invocation.getMethodSelect() instanceof MemberSelectTree select) {
                return select.getIdentifier().toString();
            }
            return null;
        }

        private void collectBridgeNormalization(
            CompilationUnitTree unit,
            MethodInvocationTree node,
            ExecutableElement enclosingMethod,
            Element calledElement
        ) {
            if (enclosingMethod == null
                || !(calledElement instanceof ExecutableElement called)) {
                return;
            }
            TreePath invocationPath = getPath(unit, node);
            if (methodKey(enclosingMethod).equals(
                "com.bong.client.network.ProtoServerDataBridge#bridgeCraftRecipeList/2"
            )) {
                collectCraftRecipeArrayNormalization(
                    unit, node, enclosingMethod, called, invocationPath
                );
            }
            if (!called.getEnclosingElement().toString()
                .equals("com.bong.client.network.ProtoServerDataBridge")) {
                return;
            }
            String helper = called.getSimpleName().toString();
            if ((helper.equals("bridgeStripEnums")
                || helper.equals("bridgeStripEnumsOmittingUnspecified"))
                && !isNormalizationHelperDeclaration(enclosingMethod)) {
                NormalizationMode mode = helper.equals("bridgeStripEnums")
                    ? NormalizationMode.SNAKE_LOWER
                    : NormalizationMode.SNAKE_LOWER_OMIT_UNSPECIFIED;
                List<NormalizationSite> sites = new ArrayList<>();
                for (int index = 2; index < node.getArguments().size(); index++) {
                    String[] pair = resolveStringPair(
                        new TreePath(invocationPath, node.getArguments().get(index))
                    );
                    assertTrue(pair != null,
                        () -> "enum field/prefix pair must be static at " + site(unit, node));
                    NormalizationSite normalization = new NormalizationSite(pair[0], pair[1], mode);
                    sites.add(normalization);
                    addBridgeNormalization(enclosingMethod, pair[0], pair[1], mode);
                }
                String getter = getterName(node.getArguments().get(0));
                assertTrue(getter != null,
                    () -> "generic bridge converter payload getter must be statically identified at "
                        + site(unit, node));
                bridgeNormalizationsByGetter
                    .computeIfAbsent(getter, ignored -> new ArrayList<>())
                    .addAll(sites);
                bridgeNormalizationOwnersByGetter
                    .computeIfAbsent(getter, ignored -> new ArrayList<>())
                    .add(enclosingMethod);
                return;
            }
            NormalizationMode mode = switch (helper) {
                case "stripEnumPrefix", "stripEnumPrefixInArray" ->
                    NormalizationMode.SNAKE_LOWER;
                case "stripEnumPrefixCapitalized", "normalizeRealmField" ->
                    NormalizationMode.CAPITALIZED;
                case "stripEnumPrefixPascalCase" -> NormalizationMode.PASCAL_CASE;
                default -> null;
            };
            if (mode == null || isNormalizationHelperDeclaration(enclosingMethod)) {
                return;
            }
            int fieldIndex;
            int prefixIndex;
            if (helper.equals("stripEnumPrefixInArray")) {
                fieldIndex = 2;
                prefixIndex = 3;
            } else if (helper.equals("normalizeRealmField")) {
                fieldIndex = 1;
                String field = resolveString(
                    new TreePath(invocationPath, node.getArguments().get(fieldIndex)),
                    new HashSet<>()
                );
                assertTrue(field != null,
                    () -> "realm normalization field must be static at " + site(unit, node));
                addBridgeNormalization(enclosingMethod, field, "REALM_", mode);
                return;
            } else {
                fieldIndex = 1;
                prefixIndex = 2;
            }
            String field = resolveString(
                new TreePath(invocationPath, node.getArguments().get(fieldIndex)),
                new HashSet<>()
            );
            String prefix = resolveString(
                new TreePath(invocationPath, node.getArguments().get(prefixIndex)),
                new HashSet<>()
            );
            assertTrue(field != null && prefix != null,
                () -> "enum field/prefix must be static at " + site(unit, node));
            addBridgeNormalization(enclosingMethod, field, prefix, mode);
        }

        private void collectCraftRecipeArrayNormalization(
            CompilationUnitTree unit,
            MethodInvocationTree node,
            ExecutableElement enclosingMethod,
            ExecutableElement called,
            TreePath invocationPath
        ) {
            String helper = called.getSimpleName().toString();
            if (!helper.equals("set") || node.getArguments().size() != 2
                || !node.getArguments().get(0).toString().equals("0")
                || !(node.getMethodSelect() instanceof MemberSelectTree select)
                || !isQiColorMinArray(new TreePath(invocationPath, select.getExpression()))) {
                return;
            }
            ExpressionTree value = node.getArguments().get(1);
            assertTrue(value instanceof NewClassTree
                    || value.toString().contains("COLOR_KIND_"),
                () -> "craft recipe qi_color_min[0] must normalize its assigned value at " + site(unit, node));
            assertTrue(value.toString().contains("substring(\"COLOR_KIND_\".length())"),
                () -> "craft recipe qi_color_min[0] must strip COLOR_KIND_ at " + site(unit, node));
            assertTrue(value.toString().contains("toLowerCase(Locale.ROOT)"),
                () -> "craft recipe qi_color_min[0] must use ROOT lowercase at " + site(unit, node));
            addBridgeNormalization(
                enclosingMethod,
                "qi_color_min[0]",
                "COLOR_KIND_",
                NormalizationMode.SNAKE_LOWER
            );
        }

        private boolean isQiColorMinArray(TreePath receiverPath) {
            Element element = trees.getElement(receiverPath);
            if (!(element instanceof VariableElement variable)
                || !variable.getSimpleName().contentEquals("qcArr")) {
                return false;
            }
            TreePath declarationPath = trees.getPath(variable);
            if (declarationPath == null || !(declarationPath.getLeaf() instanceof VariableTree declaration)
                || !(declaration.getInitializer() instanceof MethodInvocationTree initializer)
                || initializer.getArguments().size() != 1) {
                return false;
            }
            return "getAsJsonArray".contentEquals(initializer.getMethodSelect() instanceof MemberSelectTree select
                    ? select.getIdentifier() : "")
                && "\"qi_color_min\"".equals(initializer.getArguments().get(0).toString());
        }

        private void collectInventoryNormalization(
            CompilationUnitTree unit,
            MethodInvocationTree node,
            ExecutableElement enclosingMethod,
            Element calledElement
        ) {
            if (enclosingMethod == null
                || !methodKey(enclosingMethod).equals(
                    "com.bong.client.network.InventoryEventHandler#parseLocation/1"
                )) {
                return;
            }
            String helper = calledElement instanceof ExecutableElement called
                ? called.getSimpleName().toString()
                : "";
            TreePath invocationPath = getPath(unit, node);
            String variableName = receiverVariableName(invocationPath);
            String field;
            String expectedPrefix;
            if ("slotName".equals(variableName)) {
                field = "slot";
                expectedPrefix = "EQUIP_SLOT_";
            } else if ("stateName".equals(variableName)) {
                field = "state";
                expectedPrefix = "EQUIP_STATE_";
            } else {
                return;
            }
            if (helper.equals("startsWith") && node.getArguments().size() == 1) {
                String prefix = resolveString(
                    new TreePath(invocationPath, node.getArguments().get(0)),
                    new HashSet<>()
                );
                if (expectedPrefix.equals(prefix)) {
                    if (field.equals("slot")) {
                        inventoryStartsWithPrefix = true;
                    } else {
                        inventoryStateStartsWithPrefix = true;
                    }
                }
            }
            if (helper.equals("substring") && node.getArguments().size() == 1) {
                TreePath lengthPath = new TreePath(invocationPath, node.getArguments().get(0));
                if (lengthPath.getLeaf() instanceof MethodInvocationTree lengthCall
                    && lengthCall.getArguments().isEmpty()
                    && lengthCall.getMethodSelect() instanceof MemberSelectTree lengthSelect
                    && lengthSelect.getIdentifier().contentEquals("length")) {
                    String prefix = resolveString(
                        new TreePath(lengthPath, lengthSelect.getExpression()),
                        new HashSet<>()
                    );
                    if (expectedPrefix.equals(prefix)
                        && isInsideEquipPrefixGuard(invocationPath, variableName, expectedPrefix)) {
                        inventoryNormalizations.add(new NormalizationSite(
                            field, prefix, NormalizationMode.SNAKE_LOWER
                        ));
                    }
                }
            }
            if (helper.equals("toLowerCase") && node.getArguments().size() == 1
                && node.getArguments().get(0).toString().equals("java.util.Locale.ROOT")
                && isInsideEquipPrefixGuard(invocationPath, variableName, expectedPrefix)) {
                if (field.equals("slot")) {
                    inventoryLowercasesSlot = true;
                } else {
                    inventoryLowercasesState = true;
                }
            }
        }

        private String receiverVariableName(TreePath invocationPath) {
            Tree leaf = invocationPath.getLeaf();
            if (!(leaf instanceof MethodInvocationTree invocation)
                || !(invocation.getMethodSelect() instanceof MemberSelectTree select)) {
                return null;
            }
            return rootedVariableName(new TreePath(invocationPath, select.getExpression()));
        }

        private String rootedVariableName(TreePath expressionPath) {
            Element receiver = trees.getElement(expressionPath);
            if (receiver instanceof VariableElement variable) {
                String name = variable.getSimpleName().toString();
                if (name.equals("slotName") || name.equals("stateName")) {
                    return name;
                }
            }
            if (expressionPath.getLeaf() instanceof MethodInvocationTree invocation
                && invocation.getMethodSelect() instanceof MemberSelectTree select) {
                return rootedVariableName(new TreePath(expressionPath, select.getExpression()));
            }
            return null;
        }

        private boolean isInsideEquipPrefixGuard(
            TreePath invocationPath,
            String variableName,
            String expectedPrefix
        ) {
            for (TreePath current = invocationPath.getParentPath(); current != null; current = current.getParentPath()) {
                if (!(current.getLeaf() instanceof IfTree conditional)) {
                    continue;
                }
                String condition = conditional.getCondition().toString();
                if (condition.contains(variableName + ".startsWith")
                    && condition.contains(expectedPrefix)) {
                    return true;
                }
            }
            return false;
        }


        private void addBridgeNormalization(
            ExecutableElement method,
            String field,
            String prefix,
            NormalizationMode mode
        ) {
            bridgeNormalizationsByMethod
                .computeIfAbsent(method, ignored -> new ArrayList<>())
                .add(new NormalizationSite(field, prefix, mode));
        }

        private boolean isNormalizationHelperDeclaration(ExecutableElement method) {
            String name = method.getSimpleName().toString();
            return name.equals("stripEnumPrefixInArray")
                || name.equals("stripEnumPrefixCapitalized")
                || name.equals("stripEnumPrefixPascalCase")
                || name.equals("normalizeRealmField")
                || name.equals("bridgeStripEnums")
                || name.equals("bridgeStripEnumsOmittingUnspecified");
        }

        private String[] resolveStringPair(TreePath path) {
            if (!(path.getLeaf() instanceof NewArrayTree array)
                || array.getInitializers() == null
                || array.getInitializers().size() != 2) {
                return null;
            }
            String field = resolveString(
                new TreePath(path, array.getInitializers().get(0)), new HashSet<>()
            );
            String prefix = resolveString(
                new TreePath(path, array.getInitializers().get(1)), new HashSet<>()
            );
            return field == null || prefix == null ? null : new String[] {field, prefix};
        }

        private TreePath getPath(CompilationUnitTree unit, Tree tree) {
            TreePath path = trees.getPath(unit, tree);
            assertTrue(path != null, () -> "missing TreePath for " + site(unit, tree));
            return path;
        }

        private static boolean isDirectExpressionStatement(TreePath invocationPath) {
            TreePath current = invocationPath.getParentPath();
            while (current != null && current.getLeaf() instanceof ParenthesizedTree) {
                current = current.getParentPath();
            }
            if (current == null || !(current.getLeaf() instanceof ExpressionStatementTree)) {
                return false;
            }
            TreePath statementParent = current.getParentPath();
            if (statementParent == null || !(statementParent.getLeaf() instanceof BlockTree block)) {
                return false;
            }
            int statementIndex = block.getStatements().indexOf(current.getLeaf());
            if (statementIndex < 0) {
                return false;
            }
            for (Tree statement : block.getStatements().subList(0, statementIndex)) {
                if (mayCompleteAbruptly(statement)) {
                    return false;
                }
            }
            for (TreePath ancestor = statementParent.getParentPath();
                 ancestor != null;
                 ancestor = ancestor.getParentPath()) {
                Tree node = ancestor.getLeaf();
                if (node instanceof MethodTree || node instanceof CompilationUnitTree) {
                    return true;
                }
                if (node instanceof LambdaExpressionTree
                    || node instanceof ClassTree
                    || node instanceof IfTree
                    || node instanceof DoWhileLoopTree
                    || node instanceof EnhancedForLoopTree
                    || node instanceof ForLoopTree
                    || node instanceof WhileLoopTree
                    || node instanceof SwitchTree
                    || node instanceof SwitchExpressionTree
                    || node instanceof TryTree
                    || node instanceof CatchTree) {
                    return false;
                }
            }
            return false;
        }

        private static boolean mayCompleteAbruptly(Tree statement) {
            return Boolean.TRUE.equals(new TreeScanner<Boolean, Void>() {
                @Override
                public Boolean reduce(Boolean left, Boolean right) {
                    return Boolean.TRUE.equals(left) || Boolean.TRUE.equals(right);
                }

                @Override
                public Boolean visitReturn(ReturnTree node, Void ignored) {
                    return true;
                }

                @Override
                public Boolean visitThrow(ThrowTree node, Void ignored) {
                    return true;
                }

                @Override
                public Boolean visitClass(ClassTree node, Void ignored) {
                    return false;
                }

                @Override
                public Boolean visitLambdaExpression(LambdaExpressionTree node, Void ignored) {
                    return false;
                }
            }.scan(statement, null));
        }

        private String resolveIdentifier(TreePath path, Set<Element> resolving) {
            Tree leaf = path.getLeaf();
            if (leaf instanceof ParenthesizedTree parenthesized) {
                return resolveIdentifier(new TreePath(path, parenthesized.getExpression()), resolving);
            }
            if (leaf instanceof NewClassTree constructor) {
                Element type = trees.getElement(new TreePath(path, constructor.getIdentifier()));
                if (!(type instanceof TypeElement typeElement)
                    || !IDENTIFIER_OWNER.contentEquals(typeElement.getQualifiedName())
                    || constructor.getArguments().size() != 2) {
                    return null;
                }
                String namespace = resolveString(
                    new TreePath(path, constructor.getArguments().get(0)), resolving
                );
                String channelPath = resolveString(
                    new TreePath(path, constructor.getArguments().get(1)), resolving
                );
                return namespace == null || channelPath == null
                    ? null : namespace + ":" + channelPath;
            }

            Element element = trees.getElement(path);
            if (!(element instanceof VariableElement variable)
                || !variable.getModifiers().contains(Modifier.FINAL)
                || !resolving.add(variable)) {
                return null;
            }
            try {
                TreePath declarationPath = trees.getPath(variable);
                if (declarationPath == null || !(declarationPath.getLeaf() instanceof VariableTree declaration)
                    || declaration.getInitializer() == null) {
                    return null;
                }
                return resolveIdentifier(
                    new TreePath(declarationPath, declaration.getInitializer()), resolving
                );
            } finally {
                resolving.remove(variable);
            }
        }

        private String resolveString(TreePath path, Set<Element> resolving) {
            Tree leaf = path.getLeaf();
            if (leaf instanceof ParenthesizedTree parenthesized) {
                return resolveString(new TreePath(path, parenthesized.getExpression()), resolving);
            }
            if (leaf instanceof LiteralTree literal && literal.getValue() instanceof String value) {
                return value;
            }
            Element element = trees.getElement(path);
            if (element instanceof VariableElement variable
                && variable.getConstantValue() instanceof String value) {
                return value;
            }
            if (!(element instanceof VariableElement variable)
                || !variable.getModifiers().contains(Modifier.FINAL)
                || !resolving.add(variable)) {
                return null;
            }
            try {
                TreePath declarationPath = trees.getPath(variable);
                if (declarationPath == null || !(declarationPath.getLeaf() instanceof VariableTree declaration)
                    || declaration.getInitializer() == null) {
                    return null;
                }
                return resolveString(new TreePath(declarationPath, declaration.getInitializer()), resolving);
            } finally {
                resolving.remove(variable);
            }
        }

        private ExecutableElement requireSourceMethod(String owner, String name, int parameterCount) {
            String key = owner + "#" + name + "/" + parameterCount;
            ExecutableElement method = sourceMethods.get(key);
            if (method == null) {
                fail("production bootstrap method disappeared: " + key);
            }
            return method;
        }

        private int directCallCount(ExecutableElement caller, ExecutableElement callee) {
            return (int) directCalls.getOrDefault(caller, List.of()).stream()
                .filter(callee::equals)
                .count();
        }

        private Set<ExecutableElement> reachableFrom(Set<ExecutableElement> roots) {
            return reachableFrom(roots, calls);
        }

        private Set<ExecutableElement> reachableFromDirectCalls(Set<ExecutableElement> roots) {
            return reachableFrom(roots, directCalls);
        }

        private Set<ExecutableElement> reachableFrom(
            Set<ExecutableElement> roots,
            Map<ExecutableElement, List<ExecutableElement>> graph
        ) {
            Set<ExecutableElement> reachable = new HashSet<>();
            ArrayDeque<ExecutableElement> pending = new ArrayDeque<>(roots);
            while (!pending.isEmpty()) {
                ExecutableElement method = pending.removeFirst();
                if (!reachable.add(method)) {
                    continue;
                }
                for (ExecutableElement called : graph.getOrDefault(method, List.of())) {
                    if (sourceMethodElements.contains(called)) {
                        pending.addLast(called);
                    }
                }
            }
            return reachable;
        }

        private boolean isReceiverMethod(ExecutableElement method) {
            return method.getSimpleName().contentEquals("registerGlobalReceiver")
                && method.getEnclosingElement() instanceof TypeElement owner
                && RECEIVER_OWNER.contentEquals(owner.getQualifiedName());
        }

        private static boolean isReceiverSyntax(ExpressionTree methodSelect) {
            if (methodSelect instanceof MemberSelectTree memberSelect) {
                return memberSelect.getIdentifier().contentEquals("registerGlobalReceiver");
            }
            return methodSelect instanceof IdentifierTree identifier
                && identifier.getName().contentEquals("registerGlobalReceiver");
        }

        private static boolean isKnownMissingNullableDiagnostic(
            Diagnostic<? extends JavaFileObject> diagnostic
        ) {
            String message = diagnostic.getMessage(null);
            return message.contains("package org.jetbrains.annotations does not exist")
                || message.contains("cannot find symbol")
                    && message.contains("class Nullable");
        }

        private void assertClientEntrypoint() throws IOException {
            JsonObject manifest = JsonParser.parseString(Files.readString(
                clientRoot.resolve("src/main/resources/fabric.mod.json")
            )).getAsJsonObject();
            JsonElement clientEntrypoints = manifest.getAsJsonObject("entrypoints").get("client");
            assertTrue(clientEntrypoints != null && clientEntrypoints.isJsonArray(),
                "fabric.mod.json 必须声明 client entrypoint array");
            JsonArray entries = clientEntrypoints.getAsJsonArray();
            assertEquals(1, entries.size(), "R6 P0 冻结唯一 production client entrypoint");
            assertEquals(CLIENT_ENTRYPOINT, entries.get(0).getAsString(),
                "真实 Fabric client entrypoint 必须仍是 BongClient");
        }

        private static void assertGeneratedSourceContract(
            List<Path> sourceRoots,
            List<Path> sourceFiles
        ) {
            Path generatedRoot = sourceRoots.stream()
                .filter(root -> root.endsWith(Path.of("generated/sources/bongBlocks/java")))
                .findFirst()
                .orElseThrow();
            List<String> generated = sourceFiles.stream()
                .filter(path -> path.startsWith(generatedRoot))
                .map(path -> generatedRoot.relativize(path).toString().replace('\\', '/'))
                .toList();
            assertEquals(List.of(BONG_BLOCK_GENERATOR_CLASS), generated,
                "generated main Java root 只能包含冻结的 BongBlockIds；新增 production source 必须纳入 R6 审计");
        }

        private Path relativePath(Path sourcePath) {
            sourcePath = sourcePath.normalize();
            for (Path sourceRoot : sourceRoots) {
                if (sourcePath.startsWith(sourceRoot)) {
                    return sourceRoot.relativize(sourcePath);
                }
            }
            throw new AssertionError("source file escaped production roots: " + sourcePath);
        }

        private Path relativePath(CompilationUnitTree unit) {
            Path sourcePath = Path.of(unit.getSourceFile().toUri()).normalize();
            for (Path sourceRoot : sourceRoots) {
                if (sourcePath.startsWith(sourceRoot)) {
                    return sourceRoot.relativize(sourcePath);
                }
            }
            throw new AssertionError("source unit escaped production roots: " + sourcePath);
        }

        private long line(CompilationUnitTree unit, Tree tree) {
            return line(trees, unit, tree);
        }

        private long line(Trees sourceTrees, CompilationUnitTree unit, Tree tree) {
            long position = sourceTrees.getSourcePositions().getStartPosition(unit, tree);
            return unit.getLineMap().getLineNumber(position);
        }

        private String site(CompilationUnitTree unit, Tree tree) {
            return relativePath(unit) + ":" + line(unit, tree);
        }

        private static String methodKey(ExecutableElement method) {
            Element owner = method.getEnclosingElement();
            String ownerName = owner instanceof TypeElement type
                ? type.getQualifiedName().toString() : owner.toString();
            return ownerName + "#" + method.getSimpleName() + "/" + method.getParameters().size();
        }

        private static String formatDiagnostic(Diagnostic<? extends JavaFileObject> diagnostic) {
            JavaFileObject source = diagnostic.getSource();
            String location = source == null ? "<unknown>" : source.getName();
            return location + ":" + diagnostic.getLineNumber() + ": "
                + diagnostic.getMessage(null);
        }
    }

    private static Path clientRoot() {
        Path candidate = Path.of("").toAbsolutePath().normalize();
        while (candidate != null) {
            if (Files.isDirectory(candidate.resolve("src/main/java/com/bong/client"))) {
                return candidate;
            }
            Path nestedClient = candidate.resolve("client");
            if (Files.isDirectory(nestedClient.resolve("src/main/java/com/bong/client"))) {
                return nestedClient;
            }
            candidate = candidate.getParent();
        }
        throw new AssertionError("无法定位 client source tree");
    }
}
