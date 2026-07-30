package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

final class WireS2cContractPinTest {
    private static final Pattern RECEIVER_CALL = Pattern.compile(
        "ClientPlayNetworking\\.registerGlobalReceiver\\s*\\("
    );
    private static final Pattern PREFIX_LITERAL = Pattern.compile(
        "\\\"([A-Z][A-Z0-9_]*_)\\\""
    );

    private static final List<String> SIDE_CHANNELS = List.of(
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
        "bong:agent_ui_request",
        "bong:agent_ui_close",
        "bong:halfstep_rechallenge",
        "bong:shader_state"
    );

    private static final List<String> RECEIVER_ARGUMENTS = List.of(
        "new Identifier(NpcMetadataHandler.CHANNEL_NAMESPACE, NpcMetadataHandler.CHANNEL_PATH)",
        "new Identifier(NpcLodHandler.CHANNEL_NAMESPACE, NpcLodHandler.CHANNEL_PATH)",
        "new Identifier(NpcBubbleHandler.CHANNEL_NAMESPACE, NpcBubbleHandler.CHANNEL_PATH)",
        "new Identifier(NpcMoodHandler.CHANNEL_NAMESPACE, NpcMoodHandler.CHANNEL_PATH)",
        "new Identifier(TsyBossHealthHandler.CHANNEL_NAMESPACE, TsyBossHealthHandler.CHANNEL_PATH)",
        "new Identifier(TsyDeathVfxHandler.CHANNEL_NAMESPACE, TsyDeathVfxHandler.CHANNEL_PATH)",
        "new Identifier(\"bong\", \"server_data\")",
        "new Identifier(\"bong\", \"locust_swarm_warning\")",
        "new Identifier(\"bong\", \"vfx_event\")",
        "QiAttritionVfxPlayer.CHANNEL",
        "new Identifier(\"bong\", \"audio/play\")",
        "new Identifier(\"bong\", \"audio/stop\")",
        "new Identifier(\"bong\", \"tiandao_presence\")",
        "new Identifier(\"bong\", \"audio/ambient_zone\")",
        "new Identifier(\"bong\", \"zone_environment\")",
        "new Identifier(\"bong\", \"mutation_visual\")",
        "new Identifier(\"bong\", \"crack_reading\")",
        "new Identifier(\"bong\", \"resonance_lock\")",
        "new Identifier(\"bong\", \"resonance_lock_end\")",
        "new Identifier(\"bong\", \"void_erosion_visual\")",
        "new Identifier(SpiderDisguiseHandler.CHANNEL_NAMESPACE, SpiderDisguiseHandler.CHANNEL_PATH_ENTER)",
        "new Identifier(SpiderDisguiseHandler.CHANNEL_NAMESPACE, SpiderDisguiseHandler.CHANNEL_PATH_AMBUSH)",
        "new Identifier(RatQiTierHandler.CHANNEL_NAMESPACE, RatQiTierHandler.CHANNEL_PATH)",
        "new Identifier(DaoZhanDisguiseHandler.CHANNEL_NAMESPACE, DaoZhanDisguiseHandler.CHANNEL_PATH_ENTER)",
        "new Identifier(DaoZhanDisguiseHandler.CHANNEL_NAMESPACE, DaoZhanDisguiseHandler.CHANNEL_PATH_REVEAL)",
        "new Identifier( com.bong.client.fauna.HallucinationLayerHandler.CHANNEL_NAMESPACE, com.bong.client.fauna.HallucinationLayerHandler.CHANNEL_PATH )",
        "new Identifier( com.bong.client.dying_elder.DyingElderEncounterHandler.CHANNEL_NAMESPACE, com.bong.client.dying_elder.DyingElderEncounterHandler.CHANNEL_PATH )",
        "new Identifier(\"bong\", \"agent_ui_request\")",
        "com.bong.client.network.AgentUiPayloadHandler.AGENT_UI_CLOSE_CHANNEL",
        "new Identifier(\"bong\", \"halfstep_rechallenge\")",
        "new Identifier(\"bong\", \"era_ambiance\")",
        "new Identifier(ShaderStateHandler.CHANNEL_NAMESPACE, ShaderStateHandler.CHANNEL_PATH)"
    );

    private record IndirectReceiverBinding(
        String relativePath,
        String channel,
        List<String> declarations
    ) {}

