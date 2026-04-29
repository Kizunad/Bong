package com.bong.client.animation;

import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongAnimationRegistryInlineTest {
    private static final Identifier INLINE_ID = new Identifier("bong_test", "inline_pose");

    @AfterEach
    void tearDown() {
        BongAnimationRegistry.clearInlineForTest();
    }

    @Test
    void registersInlineJsonAsRuntimeAnimation() {
        assertTrue(BongAnimationRegistry.registerInlineJson(INLINE_ID, inlineJson("inline_pose", 4)));

        KeyframeAnimation animation = BongAnimationRegistry.get(INLINE_ID);
        assertNotNull(animation);
        assertEquals(4, animation.endTick);
        assertEquals(BongAnimationRegistry.Source.INLINE, BongAnimationRegistry.sourceOf(INLINE_ID));
    }

    @Test
    void invalidInlineJsonIsRejectedWithoutRegistering() {
        assertTrue(!BongAnimationRegistry.registerInlineJson(INLINE_ID, "not json"));

        assertNull(BongAnimationRegistry.get(INLINE_ID));
        assertEquals(BongAnimationRegistry.Source.NONE, BongAnimationRegistry.sourceOf(INLINE_ID));
    }

    @Test
    void repeatedInlineIdReplacesPreviousAnimation() {
        assertTrue(BongAnimationRegistry.registerInlineJson(INLINE_ID, inlineJson("inline_pose_a", 4)));
        assertEquals(4, BongAnimationRegistry.get(INLINE_ID).endTick);

        assertTrue(BongAnimationRegistry.registerInlineJson(INLINE_ID, inlineJson("inline_pose_b", 8)));

        assertEquals(8, BongAnimationRegistry.get(INLINE_ID).endTick);
    }

    private static String inlineJson(String name, int endTick) {
        return "{\"version\":3,\"name\":\"" + name + "\",\"emote\":{"
            + "\"beginTick\":0,\"endTick\":" + endTick + ",\"isLoop\":false,"
            + "\"moves\":["
            + "{\"tick\":0,\"rightArm\":{\"pitch\":-0.6},\"easing\":\"LINEAR\"},"
            + "{\"tick\":" + endTick + ",\"rightArm\":{\"pitch\":0.4},\"easing\":\"INOUTSINE\"}"
            + "]}}";
    }
}
