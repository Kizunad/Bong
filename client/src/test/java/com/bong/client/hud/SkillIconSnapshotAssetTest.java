package com.bong.client.hud;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import com.bong.client.network.SkillBarConfigHandler;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.function.Predicate;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-skill-av-relink-v1 P3 —— technique 图标资产存在性扫描 + allowlist 棘轮（图标链）。
 *
 * <p>消费与 server 侧 {@code technique_icon_snapshot_test.rs} 同一份 checked-in 快照
 * {@code bong/technique_icon_snapshot.json}（TECHNIQUE_DEFINITIONS 的
 * skill_id→icon_texture 单向导出，重生成唯一入口
 * {@code cd server && BONG_REGEN_ICON_SNAPSHOT=1 cargo test technique_icon_snapshot}），
 * 逐条断言图标路径能在 client classpath（main resources）上真实命中——server 定义表
 * 指向不存在的贴图时本测试撞红，缺资产无法静默上线。
 */
class SkillIconSnapshotAssetTest {
    /**
     * 缺资产 allowlist 棘轮：登记"已知尚无图标资产"的 technique，**只许缩小不许增长**——
     * ① 非 allowlist 条目资产必须存在；② allowlist 条目资产必须仍缺失（资产落地即强制
     * 删条目）；③ allowlist ⊆ 快照 key 集（防僵尸条目）。P2（2026-07-18）生成
     * skill_scroll_morph_yixing.png 后已清零；新缺图的唯一出路是补资产（见冻结基线）。
     */
    private static final Set<String> MISSING_ICON_ALLOWLIST = Set.of();

    /**
     * PR-1 冻结基线（机器可执行的"只缩不涨"棘轮）：{@link #MISSING_ICON_ALLOWLIST} 必须
     * 是本集合的子集——删除条目（资产落地）自然通过，新增任何条目立刻判红。本基线
     * **永不追加**：新缺图的唯一出路是补资产，不得经由扩大 allowlist 放行；P2 落图
     * 清零 allowlist 时也不修改本基线。
     */
    private static final Set<String> PR1_MISSING_ICON_BASELINE = Set.of("morph.yixing");

    /**
     * 重复映射白名单：两招显式共用同一图标文件才登记（需说明理由），当前空集。
     * server 侧 technique_icon_snapshot_test.rs 持有同一份空集白名单。
     */
    private static final Set<String> DUPLICATE_TEXTURE_ALLOWLIST = Set.of();

    private static final Set<String> ALLOWED_NAMESPACES = Set.of("bong", "bong-client");

    private static final String REGEN_HINT =
        "快照由 server TECHNIQUE_DEFINITIONS 单向生成、禁止手改；重生成："
            + "cd server && BONG_REGEN_ICON_SNAPSHOT=1 cargo test technique_icon_snapshot";

    private static Map<String, String> loadSnapshot() throws IOException {
        String resourcePath = "bong/technique_icon_snapshot.json";
        try (InputStream input = SkillIconSnapshotAssetTest.class.getClassLoader()
                .getResourceAsStream(resourcePath)) {
            assertNotNull(input, "缺少图标快照 test fixture: " + resourcePath + " —— " + REGEN_HINT);
            JsonObject root = JsonParser
                .parseReader(new InputStreamReader(input, StandardCharsets.UTF_8))
                .getAsJsonObject();
            Map<String, String> snapshot = new LinkedHashMap<>();
            for (var entry : root.entrySet()) {
                snapshot.put(entry.getKey(), entry.getValue().getAsString());
            }
            assertFalse(snapshot.isEmpty(), "图标快照不应为空对象 —— " + REGEN_HINT);
            return snapshot;
        }
    }

    /**
     * {@code ns:path} → **聚合 classpath** 资源 URL（含 test resources）——只用于
     * {@link #testResourceProbeCannotSatisfyProductionAssetCheck} 的反向探针；生产资产
     * 存在性一律走 {@link #mainResourceExists}，防 test resources 假资产冒充真实资产。
     */
    private static URL classpathResource(String iconTexture) {
        int colon = iconTexture.indexOf(':');
        String namespace = iconTexture.substring(0, colon);
        String path = iconTexture.substring(colon + 1);
        return SkillIconSnapshotAssetTest.class.getResource("/assets/" + namespace + "/" + path);
    }

