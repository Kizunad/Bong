package com.bong.client.practice;

import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.inspect.TechniqueIntent;
import com.bong.client.combat.inspect.TechniquesListPanel.Technique;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import java.util.List;
import java.util.function.*;

/** 替换确认持有旧绑定；发送与权威快照确认分开，失效窗口不能再提交。 */
public final class PracticeBindingSession {
    public record Target(boolean dash, int slot) {
        public static Target combat(int slot) { return new Target(false, slot); }
        public static Target dashKey() { return new Target(true, -1); }
    }
    public record Pending(Target target, String expected, boolean awaiting, long sentAt) {}
    private final String id;
    private final Supplier<List<Technique>> techniques;
    private final Supplier<SkillBarConfig> slots;
    private final BooleanSupplier alive;
    private final Function<Technique, String> availability;
    private final UiIntentSink<TechniqueIntent> sink;
    private final LongSupplier clock;
    private Pending pending;
    private String message = "选择一个位置，已占用的位置会先确认替换";

    public PracticeBindingSession(String id, Supplier<List<Technique>> techniques, Supplier<SkillBarConfig> slots,
                                  BooleanSupplier alive, Function<Technique, String> availability,
                                  UiIntentSink<TechniqueIntent> sink, LongSupplier clock) {
        this.id = id; this.techniques = techniques; this.slots = slots; this.alive = alive;
        this.availability = availability; this.sink = sink; this.clock = clock;
    }
    public Pending pending() { return pending; }
    public String message() { return message; }
    public Technique technique() { return techniques.get().stream().filter(t -> t.id().equals(id)).findFirst().orElse(null); }
    public void choose(Target target) {
        if (pending != null && pending.awaiting()) return;
        String reason = validate(target);
        if (!reason.isBlank()) { pending = null; message = reason; return; }
        String current = token(slots.get(), target);
        if (current.equals("skill:" + id)) { pending = null; message = "此处已绑定该功法"; return; }
        pending = new Pending(target, current, false, 0);
        if (current.isEmpty()) submit();
        else message = "该位置已有绑定，请确认替换";
    }
    public void confirm() { if (pending != null && !pending.awaiting()) submit(); }
    public void cancel() { if (pending == null || !pending.awaiting()) { pending = null; message = "已取消替换"; } }
    private void submit() {
        var candidate = pending;
        String reason = validate(candidate.target());
        if (!reason.isBlank()) { pending = null; message = reason; return; }
        if (!token(slots.get(), candidate.target()).equals(candidate.expected())) {
            pending = null; message = "原绑定已变化，请重新选择"; return;
        }
        var result = sink.dispatch(new TechniqueIntent.BindChecked(candidate.target().dash(), candidate.target().slot(), id, candidate.expected()));
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            pending = new Pending(candidate.target(), candidate.expected(), true, clock.getAsLong());
            message = "已发送，等待绑定同步…";
        } else { pending = null; message = result.reason(); }
    }
    public void refresh() {
        if (pending == null) return;
        if (!alive.getAsBoolean() || technique() == null) { pending = null; message = "功法或窗口已失效"; return; }
        String current = token(slots.get(), pending.target());
        if (pending.awaiting() && current.equals("skill:" + id)) { pending = null; message = "绑定已同步"; }
        else if (!current.equals(pending.expected())) { pending = null; message = "绑定发生变化，请重新选择"; }
        else if (pending.awaiting() && clock.getAsLong() - pending.sentAt() > 5000) {
            pending = null; message = "未收到绑定确认，请核对当前状态后重试";
        }
    }
    private String validate(Target target) {
        if (!alive.getAsBoolean()) return "窗口已关闭";
        var t = technique();
        if (t == null) return "此功法已不在所学之中";
        if (!t.active()) return "功法尚未激活";
        if (target.dash() && !t.inputKind().equals("dash")) return "只有闪避功法可绑定闪避键";
        if (!target.dash() && (!SkillBarConfig.isAvailable(target.slot()) || t.inputKind().equals("dedicated"))) return "此位置不支持该功法";
        return availability.apply(t);
    }
    public static String token(SkillBarConfig slots, Target target) {
        if (target.dash()) return "skill:" + slots.dashSkillId();
        var entry = slots.slot(target.slot());
        return entry == null ? "" : (entry.kind() == SkillBarEntry.Kind.SKILL ? "skill:" : "item:") + entry.id();
    }
}
