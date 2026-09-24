package com.bong.client.practice;

import com.bong.client.combat.inspect.TechniquesListPanel;
import io.wispforest.owo.ui.container.FlowLayout;
import java.util.List;

/** 使用两份当前功法快照对照，数值口径与详情一致。 */
public final class PracticeCompareContent {
    private final FlowLayout root = PracticeStyle.column();
    private final String identity;
    private Object previous;
    public PracticeCompareContent(String identity) { this.identity = identity; refresh(); }
    public FlowLayout component() { return root; }
    public void refresh() {
        var ids = identity.split("\\|", -1);
        var entries = java.util.Arrays.stream(ids).map(id -> TechniquesListPanel.snapshot().stream()
            .filter(t -> t.id().equals(id) && t.inputKind().equals("dash")).findFirst().orElse(null)).toList();
        var reasons = entries.stream().map(TechniqueAvailability::reason).toList();
        var next = List.of(entries, reasons);
        if (next.equals(previous)) return;
        root.clearChildren();
        if (entries.size() != 2 || entries.contains(null)) {
            root.child(PracticeStyle.label("对比功法已失效", PracticeStyle.MUTED));
            previous = next;
            return;
        }
        var left = entries.get(0); var right = entries.get(1);
        root.child(PracticeStyle.value("原绑定", left.displayName()));
        root.child(PracticeStyle.value("候选", right.displayName()));
        root.child(PracticeStyle.value("品阶", left.grade().label() + "  /  " + right.grade().label()));
        root.child(PracticeStyle.value("熟练阶段", left.proficiencyLabel() + "  /  " + right.proficiencyLabel()));
        root.child(PracticeStyle.value("体力消耗", pair(left.staminaCost(), right.staminaCost())));
        root.child(PracticeStyle.value("真元消耗", pair(left.qiCost(), right.qiCost())));
        root.child(PracticeStyle.value("基础冷却 / 秒", pair(left.cooldownTicks() / 20.0, right.cooldownTicks() / 20.0)));
        root.child(PracticeStyle.value("基础位移 / 格", pair(left.range(), right.range())));
        for (var t : entries) {
            var section = PracticeStyle.section(t.displayName());
            section.child(new PracticeStyle.Progress(t.proficiency(), t.proficiencyLabel()));
            section.child(PracticeStyle.label(t.description(), PracticeStyle.TEXT));
            section.child(PracticeStyle.value("状态", TechniqueAvailability.reason(t).isEmpty() ? "可用" : TechniqueAvailability.reason(t)));
            root.child(section);
        }
        previous = next;
    }
    private static String pair(double left, double right) { return PracticeDetailContent.number(left) + "  /  " + PracticeDetailContent.number(right); }
}