    /**
     * 生产资源根 {@code client/src/main/resources}（gradle test 工作目录 = client/；
     * 兼容从仓库根启动的场景）。定位失败直接抛错——存在性检查绝不静默降级回聚合
     * classpath。
     */
    private static Path mainResourcesRoot() {
        Path cwd = Path.of(System.getProperty("user.dir"));
        // 权威根 = main 源码目录（checked-in 生产资产，本测试要保护的对象——源码删除
        // 必须立刻判红，不受可能陈旧的构建输出遮蔽）；build/resources/main 仅作源码根
        // 不可见的打包场景兜底。两者都不含 test resources，假资产无从混入；打包一致性
        // 由 resource pack 构建 CI 与 e2e 另行把关。
        for (Path candidate : List.of(
                cwd.resolve("src/main/resources"),
                cwd.resolve("client/src/main/resources"),
                cwd.resolve("build/resources/main"),
                cwd.resolve("client/build/resources/main"))) {
            if (Files.isDirectory(candidate)) {
                return candidate;
            }
        }
        throw new IllegalStateException(
            "无法定位 client main resources 根（user.dir=" + cwd
                + "）——生产资产存在性检查必须绑定 main source set，不得回退聚合 classpath");
    }

    /** {@code ns:path} 在 **main resources**（生产资源包源）中真实存在为普通文件。 */
    private static boolean mainResourceExists(String iconTexture) {
        int colon = iconTexture.indexOf(':');
        String namespace = iconTexture.substring(0, colon);
        String path = iconTexture.substring(colon + 1);
        return Files.isRegularFile(
            mainResourcesRoot().resolve("assets").resolve(namespace).resolve(path));
    }

    /** 映射约束：空串 / 缺冒号 / 坏命名空间 / Identifier 不可解析 / 非 .png 全部判红。 */
    @Test
    void everySnapshotIconPathIsWellFormed() throws IOException {
        for (Map.Entry<String, String> entry : loadSnapshot().entrySet()) {
            String skillId = entry.getKey();
            String iconTexture = entry.getValue();
            assertFalse(iconTexture.isEmpty(),
                "technique `" + skillId + "` 的 icon_texture 为空串——Skill 槽发射契约要求"
                    + "每条 technique 都有可下发的图标路径");
            assertTrue(iconTexture.indexOf(':') > 0,
                "technique `" + skillId + "` 的 icon_texture `" + iconTexture
                    + "` 缺少 namespace:path 冒号分隔");
            String namespace = iconTexture.substring(0, iconTexture.indexOf(':'));
            assertTrue(ALLOWED_NAMESPACES.contains(namespace),
                "technique `" + skillId + "` 的图标命名空间 `" + namespace + "` 不在允许集 "
                    + ALLOWED_NAMESPACES + " 内（icon_texture=" + iconTexture
                    + "）——P0 决议只认 bong / bong-client 两个资源根");
            assertNotNull(Identifier.tryParse(iconTexture),
                "technique `" + skillId + "` 的 icon_texture `" + iconTexture
                    + "` 无法被 Identifier.tryParse 解析——HudTextureProbe/BongHud 端会静默丢弃");
            assertTrue(iconTexture.endsWith(".png"),
                "technique `" + skillId + "` 的图标 `" + iconTexture + "` 不是 .png 资产");
        }
    }

    /** allowlist 之外的每条图标路径都必须命中 main resources 真实资产。 */
    @Test
    void everyNonAllowlistedIconExistsInMainResources() throws IOException {
        List<String> missing = new ArrayList<>();
        for (Map.Entry<String, String> entry : loadSnapshot().entrySet()) {
            if (MISSING_ICON_ALLOWLIST.contains(entry.getKey())) {
                continue;
            }
            if (!mainResourceExists(entry.getValue())) {
                missing.add(entry.getKey() + " → " + entry.getValue());
            }
        }
        assertTrue(missing.isEmpty(),
            "以下 technique 图标在 client main resources（生产资源包源）中不存在，HUD 将无图可渲染："
                + missing + "——补齐 PNG 资产，或（仅限确认资产尚未生成时）登记 "
                + "MISSING_ICON_ALLOWLIST 并在 plan 中挂 [BLOCKED: 需 /gen-image] 记录");
    }

