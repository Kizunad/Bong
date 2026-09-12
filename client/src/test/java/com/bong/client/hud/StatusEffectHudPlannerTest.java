package com.bong.client.hud;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.hud.svg.NanoSvgParser;
import com.bong.client.hud.svg.SvgTessellator;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class StatusEffectHudPlannerTest {
    @AfterEach void tearDown() { StatusEffectStore.resetForTests(); }

    @Test void iconsArriveSeriallyAndRefreshDoesNotReplayOrReorderThem() {
        StatusEffectStore.replace(List.of(effect("slowed", 30_000), effect("staminacrash", 30_000)), 0);
        commands(320, 0);
        assertEquals(List.of("slowed.png"), icons(commands(320, 200)).stream().map(c -> basename(c.texturePath())).toList());
        commands(320, 900);
        var arriving = icons(commands(320, 1_100));
        assertEquals(2, arriving.size());
        assertTrue(arriving.get(1).width() > arriving.get(0).width(), "第二项单独放大，第一项已落位");
        StatusEffectStore.replace(List.of(effect("staminacrash", 60_000), effect("slowed", 25_000)), 1_200);
        var settled = icons(commands(320, 2_000));
        assertEquals(List.of("slowed.png", "staminacrash.png"), settled.stream().map(c -> basename(c.texturePath())).toList());
        assertEquals(settled.get(0).width(), settled.get(1).width(), "续期不能重播入场");
        assertCentered(settled, 320);
        assertCentered(icons(commands(180, 2_100)), 180);
    }

    @Test void clearedAndExpiredWaitingEffectsNeverFlashOnScreen() {
        StatusEffectStore.replace(List.of(effect("slowed", 30_000), effect("frailty", 200), effect("staminacrash", 30_000)), 0);
        commands(320, 0);
        StatusEffectStore.replace(List.of(effect("slowed", 29_800), effect("frailty", 1)), 200);
        commands(320, 900);
        assertEquals(List.of("slowed.png"), icons(commands(320, 1_100)).stream().map(c -> basename(c.texturePath())).toList());
        StatusEffectStore.clearOnDisconnect();
        assertTrue(commands(320, 1_200).isEmpty(), "断线清除已入场、退场和排队状态");
    }

    @Test void lastFiveSecondsBlinkThenFadeOutWithoutNewPackets() {
        StatusEffectStore.replace(List.of(effect("slowed", 10_000)), 0);
        commands(320, 0);
        assertEquals(255, icons(commands(320, 4_900)).get(0).color() >>> 24);
        int first = icons(commands(320, 5_200)).get(0).color() >>> 24;
        int second = icons(commands(320, 5_525)).get(0).color() >>> 24;
        assertNotEquals(first, second, "最后 5 秒图标本身持续闪烁");
        commands(320, 10_000);
        assertFalse(icons(commands(320, 10_140)).isEmpty(), "到期保留短退场动画");
        assertTrue(commands(320, 10_281).isEmpty(), "无新快照也必须结束过期效果");
    }

    @Test void parameterizedEffectUsesItsPngAndUnknownEffectStillHasAnEmblem() {
        StatusEffectStore.replace(List.of(effect("body_part_resist:head", 30_000), effect("unrecognized", 30_000)), 0);
        commands(320, 0);
        commands(320, 900);
        var out = commands(320, 1_800);
        assertTrue(icons(out).stream().anyMatch(c -> c.texturePath().endsWith("body_part_resist.png")));
        assertTrue(out.stream().anyMatch(c -> c.isSvgRect() && c.svgAssetKey().equals("unknown")),
            "未识别状态必须有可见降级图案，不能请求不存在的 PNG");
    }

    @Test void statusArtworkLoadsThroughTheProductionParserAndPngDecoder() throws Exception {
        for (var asset : HudRenderRegistry.require(HudRenderLayer.STATUS_EFFECTS).svgAssets()) {
            String path = "/assets/" + asset.resource().getNamespace() + "/" + asset.resource().getPath();
            try (var input = getClass().getResourceAsStream(path)) {
                assertNotNull(input, "缺少已登记 SVG: " + path);
                assertTrue(new SvgTessellator().tessellate(new NanoSvgParser().parse(input)).triangleCount() > 0,
                    "SVG 必须由生产解析器生成可见几何: " + path);
            }
        }
        for (String id : List.of("bleeding", "contaminationboost", "immobilized", "shieldblocking",
                                "health_regen_boost", "exhausted")) {
            String path = "/assets/" + iconPath(id).replace(':', '/');
            try (var input = getClass().getResourceAsStream(path)) {
                assertNotNull(input, "状态必须包含自己的 PNG: " + id);
                var image = javax.imageio.ImageIO.read(input);
                assertNotNull(image, "PNG 必须可解码: " + id);
                assertTrue(image.getColorModel().hasAlpha(), "状态图标必须保留透明背景: " + id);
            }
        }
    }

    private static String iconPath(String id) {
        String path = StatusEffectHudPlanner.iconPathFor(id);
        assertNotNull(path, "缺少状态图标映射: " + id);
        return path;
    }

    private static StatusEffectStore.Effect effect(String id, long remaining) {
        return new StatusEffectStore.Effect(id, id, StatusEffectStore.Kind.DEBUFF, 1, remaining, 0xFFE07060, "", 0);
    }

    private static List<HudRenderCommand> commands(int width, long now) {
        return StatusEffectHudPlanner.buildCommands(width, 300, now, text -> text.length() * 6);
    }

    private static List<HudRenderCommand> icons(List<HudRenderCommand> commands) {
        return commands.stream().filter(HudRenderCommand::isTexturedRect).toList();
    }

    private static String basename(String path) { return path.substring(path.lastIndexOf('/') + 1); }

    private static void assertCentered(List<HudRenderCommand> icons, int width) {
        int left = icons.get(0).x();
        var last = icons.get(icons.size() - 1);
        assertEquals(width, left + last.x() + last.width(), 1, "整栏在不同逻辑宽度保持居中");
        assertTrue(left >= 0 && last.x() + last.width() <= width, "图标不能超出视口");
    }
}
