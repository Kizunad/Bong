package com.bong.client.practice;

import com.bong.client.ui.window.UiWindowDefinition;
import java.util.Set;

/** 窗口职责独立，详情以条目 identity 去重，绑定与对比不另建管理器。 */
public final class PracticeWindows {
    public static final UiWindowDefinition CATALOG = definition("practice", "practice-catalog", 280, 220);
    public static final UiWindowDefinition DETAIL = definition("practice-detail", "practice-detail", 240, 200);
    public static final UiWindowDefinition BINDING = definition("practice-binding", "practice-detail", 260, 200);
    public static final UiWindowDefinition COMPARE = definition("practice-compare", "practice-detail", 300, 220);
    private PracticeWindows() {}
    private static UiWindowDefinition definition(String type, String template, int width, int height) {
        return new UiWindowDefinition(type, template, width, height, Set.of(UiWindowDefinition.Capability.WINDOW));
    }
}
