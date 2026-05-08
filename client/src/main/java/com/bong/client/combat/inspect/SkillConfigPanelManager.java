package com.bong.client.combat.inspect;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import com.bong.client.combat.SkillConfigStore;
import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import io.wispforest.owo.ui.container.FlowLayout;

import java.util.function.Consumer;

/** Owns the singleton floating SkillConfig editor in the techniques tab. */
public final class SkillConfigPanelManager {
    private final FlowLayout host;
    private final Runnable afterSave;
    private final Consumer<CastState> castListener = this::onCastStateChanged;
    private SkillConfigFloatingWindow activeWindow;
    private String activeSkillId = "";

    public SkillConfigPanelManager(FlowLayout host) {
        this(host, null);
    }

    public SkillConfigPanelManager(FlowLayout host, Runnable afterSave) {
        this.host = host;
        this.afterSave = afterSave;
        CastStateStore.addListener(castListener);
    }

    public boolean open(
        TechniquesListPanel.Technique technique,
        int anchorX,
        int anchorY,
        int screenWidth,
        int screenHeight
    ) {
        if (technique == null || CastStateStore.snapshot().isCasting()) return false;
        var schema = SkillConfigSchemaRegistry.schemaFor(technique.id()).orElse(null);
        if (schema == null) return false;

        close();
        activeSkillId = technique.id();
        JsonObject current = SkillConfigStore.configFor(technique.id());
        if (current == null) current = SkillConfigSchemaRegistry.defaultConfig(technique.id());
        activeWindow = new SkillConfigFloatingWindow(
            schema,
            current,
            config -> {
                SkillConfigStore.updateLocal(technique.id(), config);
                ClientRequestSender.sendSkillConfigIntent(technique.id(), config);
                if (afterSave != null) afterSave.run();
            },
            this::close
        );
        activeWindow.positionAt(anchorX, anchorY, screenWidth, screenHeight);
        host.clearChildren();
        host.child(activeWindow.component());
        return true;
    }

    public void close() {
        activeWindow = null;
        activeSkillId = "";
        host.clearChildren();
    }

    public void onSelectedTechniqueChanged(String selectedSkillId) {
        if (!isOpen()) return;
        if (selectedSkillId == null || !selectedSkillId.equals(activeSkillId)) close();
    }

    public boolean isOpen() {
        return !activeSkillId.isBlank();
    }

    public String activeSkillId() {
        return activeSkillId;
    }

    public SkillConfigFloatingWindow activeWindow() {
        return activeWindow;
    }

    public void dispose() {
        close();
        CastStateStore.removeListener(castListener);
    }

    private void onCastStateChanged(CastState state) {
        if (state != null && state.isCasting()) close();
    }

    void openHeadlessForTests(String skillId) {
        close();
        activeSkillId = skillId == null ? "" : skillId;
    }
}
