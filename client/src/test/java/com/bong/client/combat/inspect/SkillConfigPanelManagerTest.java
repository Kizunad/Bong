package com.bong.client.combat.inspect;

import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillConfigStore;
import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillConfigPanelManagerTest {
    private record Sent(String body) {}

    private static final class FakeWindow implements SkillConfigPanelManager.WindowHandle {
        private final JsonObject current;
        private final java.util.function.Consumer<JsonObject> onSave;
        private final Runnable onClose;
        private final io.wispforest.owo.ui.container.FlowLayout component =
            Containers.verticalFlow(Sizing.fixed(1), Sizing.fixed(1));

        FakeWindow(
            SkillConfigSchemaRegistry.SkillConfigSchema schema,
            JsonObject current,
            java.util.function.Consumer<JsonObject> onSave,
            Runnable onClose
        ) {
            this.current = current == null ? new JsonObject() : current.deepCopy();
            this.onSave = onSave;
            this.onClose = onClose;
        }

        @Override
        public io.wispforest.owo.ui.container.FlowLayout component() {
            return component;
        }

        @Override
        public void positionAt(int anchorX, int anchorY, int screenWidth, int screenHeight) {
        }

        void save() {
            if (onSave != null) onSave.accept(current.deepCopy());
            if (onClose != null) onClose.run();
        }
    }

    private SkillConfigPanelManager manager;
    private FakeWindow lastWindow;
    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void tearDown() {
        if (manager != null) manager.dispose();
        CastStateStore.resetForTests();
        SkillConfigStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
        lastWindow = null;
        sent.clear();
    }

    private void captureRequests() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(new String(payload, StandardCharsets.UTF_8)))
        );
    }

    private static TechniquesListPanel.Technique technique(String skillId) {
        return new TechniquesListPanel.Technique(
            skillId,
            skillId,
            TechniquesListPanel.Grade.MORTAL,
            0.0f,
            true,
            "",
            "",
            "",
            List.of(),
            0.0f,
            1,
            1,
            1.0f
        );
    }

    @Test
    void openingNewWindowReplacesPreviousSingleton() {
        manager = new SkillConfigPanelManager(Containers.verticalFlow(Sizing.fixed(1), Sizing.fixed(1)));

        manager.openHeadlessForTests("zhenmai.sever_chain");
        assertTrue(manager.isOpen());
        manager.openHeadlessForTests("burst_meridian.beng_quan");

        assertTrue(manager.isOpen());
        assertEquals("burst_meridian.beng_quan", manager.activeSkillId());
    }

    @Test
    void selectionChangeAndCastStartCloseWindow() {
        manager = new SkillConfigPanelManager(Containers.verticalFlow(Sizing.fixed(1), Sizing.fixed(1)));

        manager.openHeadlessForTests("zhenmai.sever_chain");
        manager.onSelectedTechniqueChanged("burst_meridian.beng_quan");
        assertFalse(manager.isOpen());

        manager.openHeadlessForTests("zhenmai.sever_chain");
        CastStateStore.beginSkillBarCast(0, 500, 1000L);
        assertFalse(manager.isOpen());
    }

    @Test
    void realOpenSaveUpdatesLocalStoreAndSendsIntent() {
        captureRequests();
        AtomicInteger saveCallbacks = new AtomicInteger();
        JsonObject current = new JsonObject();
        current.addProperty("meridian_id", "Pericardium");
        current.addProperty("backfire_kind", "array");
        SkillConfigStore.updateLocal("zhenmai.sever_chain", current);
        manager = new SkillConfigPanelManager(
            Containers.verticalFlow(Sizing.fixed(240), Sizing.fixed(180)),
            saveCallbacks::incrementAndGet,
            (schema, config, onSave, onClose) -> {
                lastWindow = new FakeWindow(schema, config, onSave, onClose);
                return lastWindow;
            }
        );

        assertTrue(manager.open(technique("zhenmai.sever_chain"), 200, 140, 320, 240));
        assertTrue(manager.isOpen());
        assertNotNull(manager.activeWindow());
        assertNotNull(lastWindow);

        lastWindow.save();

        assertFalse(manager.isOpen());
        assertEquals(1, saveCallbacks.get());
        assertEquals("array", SkillConfigStore.configFor("zhenmai.sever_chain").get("backfire_kind").getAsString());
        assertEquals(1, sent.size());
        assertEquals(
            "{\"type\":\"skill_config_intent\",\"v\":1,\"skill_id\":\"zhenmai.sever_chain\",\"config\":{\"meridian_id\":\"Pericardium\",\"backfire_kind\":\"array\"}}",
            sent.get(0).body()
        );
    }

    @Test
    void realOpenRejectsMissingSchemaAndActiveCast() {
        manager = new SkillConfigPanelManager(Containers.verticalFlow(Sizing.fixed(240), Sizing.fixed(180)));

        assertFalse(manager.open(technique("unknown.skill"), 0, 0, 320, 240));
        assertFalse(manager.isOpen());

        CastStateStore.beginSkillBarCast(0, 500, 1000L);
        assertFalse(manager.open(technique("zhenmai.sever_chain"), 0, 0, 320, 240));
        assertFalse(manager.isOpen());
    }
}
