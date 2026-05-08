package com.bong.client.combat.inspect;

import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillConfigStore;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillConfigPanelManagerTest {
    private SkillConfigPanelManager manager;

    @AfterEach
    void tearDown() {
        if (manager != null) manager.dispose();
        CastStateStore.resetForTests();
        SkillConfigStore.resetForTests();
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
}
