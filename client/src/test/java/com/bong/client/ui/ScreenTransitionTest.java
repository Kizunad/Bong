package com.bong.client.ui;

import com.bong.client.alchemy.AlchemyScreen;
import com.bong.client.forge.ForgeScreen;
import com.bong.client.inspect.ItemInspectScreen;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.model.InventoryModel;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.InventoryScreen;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ScreenTransitionTest {
    @AfterEach
    void resetRegistry() {
        ScreenTransitionRegistry.resetForTests();
        UiTransitionSettings.resetForTests();
    }

    @Test
    void transition_slide_up_position() {
        ScreenTransition.Frame start = ScreenTransition.sample(
            ScreenTransition.Type.SLIDE_UP,
            300,
            ScreenTransition.Easing.LINEAR,
            1_000L,
            1_000L,
            800,
            600
        );
        ScreenTransition.Frame middle = ScreenTransition.sample(
            ScreenTransition.Type.SLIDE_UP,
            300,
            ScreenTransition.Easing.LINEAR,
            1_000L,
            1_150L,
            800,
            600
        );

        assertEquals(600, start.offsetY());
        assertEquals(300, middle.offsetY());
        assertTrue(middle.inputLocked());
    }

    @Test
    void transition_fade_alpha() {
        ScreenTransition.Frame frame = ScreenTransition.sample(
            ScreenTransition.Type.FADE,
            200,
            ScreenTransition.Easing.LINEAR,
            0L,
            100L,
            320,
            180
        );

        assertEquals(0.5, frame.newAlpha(), 0.001);
        assertEquals(0.5, frame.oldAlpha(), 0.001);
        assertFalse(frame.finished());
    }

    @Test
    void rapid_switch_cancels_previous() {
        ScreenTransition.TransitionHandle first = ScreenTransition.play(
            new DummyScreen("old"),
            new DummyScreen("first"),
            ScreenTransition.Type.FADE,
            200,
            ScreenTransition.Easing.LINEAR,
            () -> {
            }
        );
        first.cancel();
        ScreenTransition.TransitionHandle second = ScreenTransition.play(
            first.oldScreen(),
            new DummyScreen("second"),
            ScreenTransition.Type.SLIDE_RIGHT,
            200,
            ScreenTransition.Easing.LINEAR,
            () -> {
            }
        );

        assertTrue(first.cancelled());
        assertFalse(second.cancelled());
        assertEquals(ScreenTransition.Type.SLIDE_RIGHT, second.type());
    }

    @Test
    void null_type_completes_immediately() {
        final boolean[] called = {false};
        ScreenTransition.TransitionHandle handle = ScreenTransition.play(
            new DummyScreen("old"),
            new DummyScreen("new"),
            null,
            200,
            ScreenTransition.Easing.LINEAR,
            () -> called[0] = true
        );

        assertTrue(handle.completed());
        assertTrue(called[0]);
        assertEquals(ScreenTransition.Type.NONE, handle.type());
    }

    @Test
    void unregistered_screen_uses_default() {
        TransitionConfig config = ScreenTransitionRegistry.getOrDefault(DummyScreen.class);

        assertEquals(ScreenTransition.Type.FADE, config.openTransition());
        assertEquals(200, config.openDurationMs());
    }

    @Test
    void inventory_uses_slide_up() {
        ScreenTransitionRegistry.bootstrapDefaults();

        TransitionConfig config = ScreenTransitionRegistry.getOrDefault(InspectScreen.class);

        assertEquals(ScreenTransition.Type.SLIDE_UP, config.openTransition());
        assertEquals(ScreenTransition.Type.SLIDE_DOWN, config.closeTransition());
        assertEquals(300, config.openDurationMs());
    }

    @Test
    void vanilla_inventory_is_rerouted_to_inspect_transition_surface() {
        ScreenTransitionRegistry.bootstrapDefaults();

        assertTrue(ScreenTransitionRegistry.get(InventoryScreen.class).isEmpty());
        TransitionConfig.TransitionSpec spec = ScreenTransitionRegistry.preview(
            null,
            new InspectScreen(InventoryModel.empty())
        );

        assertEquals(ScreenTransition.Type.SLIDE_UP, spec.type());
        assertEquals(300, spec.durationMs());
    }

    @Test
    void esc_menu_fastest() {
        ScreenTransitionRegistry.bootstrapDefaults();

        TransitionConfig config = ScreenTransitionRegistry.getOrDefault(GameMenuScreen.class);

        assertEquals(ScreenTransition.Type.FADE, config.openTransition());
        assertEquals(150, config.openDurationMs());
    }

    @Test
    void cultivation_slowest() {
        ScreenTransitionRegistry.bootstrapDefaults();

        TransitionConfig config = ScreenTransitionRegistry.getOrDefault(CultivationScreen.class);

        assertEquals(ScreenTransition.Type.FADE, config.openTransition());
        assertEquals(600, config.openDurationMs());
        assertEquals(TransitionConfig.OverlayStyle.VIGNETTE, config.overlayStyle());
    }

    @Test
    void eight_screen_defaults_cover_core_surfaces() {
        ScreenTransitionRegistry.bootstrapDefaults();

        assertTrue(ScreenTransitionRegistry.get(InspectScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(ForgeScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(AlchemyScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(CultivationScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(ItemInspectScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(GameMenuScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(com.bong.client.social.SparringInviteScreen.class).isPresent());
        assertTrue(ScreenTransitionRegistry.get(com.bong.client.social.TradeOfferScreen.class).isPresent());
    }

    @Test
    void npc_dialogue_to_trade_slide() {
        ScreenTransitionRegistry.bootstrapDefaults();

        TransitionConfig.TransitionSpec spec = ScreenTransitionRegistry.resolve(
            new DummyNpcDialogueScreen(),
            new DummyNpcTradeScreen()
        );

        assertEquals(ScreenTransition.Type.FADE, spec.type());

        ScreenTransitionRegistry.register(DummyNpcTradeScreen.class, TransitionConfig.of(
            DummyNpcTradeScreen.class,
            ScreenTransition.Type.SLIDE_RIGHT,
            200,
            ScreenTransition.Type.FADE,
            200
        ));
        ScreenTransitionRegistry.registerChain(
            DummyNpcDialogueScreen.class,
            DummyNpcTradeScreen.class,
            new TransitionConfig.TransitionSpec(
                ScreenTransition.Type.SLIDE_RIGHT,
                200,
                ScreenTransition.Easing.EASE_OUT_CUBIC,
                TransitionConfig.OverlayStyle.NONE,
                false
            )
        );

        TransitionConfig.TransitionSpec chained = ScreenTransitionRegistry.resolve(
            new DummyNpcDialogueScreen(),
            new DummyNpcTradeScreen()
        );
        assertEquals(ScreenTransition.Type.SLIDE_RIGHT, chained.type());
        assertEquals(200, chained.durationMs());
    }

    @Test
    void input_locked_during_transition_and_released_when_done() {
        ScreenTransition.Frame active = ScreenTransition.sample(
            ScreenTransition.Type.SCALE_UP,
            400,
            ScreenTransition.Easing.LINEAR,
            1_000L,
            1_100L,
            800,
            600
        );
        ScreenTransition.Frame done = ScreenTransition.sample(
            ScreenTransition.Type.SCALE_UP,
            400,
            ScreenTransition.Easing.LINEAR,
            1_000L,
            1_400L,
            800,
            600
        );

        assertTrue(active.inputLocked());
        assertTrue(done.finished());
        assertFalse(done.inputLocked());
    }

    @Test
    void input_policy_consumes_mouse_and_keeps_esc_escape_hatch() {
        ScreenTransition.Frame active = ScreenTransition.sample(
            ScreenTransition.Type.FADE,
            200,
            ScreenTransition.Easing.LINEAR,
            0L,
            50L,
            1,
            1
        );

        assertTrue(active.inputLocked());
        assertTrue(TransitionInputPolicy.shouldBlockMouse(active.inputLocked()));
        assertEquals(
            TransitionInputPolicy.KeyDecision.CANCEL_AND_CLOSE,
            TransitionInputPolicy.keyDecision(active.inputLocked(), GLFW.GLFW_KEY_ESCAPE, GLFW.GLFW_PRESS)
        );
        assertEquals(
            TransitionInputPolicy.KeyDecision.CONSUME,
            TransitionInputPolicy.keyDecision(active.inputLocked(), GLFW.GLFW_KEY_A, GLFW.GLFW_PRESS)
        );
    }

    private static class DummyScreen extends Screen {
        DummyScreen(String title) {
            super(Text.literal(title));
        }
    }

    private static final class DummyNpcDialogueScreen extends DummyScreen {
        DummyNpcDialogueScreen() {
            super("dialogue");
        }
    }

    private static final class DummyNpcTradeScreen extends DummyScreen {
        DummyNpcTradeScreen() {
            super("trade");
        }
    }
}
