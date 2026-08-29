package com.bong.client.ui;

import com.bong.client.ui.bootstrap.UiBootstrapRegistry;
import com.bong.client.ui.bootstrap.UiRuntime;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ClientUiBootstrapTest {
    @Test
    void referenceModulesPreserveOrderAndRegisterExactlyOnce() {
        List<String> calls = new ArrayList<>();
        UiBootstrapRegistry registry = ClientUiBootstrap.referenceRegistry(
            () -> calls.add("screen_transition"),
            () -> calls.add("craft_screen")
        );
        UiRuntime runtime = new UiRuntime() {
        };

        registry.register("screen_transition", runtime);
        registry.register("screen_transition", runtime);
        registry.register("craft_screen", runtime);
        registry.register("craft_screen", runtime);

        assertEquals(List.of("screen_transition", "craft_screen"), calls,
            "分阶段触发不能重复注册 Fabric callback，且 Craft 必须晚于 Screen transition");
        assertEquals(List.of("screen_transition", "craft_screen"), registry.completedModuleIds());
    }

    @Test
    void craftRegistrationPullsInRequiredTransitionWhenCalledFirst() {
        List<String> calls = new ArrayList<>();
        UiBootstrapRegistry registry = ClientUiBootstrap.referenceRegistry(
            () -> calls.add("screen_transition"),
            () -> calls.add("craft_screen")
        );

        registry.register("craft_screen", new UiRuntime() {
        });

        assertEquals(List.of("screen_transition", "craft_screen"), calls);
        assertTrue(registry.isRegistered("screen_transition"));
        assertTrue(registry.isRegistered("craft_screen"));
    }
}
