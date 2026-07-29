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
    private static final Pattern DIRECT_RECEIVER_CHANNEL = Pattern.compile(
        "registerGlobalReceiver\\s*\\(\\s*new Identifier\\(\\s*\\\"bong\\\"\\s*,\\s*\\\"([^\\\"]+)\\\""
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
        List<String> registeredDirectChannels = new ArrayList<>();
        int receiverCount = 0;
        try (Stream<Path> files = Files.walk(sourceRoot)) {
            for (Path path : files.filter(path -> path.toString().endsWith(".java")).toList()) {
                String source = Files.readString(path);
                int count = countMatches(RECEIVER_CALL, source);
                if (count > 0) {
                    receiverFiles.add(sourceRoot.relativize(path));
                    receiverCount += count;
                    Matcher directChannels = DIRECT_RECEIVER_CHANNEL.matcher(source);
                    while (directChannels.find()) {
                        registeredDirectChannels.add("bong:" + directChannels.group(1));
                    }
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
        registeredDirectChannels.sort(String::compareTo);
        assertEquals(
            DIRECT_RECEIVER_CHANNELS.stream().sorted().toList(),
            registeredDirectChannels,
            "直接字面量注册的 receiver 必须与 R6 channel 决策账本逐项对齐"
        );
        assertEquals(31, SIDE_CHANNELS.size());
        assertEquals(3, EXEMPT_SIDE_CHANNELS.size());
        assertEquals(28, SIDE_CHANNELS.size() - EXEMPT_SIDE_CHANNELS.size());

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
