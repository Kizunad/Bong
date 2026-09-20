package com.bong.client.practice;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.combat.inspect.TechniquesListPanel.Technique;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;

/** 修习目录的不可变投影；标签来自功法元数据及真实技艺种类，不由名称猜测。 */
public final class PracticeCatalog {
    private PracticeCatalog() {}

    public record Entry(String key, String name, List<String> tags, String searchable,
                        double progress, String stage, Technique technique, SkillId skill,
                        SkillSetSnapshot.Entry experience) {
        public Entry { tags = List.copyOf(tags); }
        public boolean isTechnique() { return technique != null; }
    }

    public static List<Entry> entries(List<Technique> techniques, SkillSetSnapshot skills) {
        var entries = new ArrayList<Entry>();
        for (var t : techniques) {
            var tags = new LinkedHashSet<String>();
            tags.add("功法");
            tags.add(t.grade().label());
            String category = category(t.category());
            if (t.inputKind().equals("dash")) tags.add("闪避");
            else if (!category.isBlank()) tags.add(category);
            if (!t.requiredRealm().isBlank()) tags.add(com.bong.client.util.RealmLabel.displayName(t.requiredRealm()));
            for (var required : t.requiredMeridians()) {
                TechniquesListPanel.channelFromWire(required.channel())
                    .ifPresent(channel -> tags.add(channel.displayName()));
            }
            entries.add(new Entry("technique:" + t.id(), t.displayName(), List.copyOf(tags),
                normalize(t.id() + " " + t.displayName() + " " + String.join(" ", t.aliases())
                    + " " + t.description() + " " + String.join(" ", tags)),
                t.proficiency(), t.proficiencyLabel(), t, null, null));
        }
        for (var id : SkillId.values()) {
            var xp = skills.get(id);
            var tags = List.of("技艺", id.displayName(), experienceName(id));
            entries.add(new Entry("skill:" + id.wireId(), id.displayName(), tags,
                normalize(id.wireId() + " " + String.join(" ", tags)),
                xp.lv() == 10 ? 1 : xp.progressRatio(), "Lv." + xp.lv(), null, id, xp));
        }
        return List.copyOf(entries);
    }

    /** 多个标签和普通词都取交集；标签精确匹配，正文词可匹配名称、别名、说明。 */
    public static List<Entry> filter(List<Entry> entries, String query) {
        var terms = normalize(query).replace("#", " #").strip().split("\\s+");
        return entries.stream().filter(entry -> {
            for (var term : terms) {
                if (term.isBlank() || term.equals("#")) continue;
                if (term.startsWith("#")) {
                    String tag = term.substring(1);
                    if (entry.tags().stream().noneMatch(value -> normalize(value).equals(tag))) return false;
                } else if (!entry.searchable().contains(term)) return false;
            }
            return true;
        }).toList();
    }

    public static String experienceName(SkillId id) {
        return switch (id) {
            case COMBAT -> "实战经验";
            case CULTIVATION -> "修仙经验";
            default -> id.displayName() + "经验";
        };
    }

    private static String category(String wire) {
        return switch (wire) {
            case "attack" -> "攻击";
            case "heal" -> "治疗";
            case "buff" -> "增益";
            case "control" -> "控制";
            case "defense" -> "防御";
            default -> "";
        };
    }

    private static String normalize(String text) { return text == null ? "" : text.toLowerCase(Locale.ROOT).strip(); }
}
