package com.bong.client.visual.particle;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-anim-fidelity-v1 P5 —— 招式粒子接线矩阵的 <b>client 侧闭环</b>。
 *
 * <p>消费与 server 侧 {@code skill_vfx_wiring_test.rs} 同一份 checked-in 清单
 * {@code bong/skill_vfx_wiring_manifest.json}（server {@code P5_SKILL_VFX_WIRING} 常量表的
 * 单向导出，重生成唯一入口
 * {@code cd server && BONG_REGEN_VFX_MANIFEST=1 cargo test skill_vfx_wiring}），
 * 逐行断言 server 会发射的每个 event_id 在 client 端真的有播放器接住、且落到清单声明的那个类上。
 *
 * <p><b>这组断言防的是什么</b>：event_id 是两端各写一遍的裸字符串。server 改了 emit 的 id、
 * client 忘了同步 {@link VfxBootstrap} 注册 → {@link com.bong.client.network.VfxParticleBridge}
 * 查表落空记 bridgeMiss，招式<b>静默无特效</b>——不报错、不崩溃，只是玩家什么都看不到。
 * 这类断链在 P5 之前的仓库里出现过多次（见 VfxRegistryTest 中 woliu / tuike 各条注释）。
 */
public class SkillVfxWiringManifestTest {
    static final String MANIFEST_RESOURCE = "bong/skill_vfx_wiring_manifest.json";

    private static final String REGEN_HINT =
        "清单由 server P5_SKILL_VFX_WIRING 单向生成、禁止手改；重生成："
            + "cd server && BONG_REGEN_VFX_MANIFEST=1 cargo test skill_vfx_wiring";

    /** P5 去复用前借用的那些 player：任何一行回落到它们身上就是去复用被撤销。 */
    private static final Set<String> LEGACY_BORROW_PLAYERS = Set.of(
        "SwordQiSlashPlayer",
        "BurstMeridianBengQuanPlayer",
        "YidaoPeacePulsePlayer"
    );

    /** 清单一行。 */
    record WiringRow(
        String skillId,
        String eventId,
        String color,
        String playerClass,
        String legacyEventId
    ) {
        Identifier identifier() {
            return new Identifier(
                eventId.substring(0, eventId.indexOf(':')),
                eventId.substring(eventId.indexOf(':') + 1)
            );
        }
    }

    @BeforeEach
    void registerDefaults() {
        VfxRegistry.instance().clearForTests();
        VfxBootstrap.registerDefaults();
    }

    @AfterEach
    void clearRegistry() {
        VfxRegistry.instance().clearForTests();
    }

    static List<WiringRow> loadManifest() throws IOException {
        try (InputStream input = SkillVfxWiringManifestTest.class.getClassLoader()
                .getResourceAsStream(MANIFEST_RESOURCE)) {
            assertNotNull(input, "缺少粒子接线清单 test fixture: " + MANIFEST_RESOURCE
                + " —— " + REGEN_HINT);
            JsonArray root = JsonParser
                .parseReader(new InputStreamReader(input, StandardCharsets.UTF_8))
                .getAsJsonArray();
            List<WiringRow> rows = new ArrayList<>();
            for (JsonElement element : root) {
                JsonObject object = element.getAsJsonObject();
                rows.add(new WiringRow(
                    object.get("skill_id").getAsString(),
                    object.get("event_id").getAsString(),
                    object.get("color").getAsString(),
                    object.get("player_class").getAsString(),
                    object.get("legacy_event_id").getAsString()
                ));
            }
            assertFalse(rows.isEmpty(), "粒子接线清单不应为空数组 —— " + REGEN_HINT);
            return rows;
        }
    }

    /** 清单形态：按 event_id 严格升序（已排序且无重复），锁 server 侧导出契约。 */
    @Test
    void manifestIsSortedAndUnique() throws IOException {
        List<WiringRow> rows = loadManifest();
        for (int i = 1; i < rows.size(); i++) {
            assertTrue(rows.get(i - 1).eventId().compareTo(rows.get(i).eventId()) < 0,
                "清单第 " + i + " 行 `" + rows.get(i).eventId() + "` 未严格大于前行 `"
                    + rows.get(i - 1).eventId() + "`（排序漂移或重复条目，疑似手改）—— " + REGEN_HINT);
        }
    }

