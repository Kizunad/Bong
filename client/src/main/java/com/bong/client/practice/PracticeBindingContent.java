package com.bong.client.practice;

import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.combat.inspect.TechniqueClientIntentSink;
import com.bong.client.movement.MovementKeybindings;
import com.bong.client.ui.window.UiWindowManager;
import io.wispforest.owo.ui.container.FlowLayout;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BiConsumer;

public final class PracticeBindingContent {
    private final FlowLayout root = PracticeStyle.column();
    private final PracticeBindingSession session;
    private final BiConsumer<String, String> compare;
    private Object previous;

    public PracticeBindingContent(UiWindowManager.WindowState owner, BiConsumer<String, String> compare) {
        this.compare = compare;
        session = new PracticeBindingSession(owner.key().identity(), TechniquesListPanel::snapshot, SkillBarStore::snapshot,
            () -> !owner.closed(), TechniqueAvailability::reason, new TechniqueClientIntentSink(), System::currentTimeMillis);
        refresh();
    }
    public FlowLayout component() { return root; }
    public void refresh() {
        session.refresh();
        var technique = session.technique();
        var config = SkillBarStore.snapshot();
        var tokens = new ArrayList<String>();
        for (int slot = 0; slot < SkillBarConfig.SLOT_COUNT; slot++) tokens.add(PracticeBindingSession.token(config, PracticeBindingSession.Target.combat(slot)));
        var next = List.of(technique == null ? "" : technique, tokens, config.dashSkillId(), session.message(),
            session.pending() == null ? "" : session.pending(), MovementKeybindings.dashKeyLabel());
        if (next.equals(previous)) return;
        previous = next;
        root.clearChildren();
        if (technique == null) { root.child(PracticeStyle.label("功法已失效", PracticeStyle.MUTED)); return; }
        root.child(PracticeStyle.label(technique.displayName(), PracticeStyle.GOLD));
        root.child(PracticeStyle.label(session.message(), PracticeStyle.MUTED));
        if (technique.inputKind().equals("dash")) {
            var dash = PracticeStyle.section("闪避键");
            dash.child(PracticeStyle.value("当前功法", name(config.dashSkillId())));
            var button = PracticeStyle.button("绑定到闪避键 [ " + MovementKeybindings.dashKeyLabel() + " ]",
                () -> session.choose(PracticeBindingSession.Target.dashKey()));
            button.id("bind-dash");
            button.active(session.pending() == null || !session.pending().awaiting());
            dash.child(button);
            root.child(dash);
        }
        var grid = PracticeStyle.section("战斗技能");
        for (int index = 0; index < SkillBarConfig.SLOT_COUNT; index++) {
            int slot = index;
            var entry = config.slot(slot);
            var button = PracticeStyle.button("[ " + (slot + 1) + " ]  " + (entry == null ? "空位" : entry.displayName()),
                () -> session.choose(PracticeBindingSession.Target.combat(slot)));
            button.id("bind-combat-" + slot);
            button.active(session.pending() == null || !session.pending().awaiting());
            grid.child(button);
        }
        root.child(grid);
        var pending = session.pending();
        if (pending != null && !pending.awaiting()) {
            var confirmation = PracticeStyle.section("确认替换？");
            String old = pending.expected();
            confirmation.child(PracticeStyle.value("原绑定", old.startsWith("skill:") ? name(old.substring(6)) : old.substring(old.indexOf(':') + 1)));
            confirmation.child(PracticeStyle.value("新绑定", technique.displayName()));
            if (old.startsWith("skill:") && technique.inputKind().equals("dash")) {
                String oldId = old.substring(6);
                TechniquesListPanel.snapshot().stream().filter(t -> t.id().equals(oldId) && t.inputKind().equals("dash"))
                    .findFirst().ifPresent(t -> {
                        var button = PracticeStyle.button("对比", () -> compare.accept(oldId, technique.id()));
                        button.id("bind-compare"); confirmation.child(button);
                    });
            }
            var confirm = PracticeStyle.button("确认替换", session::confirm); confirm.id("bind-confirm"); confirmation.child(confirm);
            var cancel = PracticeStyle.button("保留原绑定", session::cancel); cancel.id("bind-cancel"); confirmation.child(cancel);
            root.child(confirmation);
        }
    }
    private static String name(String id) {
        return TechniquesListPanel.snapshot().stream().filter(t -> t.id().equals(id)).map(TechniquesListPanel.Technique::displayName)
            .findFirst().orElse("未习得");
    }
}
