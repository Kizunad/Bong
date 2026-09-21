package com.bong.client.fauna;

import org.junit.jupiter.api.Test;
import com.google.gson.JsonParser;
import software.bernie.geckolib.loading.json.raw.Model;
import software.bernie.geckolib.loading.object.BakedAnimations;
import software.bernie.geckolib.util.JsonUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;

import static org.junit.jupiter.api.Assertions.*;

class FaunaPlaybackTest {
    @Test
    void horseUsesItsFourGaitsAndActionTemporarilyOverridesThem() {
        var state = new FaunaPlayback(FaunaVisualKind.HORSE);
        float[] speeds = {0, 0.03f, 0.12f, 0.2f, 0.3f};
        String[] gaits = {"idle", "walk", "trot", "canter", "gallop"};
        for (int i = 0; i < speeds.length; i++) {
            assertEquals(gaits[i], FaunaAnimations.shortName(state.current(speeds[i]).name()));
        }
        state.trigger("kick", 1);
        assertEquals("kick", FaunaAnimations.shortName(state.current(0.3f).name()));
        state.tick();
        assertEquals("gallop", FaunaAnimations.shortName(state.current(0.3f).name()));
    }

    @Test
    void everyRuntimeProfileLoadsWithGeckoLibAndAllTracksBindToRealBones() throws Exception {
        Path root = Path.of("src/main/resources/assets/bong");
        var checked = new HashSet<String>();
        for (var kind : FaunaVisualKind.values()) {
            for (var profile : FaunaAnimations.profiles(kind)) {
                if (!checked.add(profile.path())) continue;
                var geometry = JsonParser.parseString(Files.readString(root.resolve("geo/" + profile.path() + ".geo.json")));
                Model parsed = JsonUtil.GEO_GSON.fromJson(geometry, Model.class);
                assertEquals(1, parsed.minecraftGeometry().length);
                var bones = new HashSet<String>();
                for (var bone : geometry.getAsJsonObject().getAsJsonArray("minecraft:geometry").get(0)
                    .getAsJsonObject().getAsJsonArray("bones")) {
                    assertTrue(bones.add(bone.getAsJsonObject().get("name").getAsString()), "重复骨名：" + profile.path());
                }
                var clips = JsonParser.parseString(Files.readString(root.resolve("animations/" + profile.path() + ".animation.json")))
                    .getAsJsonObject().getAsJsonObject("animations");
                BakedAnimations baked = JsonUtil.GEO_GSON.fromJson(clips, BakedAnimations.class);
                assertEquals(clips.size(), baked.animations().size(), "GeckoLib 不得静默丢弃动作：" + profile.path());
                for (var clip : clips.entrySet()) {
                    var tracks = clip.getValue().getAsJsonObject().getAsJsonObject("bones");
                    if (tracks != null) assertTrue(bones.containsAll(tracks.keySet()), clip.getKey() + " 存在无法绑定的骨轨道");
                }
                assertTrue(Files.isRegularFile(root.resolve("textures/entity/fauna/" + profile.path() + ".png")));
            }
        }
    }

    @Test
    void shapeAndAnimationSwitchTogetherAndUnknownActionsDoNotInterrupt() {
        var state = new FaunaPlayback(FaunaVisualKind.HYBRID_BEAST);
        assertTrue(state.trigger("animation.bong.hybrid_beast_core.core_split", 2));
        assertEquals("hybrid_beast_core", state.profile().path());
        assertFalse(state.trigger("animation.bong.dainu_lion.bite", 40));
        assertEquals("core_split", FaunaAnimations.shortName(state.current(0).name()));
        state.tick();
        state.tick();
        assertEquals("core_crawl", FaunaAnimations.shortName(state.current(0.1f).name()));
        assertTrue(state.trigger("animation.bong.hybrid_beast_shard.shard_crawl", 20));
        assertEquals("hybrid_beast_shard", state.profile().path());
    }

    @Test
    void repeatedAttackRestartsButInvalidInputPreservesCurrentAction() {
        var state = new FaunaPlayback(FaunaVisualKind.DAINU_LION);
        assertTrue(state.play("bite"));
        state.tick();
        int revision = state.revision();
        assertFalse(state.trigger("missing", 10));
        assertEquals(revision, state.revision());
        assertTrue(state.play("bite"));
        assertTrue(state.revision() > revision, "同名连续攻击需要通知控制器重启时间轴");
        assertEquals(state.profile().find("bite").ticks(), state.remainingTicks());
    }

    @Test
    void spiderFullSyncIsStillAndDisguiseWaitsForFoldToFinish() {
        var state = new FaunaPlayback(FaunaVisualKind.ASH_SPIDER);
        state.spiderDisguise(true, false);
        assertTrue(state.blockDisguise());
        assertNull(state.actionName(), "进视距全量同步不该播放暴起");
        state.spiderDisguise(false, true);
        assertFalse(state.blockDisguise());
        assertEquals("ambush_burst", FaunaAnimations.shortName(state.actionName()));
        state.spiderDisguise(true, false);
        assertEquals("fold", FaunaAnimations.shortName(state.actionName()));
        int duration = state.remainingTicks();
        for (int i = 1; i < duration; i++) state.tick();
        assertFalse(state.blockDisguise(), "折叠结束前应继续显示蜘蛛几何");
        state.tick();
        assertTrue(state.blockDisguise());
        state.spiderDisguise(true, false);
        assertNull(state.actionName(), "周期 full sync 不重播折叠");
    }

    @Test
    void vultureUnfoldAndLandingReturnToTheCorrectBindingPose() {
        var state = new FaunaPlayback(FaunaVisualKind.FUYU_VULTURE);
        state.trigger("unfold", 1);
        state.tick();
        assertEquals("fuyu_vulture_flight", state.profile().path());
        assertEquals("glide", FaunaAnimations.shortName(state.current(0).name()));
        state.trigger("land", 1);
        state.tick();
        assertEquals("fuyu_vulture", state.profile().path());
        assertEquals("idle", FaunaAnimations.shortName(state.current(0).name()));
    }
}