    /** 棘轮上限：allowlist 必须是 PR-1 冻结基线的子集——新增条目（掩盖新缺图）判红。 */
    @Test
    void allowlistMayOnlyShrinkFromFrozenBaseline() {
        assertTrue(PR1_MISSING_ICON_BASELINE.containsAll(MISSING_ICON_ALLOWLIST),
            "MISSING_ICON_ALLOWLIST 出现冻结基线之外的条目："
                + MISSING_ICON_ALLOWLIST.stream()
                    .filter(id -> !PR1_MISSING_ICON_BASELINE.contains(id)).toList()
                + "——棘轮只许缩小：新缺图必须补资产判绿，不得经由扩大 allowlist 放行"
                + "（PR1_MISSING_ICON_BASELINE 永不追加）");
    }

    /**
     * 反向探针：仅存在于 test resources 的假资产能被聚合 classpath 查到，但**不能**
     * 满足生产资产检查——证明 {@link #mainResourceExists} 真的绑定 main source set，
     * 存在性扫描无法被 test resources 假资产绕过。
     */
    @Test
    void testResourceProbeCannotSatisfyProductionAssetCheck() {
        String probe = "bong-client:textures/gui/items/__pr1_test_probe_only__.png";
        assertNotNull(classpathResource(probe),
            "前置条件破坏：test resources 探针文件缺失（client/src/test/resources/assets/"
                + "bong-client/textures/gui/items/__pr1_test_probe_only__.png）");
        assertFalse(mainResourceExists(probe),
            "main resources 存在性检查被 test resources 假资产满足——检查没有绑定生产资源根，"
                + "资产扫描可被绕过（安全网失效）");
    }