    /**
     * 核心闭环：清单每一行的 event_id 都被 {@link VfxBootstrap} 注册，且落到清单声明的 player 类。
     *
     * <p>只断言"注册了"不够——注册到<b>错误的 player</b> 一样是错的视觉（P5 之前真脉 5 招正是
     * "注册了，但注册到剑气播放器"）。所以逐行连类名一起对拍。
     */
    @Test
    void everyWiredEventIdResolvesToDeclaredPlayer() throws IOException {
        for (WiringRow row : loadManifest()) {
            Identifier id = row.identifier();
            assertTrue(VfxRegistry.instance().contains(id),
                "招式 " + row.skillId() + " 的粒子 event_id `" + row.eventId()
                    + "` 未在 VfxBootstrap 注册——server 会发射它，client 查表落空走 bridgeMiss，"
                    + "该招式在真机上完全没有粒子反馈。" + REGEN_HINT);

            Optional<VfxPlayer> player = VfxRegistry.instance().lookup(id);
            assertTrue(player.isPresent(),
                "contains() 为真但 lookup() 落空，registry 自相矛盾：" + row.eventId());
            assertEquals(row.playerClass(), player.get().getClass().getSimpleName(),
                "招式 " + row.skillId() + " 的 `" + row.eventId() + "` 注册到了 `"
                    + player.get().getClass().getSimpleName() + "`，清单声明的是 `"
                    + row.playerClass() + "`——接错 player 会画出别家招式的形态");
        }
    }

    /**
     * 负向锁：11 行全部脱离了 P5 之前借用的那些 player。
     *
     * <p>这条与上一条方向相反且不可互相替代——上一条锁"接到了对的类"，本条锁"没接回旧的借用类"，
     * 后者是 P5 交付物本身（去复用）。
     */
    @Test
    void noWiredEventIdFallsBackToBorrowedPlayer() throws IOException {
        for (WiringRow row : loadManifest()) {
            assertFalse(LEGACY_BORROW_PLAYERS.contains(row.playerClass()),
                "招式 " + row.skillId() + " 的粒子仍指向借用播放器 `" + row.playerClass()
                    + "`——P5 的交付物就是让这 11 招脱离借用");

            VfxPlayer actual = VfxRegistry.instance().lookup(row.identifier()).orElseThrow();
            assertFalse(LEGACY_BORROW_PLAYERS.contains(actual.getClass().getSimpleName()),
                "招式 " + row.skillId() + " 在 registry 里实际落到借用播放器 `"
                    + actual.getClass().getSimpleName() + "`（清单声明 `" + row.playerClass()
                    + "`）——去复用被撤销");
        }
    }

    /**
     * 集合一致性：清单里归属某个 player 类的 event_id 集合，与该 player 类自己声明的
     * {@code EVENT_IDS} 完全相等。
     *
     * <p>防的是"两端各加各的"——比如 server 表加了第 4 个爆脉招、client player 的 EVENT_IDS
     * 忘了同步（或反过来 client 多注册了一个 server 根本不发的死 id）。
     */
    @Test
    void manifestEventIdSetMatchesEachPlayerDeclaredIds() throws IOException {
        List<WiringRow> rows = loadManifest();

        assertPlayerIdSetMatches(rows, "ZhenmaiPulsePlayer", ZhenmaiPulsePlayer.EVENT_IDS);
        assertPlayerIdSetMatches(
            rows, "BurstMeridianFamilyPlayer", BurstMeridianFamilyPlayer.EVENT_IDS);
        assertPlayerIdSetMatches(rows, "NpcSkillAuraPlayer", NpcSkillAuraPlayer.EVENT_IDS);
    }

    private static void assertPlayerIdSetMatches(
        List<WiringRow> rows,
        String playerClass,
        Identifier[] declaredIds
    ) {
        Set<String> fromManifest = new TreeSet<>();
        for (WiringRow row : rows) {
            if (row.playerClass().equals(playerClass)) {
                fromManifest.add(row.eventId());
            }
        }
        Set<String> fromPlayer = new TreeSet<>();
        for (Identifier id : declaredIds) {
            fromPlayer.add(id.toString());
        }
        assertEquals(fromManifest, fromPlayer,
            playerClass + " 声明的 EVENT_IDS 与清单归属它的 event_id 集合不一致——"
                + "清单=" + fromManifest + "，player 声明=" + fromPlayer + "。"
                + "两端各加各的会让新招式 server 发得出、client 接不住（或反之留死注册）。"
                + REGEN_HINT);

        // EVENT_IDS 自身不得有重复条目（重复会让 bootstrap 白注册一次、掩盖漏项）。
        assertEquals(declaredIds.length, fromPlayer.size(),
            playerClass + ".EVENT_IDS 含重复条目：" + java.util.Arrays.toString(declaredIds));
    }