    private static final List<IndirectReceiverBinding> INDIRECT_RECEIVER_BINDINGS = List.of(
        binding("com/bong/client/npc/NpcMetadataHandler.java", "bong:npc_metadata",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"npc_metadata\";"),
        binding("com/bong/client/npc/NpcLodHandler.java", "bong:npc_lod",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"npc_lod\";"),
        binding("com/bong/client/npc/NpcBubbleHandler.java", "bong:npc_bubble",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"npc_bubble\";"),
        binding("com/bong/client/npc/NpcMoodHandler.java", "bong:npc_mood",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"npc_mood\";"),
        binding("com/bong/client/tsy/TsyBossHealthHandler.java", "bong:tsy_boss_health",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"tsy_boss_health\";"),
        binding("com/bong/client/tsy/TsyDeathVfxHandler.java", "bong:tsy_death_vfx",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"tsy_death_vfx\";"),
        binding("com/bong/client/visual/particle/QiAttritionVfxPlayer.java", "bong:vfx/qi_attrition",
            "public static final Identifier CHANNEL = new Identifier(\"bong\", \"vfx/qi_attrition\");"),
        binding("com/bong/client/spider/SpiderDisguiseHandler.java", "bong:spider_disguise_enter",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH_ENTER = \"spider_disguise_enter\";"),
        binding("com/bong/client/spider/SpiderDisguiseHandler.java", "bong:spider_ambush_trigger",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH_AMBUSH = \"spider_ambush_trigger\";"),
        binding("com/bong/client/fauna/RatQiTierHandler.java", "bong:rat_qi_tier",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"rat_qi_tier\";"),
        binding("com/bong/client/daozhan/DaoZhanDisguiseHandler.java", "bong:daozhan_disguise_enter",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH_ENTER = \"daozhan_disguise_enter\";"),
        binding("com/bong/client/daozhan/DaoZhanDisguiseHandler.java", "bong:daozhan_reveal",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH_REVEAL = \"daozhan_reveal\";"),
        binding("com/bong/client/fauna/HallucinationLayerHandler.java", "bong:core_absorption_hallucination",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"core_absorption_hallucination\";"),
        binding("com/bong/client/dying_elder/DyingElderEncounterHandler.java", "bong:elder_encounter",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"elder_encounter\";"),
        binding("com/bong/client/network/AgentUiPayloadHandler.java", "bong:agent_ui_close",
            "public static final Identifier AGENT_UI_CLOSE_CHANNEL =\n        new Identifier(\"bong\", \"agent_ui_close\");"),
        binding("com/bong/client/iris/ShaderStateHandler.java", "bong:shader_state",
            "public static final String CHANNEL_NAMESPACE = \"bong\";",
            "public static final String CHANNEL_PATH = \"shader_state\";")
    );

    private static final Set<String> DIRECT_RECEIVER_CHANNELS = Set.of(
        "bong:server_data",
        "bong:locust_swarm_warning",
        "bong:vfx_event",
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
        "bong:agent_ui_request",
        "bong:halfstep_rechallenge",
        "bong:era_ambiance"
    );

