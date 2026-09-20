package com.bong.client.practice;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.skill.*;
import org.junit.jupiter.api.Test;
import java.util.List;
import static org.junit.jupiter.api.Assertions.*;

class PracticeCatalogTest {
    static TechniquesListPanel.Technique technique(String id, String inputKind) {
        return new TechniquesListPanel.Technique(id, "短距步", List.of("侧身"), TechniquesListPanel.Grade.MORTAL,
            .5f, "入门", true, "", "避开迎面攻势", "Awaken", List.of(), 0, 0, 40, 2.8f,
            15, "defense", inputKind, "");
    }
    @Test void textAndMultipleTagsIntersectWithoutTurningDescriptionsIntoTypes() {
        var entries = PracticeCatalog.entries(List.of(technique("dash.test", "dash"), technique("skill.test", "skill")), SkillSetSnapshot.empty());
        assertEquals(List.of("technique:dash.test"), PracticeCatalog.filter(entries, "#凡阶 #闪避 侧身").stream().map(PracticeCatalog.Entry::key).toList());
        assertTrue(PracticeCatalog.filter(entries, "#技艺 #闪避").isEmpty());
        assertEquals(1, PracticeCatalog.filter(entries, "#技艺 实战经验").size());
        assertTrue(PracticeCatalog.filter(entries, "#不存在").isEmpty());
    }
    @Test void experienceProgressUsesRealLevelAndRetainsSuppression() {
        var xp = new SkillSetSnapshot.Entry(10, 0, 100, 5000, 4, 0, 0);
        var entries = PracticeCatalog.entries(List.of(), SkillSetSnapshot.empty().withSkill(SkillId.COMBAT, xp));
        var combat = entries.stream().filter(entry -> entry.skill() == SkillId.COMBAT).findFirst().orElseThrow();
        assertEquals(1, combat.progress(), "满级经验桶归零不能把进度条显示为空");
        assertEquals(4, combat.experience().effectiveLv(), "进度不应抹掉境界压制");
    }
}