    /**
     * 图标链端到端（P3）：SkillBarConfigV1 payload 经真实客户端消费入口——
     * {@link ServerDataEnvelope#parse} → {@link SkillBarConfigHandler} → {@link SkillBarStore}
     * 槽位模型 → {@link LoadoutIconLayer}{@code .resolveExistingSkillTexture} /
     * {@link QuickBarHudPlanner}——全程复用生产代码路径，不另写替代解析逻辑。
     *
     * <p>存在性谓词以 main resources 文件检查顶替需 MC 运行时的 {@code HudTextureProbe::exists}
     * （{@code LoadoutIconLayer} 文档声明的注入缝；两者查的是同一份生产资产，且不受
     * test resources 假资产干扰）。
     * Skill 条目断言最终命中服务端下发的真实纹理路径；Item 条目断言空串（server 发射
     * 契约，plan P0 #4 修订）走 {@code itemTexture(itemId)} 富解析分支、绝不落入裸
     * {@code texture()} 分支。</p>
     */
    @Test
    void skillBarConfigPayloadDrivesProductionIconResolutionChain() throws IOException {
        Map<String, String> snapshot = loadSnapshot();
        String skillId = "sword.cleave";
        String skillIcon = snapshot.get(skillId);
        assertNotNull(skillIcon, "快照缺少 " + skillId + " —— " + REGEN_HINT);
        assertTrue(mainResourceExists(skillIcon),
            "前置条件破坏：" + skillId + " 的图标资产 " + skillIcon
                + " 应真实存在于 main resources（见存在性扫描用例）");

        String itemTemplateId = "kai_mai_pill_v0";
        String json = """
            {"v":1,"type":"skillbar_config","slots":[
              {"kind":"skill","skill_id":"%s","display_name":"劈斩","cast_duration_ms":400,"cooldown_ms":3000,"icon_texture":"%s"},
              {"kind":"item","template_id":"%s","display_name":"开脉丹","cast_duration_ms":1500,"cooldown_ms":500,"icon_texture":""},
              null,null,null,null,null,null,null
            ],"cooldown_until_ms":[0,0,0,0,0,0,0,0,0]}"""
            .formatted(skillId, skillIcon, itemTemplateId);

        SkillBarStore.resetForTests();
        try {
            ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
                json, json.getBytes(StandardCharsets.UTF_8).length);
            assertTrue(parsed.isSuccess(), "skillbar_config payload 应可解析：" + parsed.errorMessage());
            ServerDataDispatch dispatch = new SkillBarConfigHandler().handle(parsed.envelope());
            assertTrue(dispatch.handled(),
                "skillbar_config 应被真实 handler 消费而非 noOp：" + dispatch.logMessage());

            SkillBarConfig config = SkillBarStore.snapshot();
            assertEquals(SkillBarEntry.Kind.SKILL, config.slot(0).kind(), "槽 0 应落成 Skill 条目");
            assertEquals(SkillBarEntry.Kind.ITEM, config.slot(1).kind(), "槽 1 应落成 Item 条目");
            assertEquals("", config.slot(1).iconTexture(),
                "Item 槽 icon_texture 恒空串是 server 发射契约（plan P0 #4 修订），槽位模型不得篡改");

            Predicate<String> mainResourceProbe = SkillIconSnapshotAssetTest::mainResourceExists;

            // Skill 条目：生产纹理解析器命中服务端下发的真实资产路径。
            String resolved =
                LoadoutIconLayer.resolveExistingSkillTexture(config.slot(0), mainResourceProbe);
            assertEquals(skillIcon, resolved,
                "Skill 槽应经 resolveExistingSkillTexture 命中服务端下发的真实纹理 " + skillIcon
                    + "，实际 " + resolved);

            // 全量 planner 链：Skill 槽产出该纹理的贴图命令；Item 槽空串走 itemTexture
            // 富解析分支。
            List<HudRenderCommand> commands = QuickBarHudPlanner.buildCommands(
                QuickSlotConfig.empty(), config, 0, null, List.of(), 0L, 480, 270,
                mainResourceProbe);
            assertTrue(commands.stream().anyMatch(cmd ->
                    cmd.isTexturedRect() && skillIcon.equals(cmd.texturePath())),
                "planner 输出应包含 Skill 槽真实纹理 " + skillIcon + " 的贴图命令");
            assertTrue(commands.stream().anyMatch(cmd ->
                    cmd.isItemTexture() && itemTemplateId.equals(cmd.text())),
                "Item 槽空串应走 itemTexture(" + itemTemplateId + ") 富解析分支（ItemIconRegistry 链）");
            assertTrue(commands.stream().filter(HudRenderCommand::isTexturedRect)
                    .allMatch(cmd -> skillIcon.equals(cmd.texturePath())),
                "planner 输出中唯一的裸 texture 命令应只来自 Skill 槽——Item 槽空串若被回填"
                    + "路径落入裸 texture() 分支会在此撞红");
        } finally {
            SkillBarStore.resetForTests();
        }
    }

    /** 棘轮反向断言：allowlist 条目的资产必须仍然缺失——资产落地即强制删条目，只缩不涨。 */
    @Test
    void allowlistedIconsMustStillBeMissing() throws IOException {
        Map<String, String> snapshot = loadSnapshot();
        for (String skillId : MISSING_ICON_ALLOWLIST) {
            String iconTexture = snapshot.get(skillId);
            assertNotNull(iconTexture, "allowlist 条目 `" + skillId + "` 不在快照里（见另一用例）");
            assertFalse(mainResourceExists(iconTexture),
                "technique `" + skillId + "` 的图标资产 `" + iconTexture
                    + "` 已经落地——必须立刻从 MISSING_ICON_ALLOWLIST 删除该条目（棘轮只缩不涨）");
        }
    }

    /** 棘轮防僵尸：allowlist 必须是快照 key 集的子集（技能改名/删除后条目不得残留）。 */
    @Test
    void allowlistIsSubsetOfSnapshotKeys() throws IOException {
        Map<String, String> snapshot = loadSnapshot();
        for (String skillId : MISSING_ICON_ALLOWLIST) {
            assertTrue(snapshot.containsKey(skillId),
                "MISSING_ICON_ALLOWLIST 僵尸条目 `" + skillId
                    + "`：快照（即 TECHNIQUE_DEFINITIONS）里已无此 technique，删掉它");
        }
    }

    /** 重复映射判红：两招共用同一图标文件（白名单外）玩家无法从 HUD 分辨招式。 */
    @Test
    void noTwoTechniquesShareOneTextureFile() throws IOException {
        Map<String, List<String>> byTexture = new TreeMap<>();
        for (Map.Entry<String, String> entry : loadSnapshot().entrySet()) {
            byTexture.computeIfAbsent(entry.getValue(), key -> new ArrayList<>())
                .add(entry.getKey());
        }
        List<String> duplicated = new ArrayList<>();
        for (Map.Entry<String, List<String>> entry : byTexture.entrySet()) {
            if (entry.getValue().size() > 1 && !DUPLICATE_TEXTURE_ALLOWLIST.contains(entry.getKey())) {
                duplicated.add(entry.getKey() + " ← " + entry.getValue());
            }
        }
        assertTrue(duplicated.isEmpty(),
            "以下图标文件被多条 technique 共用（白名单外）：" + duplicated
                + "——确属刻意共用需登记 DUPLICATE_TEXTURE_ALLOWLIST 并说明理由");
    }
}