    private static final Set<String> EXEMPT_SIDE_CHANNELS = Set.of(
        "bong:agent_ui_request",
        "bong:agent_ui_close",
        "bong:shader_state"
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

    @Test
    void everyClientS2cReceiverIsCountedAndEveryBypassHasAMigrationDecision() throws IOException {
        Path sourceRoot = clientRoot().resolve("src/main/java");
        List<Path> receiverFiles = new ArrayList<>();
        List<String> receiverArguments = new ArrayList<>();
        int receiverCount = 0;
        try (Stream<Path> files = Files.walk(sourceRoot)) {
            for (Path path : files.filter(path -> path.toString().endsWith(".java")).toList()) {
                String source = Files.readString(path);
                int count = countMatches(RECEIVER_CALL, source);
                if (count > 0) {
                    receiverFiles.add(sourceRoot.relativize(path));
                    receiverCount += count;
                    receiverArguments.addAll(receiverArguments(source));
                }
            }
        }

        receiverFiles.sort(Comparator.comparing(Path::toString));
        assertEquals(
            List.of(
                Path.of("com/bong/client/BongNetworkHandler.java"),
                Path.of("com/bong/client/iris/IrisBootstrap.java")
            ),
            receiverFiles,
            "新增 receiver 文件必须进入 R6 旁路普查，而不是绕过 BongNetworkHandler"
        );
        assertEquals(32, receiverCount,
            "P0 基线为 32 个 global receiver（server_data 1 + side channels 31）");
        receiverArguments.sort(String::compareTo);
        assertEquals(
            RECEIVER_ARGUMENTS.stream().sorted().toList(),
            receiverArguments,
            "所有 receiver 的注册参数必须与 R6 channel 决策账本逐项对齐，间接常量也不能重定向"
        );
        assertEquals(31, SIDE_CHANNELS.size());
        assertEquals(3, EXEMPT_SIDE_CHANNELS.size());
        assertEquals(28, SIDE_CHANNELS.size() - EXEMPT_SIDE_CHANNELS.size());
        assertEquals(16, DIRECT_RECEIVER_CHANNELS.size());
        assertEquals(16, INDIRECT_RECEIVER_BINDINGS.size());
        Set<String> indirectChannels = INDIRECT_RECEIVER_BINDINGS.stream()
            .map(IndirectReceiverBinding::channel)
            .collect(java.util.stream.Collectors.toSet());
        assertEquals(16, indirectChannels.size(),
            "间接 receiver channel 必须一对一，不能重复指向同一 ID");
        Set<String> allChannels = new TreeSet<>(DIRECT_RECEIVER_CHANNELS);
        Set<String> channelOverlap = new TreeSet<>(indirectChannels);
        channelOverlap.retainAll(DIRECT_RECEIVER_CHANNELS);
        assertEquals(Set.of(), channelOverlap,
            "直接与间接 receiver channel 不能重复");
        allChannels.addAll(indirectChannels);
        assertTrue(allChannels.remove("bong:server_data"));
        assertEquals(
            Set.copyOf(SIDE_CHANNELS),
            allChannels,
            "直接/间接 receiver ID 必须唯一解析成 server_data 外的 31 个旁路"
        );

        for (IndirectReceiverBinding binding : INDIRECT_RECEIVER_BINDINGS) {
            String source = Files.readString(sourceRoot.resolve(binding.relativePath()));
            for (String declaration : binding.declarations()) {
                assertEquals(
                    1,
                    countOccurrences(source, declaration),
                    () -> binding.relativePath() + " 必须精确声明一次 " + binding.channel()
                );
            }
        }

        String allProductionSource = readAllProductionSource(sourceRoot);
        for (String channel : SIDE_CHANNELS) {
            String path = channel.substring("bong:".length());
            assertTrue(
                allProductionSource.contains("\"" + path + "\""),
                () -> "旁路清单 channel 已不在 production source：" + channel
            );
        }
    }

    @Test
    void protoEnumPrefixInventoryAndNormalizationModesStayFrozen() throws IOException {
        String source = Files.readString(clientRoot().resolve(
            "src/main/java/com/bong/client/network/ProtoServerDataBridge.java"
        ));
        Set<String> actualPrefixes = new TreeSet<>();
        Matcher matcher = PREFIX_LITERAL.matcher(source);
        int prefixLiteralReferences = 0;
        while (matcher.find()) {
            actualPrefixes.add(matcher.group(1));
            prefixLiteralReferences++;
        }

        assertEquals(ENUM_PREFIXES, actualPrefixes,
            "新增/删除 proto enum 前缀必须更新 R6 bridge normalization 账本");
        assertEquals(43, actualPrefixes.size());
        assertEquals(57, prefixLiteralReferences,
            "P0 冻结 57 处 enum prefix literal 引用；迁到 registry 时要原子更新该断言");
        for (String helper : List.of(
            "stripEnumPrefix(",
            "stripEnumPrefixCapitalized(",
            "stripEnumPrefixPascalCase(",
            "bridgeStripEnumsOmittingUnspecified("
        )) {
            assertTrue(source.contains(helper),
                () -> "R6 冻结的 enum normalization mode 消失：" + helper);
        }
    }

    private static IndirectReceiverBinding binding(
        String relativePath,
        String channel,
        String... declarations
    ) {
        return new IndirectReceiverBinding(relativePath, channel, List.of(declarations));
    }

    private static int countOccurrences(String source, String needle) {
        int count = 0;
        int searchFrom = 0;
        while ((searchFrom = source.indexOf(needle, searchFrom)) >= 0) {
            count++;
            searchFrom += needle.length();
        }
        return count;
    }

    private static List<String> receiverArguments(String source) {
        List<String> arguments = new ArrayList<>();
        String needle = "ClientPlayNetworking.registerGlobalReceiver";
        int searchFrom = 0;
        while ((searchFrom = source.indexOf(needle, searchFrom)) >= 0) {
            int argumentStart = source.indexOf('(', searchFrom + needle.length()) + 1;
            assertTrue(argumentStart > 0, "receiver call must have an argument list");
            int depth = 0;
            boolean inString = false;
            boolean escaped = false;
            int index = argumentStart;
            for (; index < source.length(); index++) {
                char current = source.charAt(index);
                if (inString) {
                    if (escaped) {
                        escaped = false;
                    } else if (current == '\\') {
                        escaped = true;
                    } else if (current == '"') {
                        inString = false;
                    }
                } else if (current == '"') {
                    inString = true;
                } else if (current == '(') {
                    depth++;
                } else if (current == ')') {
                    assertTrue(depth > 0, "receiver identifier has balanced parentheses");
                    depth--;
                } else if (current == ',' && depth == 0) {
                    break;
                }
            }
            assertTrue(index < source.length(), "receiver call must separate identifier and callback");
            arguments.add(source.substring(argumentStart, index).replaceAll("\\s+", " ").trim());
            searchFrom = index + 1;
        }
        return arguments;
    }

    private static String readAllProductionSource(Path sourceRoot) throws IOException {
        try (Stream<Path> files = Files.walk(sourceRoot)) {
            StringBuilder joined = new StringBuilder();
            for (Path path : files.filter(path -> path.toString().endsWith(".java")).toList()) {
                joined.append(Files.readString(path)).append('\n');
            }
            return joined.toString();
        }
    }

    private static int countMatches(Pattern pattern, String source) {
        int count = 0;
        Matcher matcher = pattern.matcher(source);
        while (matcher.find()) {
            count++;
        }
        return count;
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
