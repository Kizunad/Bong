package com.bong.client.practice;

import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.combat.inspect.SkillConfigSchemaRegistry;
import com.bong.client.inventory.model.MeridianChannel;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.util.RealmLabel;
import io.wispforest.owo.ui.container.FlowLayout;
import java.util.List;
import java.util.Locale;
import java.util.function.Consumer;

/** 每个详情只持有目录 identity；经验、状态、参数始终由当前权威快照生成。 */
public final class PracticeDetailContent {
    private final FlowLayout root = PracticeStyle.column();
    private final String key;
    private final Consumer<String> search, bind;
    private final Consumer<MeridianChannel> meridian;
    private final Consumer<TechniquesListPanel.Technique> configure;
    private Object previous;

    public PracticeDetailContent(String key, Consumer<String> search, Consumer<MeridianChannel> meridian,
                                 Consumer<String> bind, Consumer<TechniquesListPanel.Technique> configure) {
        this.key = key; this.search = search; this.meridian = meridian; this.bind = bind; this.configure = configure;
        refresh();
    }
    public FlowLayout component() { return root; }
    public void refresh() {
        var entry = PracticeCatalog.entries(TechniquesListPanel.snapshot(), SkillSetStore.snapshot()).stream()
            .filter(value -> value.key().equals(key)).findFirst().orElse(null);
        if (entry == null) {
            if (!"missing".equals(previous)) {
                root.clearChildren();
                root.child(PracticeStyle.label("此项已不在所学之中", PracticeStyle.MUTED));
                previous = "missing";
            }
            return;
        }
        List<com.bong.client.skill.SkillRecentEventStore.Entry> events = entry.skill() == null ? List.of() : SkillExperienceView.recentEventsForSkill(entry.skill());
        List<com.bong.client.skill.SkillMilestoneSnapshot> milestones = entry.skill() == null ? List.of() : SkillExperienceView.recentMilestonesForSkill(entry.skill());
        String reason = entry.isTechnique() ? TechniqueAvailability.reason(entry.technique()) : "";
        int slot = entry.isTechnique() ? SkillBarStore.findSkill(entry.technique().id()) : -1;
        boolean dashBound = entry.isTechnique() && entry.technique().id().equals(SkillBarStore.snapshot().dashSkillId());
        String dashKey = com.bong.client.movement.MovementKeybindings.dashKeyLabel();
        var next = List.of(entry, events, milestones, reason, slot, dashBound, dashKey);
        if (next.equals(previous)) return;
        root.clearChildren();
        root.child(PracticeStyle.label(entry.name(), PracticeStyle.GOLD));
        var tags = PracticeStyle.column();
        for (String tag : entry.tags()) {
            // 经脉需求在下方以模型链接呈现，普通标签才回到目录。
            if (TechniquesListPanel.channelFromWire(tag).isPresent()) continue;
            var link = PracticeStyle.link("#" + tag + "  ›", () -> search.accept("#" + tag));
            link.id("practice-tag-" + tag);
            tags.child(link);
        }
        root.child(tags);
        if (entry.isTechnique()) {
            var t = entry.technique();
            root.child(PracticeStyle.label(t.description(), PracticeStyle.TEXT));
            var progress = PracticeStyle.section("行功熟练");
            progress.child(new PracticeStyle.Progress(entry.progress(), entry.stage() + "  ·  " + Math.round(entry.progress() * 100) + "%"));
            progress.child(PracticeStyle.label("生疏 → 入门 → 熟练 → 精通 → 化境", PracticeStyle.MUTED));
            root.child(progress);
            var requirements = PracticeStyle.section("修习需求");
            requirements.child(PracticeStyle.value("境界", RealmLabel.displayName(t.requiredRealm())));
            if (t.requiredMeridians().isEmpty()) requirements.child(PracticeStyle.label("无指定经脉需求", PracticeStyle.MUTED));
            for (var required : t.requiredMeridians()) {
                var channel = TechniquesListPanel.channelFromWire(required.channel());
                if (channel.isPresent()) {
                    var link = PracticeStyle.link(channel.get().displayName() + "  ·  健康 ≥ " + Math.round(required.minHealth() * 100) + "%  ›",
                        () -> meridian.accept(channel.get()));
                    link.id("practice-meridian-" + channel.get().name());
                    requirements.child(link);
                } else requirements.child(PracticeStyle.label(required.channel(), PracticeStyle.MUTED));
            }
            root.child(requirements);
            var parameters = PracticeStyle.section("行功参数");
            parameters.child(PracticeStyle.value("真元消耗", number(t.qiCost())));
            parameters.child(PracticeStyle.value("体力消耗", number(t.staminaCost())));
            parameters.child(PracticeStyle.value("施放", number(t.castTicks() / 20.0) + " 秒"));
            parameters.child(PracticeStyle.value("基础冷却", number(t.cooldownTicks() / 20.0) + " 秒"));
            parameters.child(PracticeStyle.value("基础范围", number(t.range()) + " 格"));
            parameters.child(PracticeStyle.value("状态", reason.isEmpty() ? "可用" : reason));
            parameters.child(PracticeStyle.value("战斗槽", slot < 0 ? "未绑定" : Integer.toString(slot + 1)));
            if (t.inputKind().equals("dash")) parameters.child(PracticeStyle.value("闪避键", dashBound ? dashKey : "未绑定"));
            root.child(parameters);
            if (SkillConfigSchemaRegistry.hasSchema(t.id())) {
                var config = PracticeStyle.button("行功配置", () -> configure.accept(t));
                config.id("practice-config"); root.child(config);
            }
            var button = PracticeStyle.button("快捷绑定", () -> bind.accept(t.id()));
            button.id("practice-bind"); root.child(button);
        } else {
            var xp = entry.experience();
            var progress = PracticeStyle.section(PracticeCatalog.experienceName(entry.skill()));
            progress.child(new PracticeStyle.Progress(entry.progress(), xp.lv() == 10 ? "已满级" : xp.xp() + " / " + xp.xpToNext()));
            progress.child(PracticeStyle.value("等级", "Lv." + xp.lv()));
            progress.child(PracticeStyle.value("实际生效", "Lv." + xp.effectiveLv()));
            progress.child(PracticeStyle.value("境界上限", "Lv." + xp.cap()));
            progress.child(PracticeStyle.value("累计经验", Long.toString(xp.totalXp())));
            progress.child(PracticeStyle.value("状态", xp.lv() > xp.cap() ? "境界压制" : xp.lv() == 10 ? "已满级" : "持续积累"));
            root.child(progress);
            root.child(PracticeStyle.label(SkillExperienceView.formatSkillCurrentEffect(entry.skill(), xp), PracticeStyle.TEXT));
            root.child(PracticeStyle.label(SkillExperienceView.formatSkillNextEffect(entry.skill(), xp), PracticeStyle.MUTED));
            var history = PracticeStyle.section("修习留痕");
            events.forEach(event -> history.child(PracticeStyle.label(SkillExperienceView.formatSkillRecentEventLine(event), PracticeStyle.TEXT)));
            milestones.forEach(event -> history.child(PracticeStyle.label(SkillExperienceView.formatSkillMilestoneLine(event), PracticeStyle.GOLD)));
            if (events.isEmpty() && milestones.isEmpty()) history.child(PracticeStyle.label("尚无近期记录", PracticeStyle.MUTED));
            root.child(history);
        }
        previous = next;
    }
    static String number(double value) { return String.format(Locale.ROOT, "%.1f", value); }
}