    /** 清单每个 id 都是合法的 `bong:` 命名空间 Identifier（非法字符会让 client 解析静默失败）。 */
    @Test
    void manifestEventIdsAreBongNamespaced() throws IOException {
        for (WiringRow row : loadManifest()) {
            String eventId = row.eventId();
            int colon = eventId.indexOf(':');
            assertTrue(colon > 0, "event_id `" + eventId + "` 缺少 `namespace:path` 冒号分隔");
            assertEquals("bong", eventId.substring(0, colon),
                "event_id `" + eventId + "` 命名空间必须是 bong");
            assertNotNull(Identifier.tryParse(eventId),
                "event_id `" + eventId + "` 不是合法 MC Identifier——client 端解析会静默失败");
        }
    }

    /** 清单每种颜色都是 `#RRGGBB`（server 端同样断言一次，两端各自守住自己的解析前提）。 */
    @Test
    void manifestColorsAreSixDigitHex() throws IOException {
        for (WiringRow row : loadManifest()) {
            String color = row.color();
            assertTrue(color.length() == 7 && color.charAt(0) == '#',
                row.skillId() + " 的颜色 `" + color + "` 不是 #RRGGBB 形态");
            assertTrue(color.substring(1).matches("[0-9A-Fa-f]{6}"),
                row.skillId() + " 的颜色 `" + color + "` 含非十六进制字符");
        }
    }

    /**
     * 错误分支：<b>未注册</b>的 event_id 走 bridge 时返回 false（bridgeMiss）而不是抛异常。
     *
     * <p>粒子 bridge 跑在渲染路径上，一个未知 id 让它抛异常就等于一条坏 server 事件能把客户端
     * 渲染打断。这里连同 P5 退役的两个旧借用 id 一起验——它们已从 bootstrap 撤除注册，
     * 万一还有旧版 server 在发，客户端必须安静地忽略掉。
     */
    @Test
    void unregisteredEventIdMissesBridgeWithoutThrowing() {
        // 同包，直接注入 registry 构造（生产构造函数走 VfxRegistry.instance()，语义相同）。
        BongVfxParticleBridge bridge = new BongVfxParticleBridge(VfxRegistry.instance());

        for (String deadId : List.of(
                "bong:definitely_not_registered",
                "bong:jiemai_burst_blood",
                "bong:jiemai_neutralize_dust")) {
            var payload = new com.bong.client.network.VfxEventPayload.SpawnParticle(
                Identifier.tryParse(deadId),
                new double[] { 0.0, 64.0, 0.0 },
                Optional.empty(),
                java.util.OptionalInt.empty(),
                Optional.empty(),
                java.util.OptionalInt.empty(),
                java.util.OptionalInt.empty()
            );
            assertFalse(bridge.spawnParticle(payload),
                "未注册 event_id `" + deadId + "` 应记 bridgeMiss（返回 false）而不是被静默当成成功");
        }
    }

    /**
     * 已注册的 P5 id 在 registry 层查得到（bridge 层因单测无 MinecraftClient 实例必然返回
     * false，所以成功路径只能断到 registry 这一层——再往下属真机行为）。
     */
    @Test
    void registeredEventIdsAreResolvableAtRegistryLayer() throws IOException {
        Set<String> seen = new LinkedHashSet<>();
        for (WiringRow row : loadManifest()) {
            assertTrue(VfxRegistry.instance().lookup(row.identifier()).isPresent(),
                "已接线的 `" + row.eventId() + "` 在 registry 查不到");
            assertTrue(seen.add(row.eventId()),
                "清单出现重复 event_id `" + row.eventId() + "`——" + REGEN_HINT);
        }
        assertEquals(11, seen.size(),
            "P5 接线矩阵应为 11 行（真脉 5 + 爆脉 3 + NPC 3），实际 " + seen.size()
                + "。行数变化要连同 plan §P5.2 矩阵表一起更新。");
    }
}
