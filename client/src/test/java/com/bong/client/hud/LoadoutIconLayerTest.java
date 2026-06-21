package com.bong.client.hud;

import com.bong.client.combat.SkillBarEntry;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LoadoutIconLayerTest {
    @Test
    void skillWithIconBuildsTextureAndAnqiOverlay() {
        SkillBarEntry entry = SkillBarEntry.skill(
            "anqi.multi_shot",
            "Multi Shot",
            0,
            0,
            "bong-client:textures/gui/skills/anqi_multi_shot.png"
        );

        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(entry, 10, 20, 16);

        assertEquals(3, commands.size());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isTexturedRect()
            && "bong-client:textures/gui/skills/anqi_multi_shot.png".equals(cmd.texturePath())));
        assertEquals(2, commands.stream().filter(HudRenderCommand::isRect).count());
    }

    @Test
    void echoFractalAddsSecondaryRectMarker() {
        SkillBarEntry entry = SkillBarEntry.skill(
            "anqi.echo_fractal",
            "Echo Fractal",
            0,
            0,
            "bong-client:textures/gui/skills/anqi_echo_fractal.png"
        );

        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(entry, 10, 20, 16);

        assertEquals(4, commands.size());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isRect()
            && cmd.x() == 20
            && cmd.y() == 22
            && cmd.width() == 4
            && cmd.height() == 4));
    }

    @Test
    void skillWithoutIconKeepsQuickBarTextFallbackAvailable() {
        SkillBarEntry entry = SkillBarEntry.skill("anqi.soul_inject", "Soul Inject", 0, 0, "");

        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(entry, 10, 20, 16);

        assertTrue(commands.isEmpty());
    }

    @Test
    void nullEntryKeepsQuickBarTextFallbackAvailable() {
        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(null, 10, 20, 16);

        assertTrue(commands.isEmpty());
    }

    // ---- 存在性感知解析 resolveExistingSkillTexture：消除紫黑 missing-texture 的核心逻辑 ----
    // 优先级：① 服务端 iconTexture(存在) → ② skill_scroll_<id>(存在) → ③ null(走文字标签)。

    @Test
    void resolvePrefersServerIconWhenItExists() {
        SkillBarEntry entry = SkillBarEntry.skill("woliu.heart", "涡心", 0, 0,
            "bong:textures/gui/skill/woliu_heart.png");
        // 服务端图存在 → 直接用它，不去拼 skill_scroll_。
        String resolved = LoadoutIconLayer.resolveExistingSkillTexture(
            entry, path -> path.equals("bong:textures/gui/skill/woliu_heart.png"));
        assertEquals("bong:textures/gui/skill/woliu_heart.png", resolved,
            "服务端 iconTexture 存在时优先返回它");
    }

    @Test
    void resolveFallsBackToScrollCandidateWhenServerIconMissing() {
        SkillBarEntry entry = SkillBarEntry.skill("woliu.heart", "涡心", 0, 0,
            "bong:textures/gui/skill/does_not_exist.png");
        // 服务端图不存在，但 skill_scroll_<id> 存在 → 用 id 候选路径（与查看界面一致）。
        String candidate = "bong-client:textures/gui/items/skill_scroll_woliu_heart.png";
        String resolved = LoadoutIconLayer.resolveExistingSkillTexture(
            entry, path -> path.equals(candidate));
        assertEquals(candidate, resolved,
            "服务端图缺失时回退到存在的 skill_scroll_<id> 候选");
    }

    @Test
    void resolveUsesScrollCandidateWhenServerIconBlank() {
        SkillBarEntry entry = SkillBarEntry.skill("woliu.heart", "涡心", 0, 0, "");
        String candidate = "bong-client:textures/gui/items/skill_scroll_woliu_heart.png";
        String resolved = LoadoutIconLayer.resolveExistingSkillTexture(entry, path -> path.equals(candidate));
        assertEquals(candidate, resolved,
            "服务端 iconTexture 为空时，直接看 skill_scroll_<id> 候选");
    }

    @Test
    void resolveReturnsNullWhenNothingExists() {
        SkillBarEntry entry = SkillBarEntry.skill("woliu.heart", "涡心", 0, 0,
            "bong:textures/gui/skill/missing.png");
        // 服务端图、id 候选都不存在 → null → 调用方走文字标签（绝不画紫黑）。
        String resolved = LoadoutIconLayer.resolveExistingSkillTexture(entry, path -> false);
        assertEquals(null, resolved, "都不存在时返回 null，交给文字兜底");
    }

    @Test
    void resolveReturnsNullForNullEntryOrPredicate() {
        assertEquals(null, LoadoutIconLayer.resolveExistingSkillTexture(null, path -> true),
            "entry 为 null → null");
        SkillBarEntry entry = SkillBarEntry.skill("woliu.heart", "涡心", 0, 0, "");
        assertEquals(null, LoadoutIconLayer.resolveExistingSkillTexture(entry, null),
            "谓词为 null → null（防 NPE）");
    }

    @Test
    void existenceAwareBuildEmitsTextureAndOverlayWhenExists() {
        SkillBarEntry entry = SkillBarEntry.skill("anqi.multi_shot", "多射", 0, 0, "");
        String candidate = "bong-client:textures/gui/items/skill_scroll_anqi_multi_shot.png";
        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(
            entry, 10, 20, 16, path -> path.equals(candidate));
        // 候选存在 → 画该贴图 + anqi 上下描边(2 rect)。
        assertEquals(3, commands.size(), "存在则出 1 贴图 + 2 anqi 描边");
        assertTrue(commands.stream().anyMatch(cmd -> cmd.isTexturedRect() && candidate.equals(cmd.texturePath())),
            "应画 id 候选贴图");
    }

    @Test
    void existenceAwareBuildEmptyWhenMissingSoTextFallbackKicksIn() {
        SkillBarEntry entry = SkillBarEntry.skill("woliu.heart", "涡心", 0, 0, "bad:path.png");
        List<HudRenderCommand> commands = LoadoutIconLayer.buildSkillIconCommands(
            entry, 10, 20, 16, path -> false);
        assertTrue(commands.isEmpty(), "贴图不存在 → 返回空 → 战斗栏走文字标签（不画紫黑）");
    }
}
