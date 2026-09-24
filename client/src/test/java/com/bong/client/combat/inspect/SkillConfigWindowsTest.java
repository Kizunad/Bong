package com.bong.client.combat.inspect;

import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.window.UiWindowManager;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class SkillConfigWindowsTest {
    private final UiWindowManager manager = new UiWindowManager(640, 360);
    private final List<TechniquesListPanel.Technique> techniques = new ArrayList<>(List.of(
        new TechniquesListPanel.Technique("zhenmai.sever_chain", "绝脉", TechniquesListPanel.Grade.MORTAL,
            1, true, "", "", "", List.of(), 0, 0, 0, 0)));
    private boolean casting;
    private final List<TechniqueIntent> sent = new ArrayList<>();
    private final SkillConfigWindows windows = new SkillConfigWindows(manager, () -> techniques, () -> casting,
        intent -> { sent.add(intent); return UiIntentResult.accepted(); });
    private final UiWindowManager.Rect bounds = new UiWindowManager.Rect(20, 20, 240, 200);

    @Test void minimizeAndReopenPreserveOwnerButLateSaveCannotAffectReplacement() {
        var first = windows.open("zhenmai.sever_chain", bounds);
        manager.minimize(first.key());
        windows.refresh();
        assertFalse(first.closed(), "最小化不能销毁编辑 scope");
        assertSame(first, windows.open("zhenmai.sever_chain", bounds));
        manager.close(first.key());
        var next = windows.open("zhenmai.sever_chain", bounds);
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.save(first, new JsonObject()).kind());
        assertTrue(sent.isEmpty(), "关闭后旧保存回调不能发送配置");
        assertFalse(next.closed(), "旧窗口回调不能关闭同 key 的新窗口");
        var draft = SkillConfigSchemaRegistry.defaultConfig("zhenmai.sever_chain");
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, windows.save(next, draft).kind());
        assertEquals("zhenmai.sever_chain", ((TechniqueIntent.Configure) sent.get(0)).skillId());
        assertEquals(draft, ((TechniqueIntent.Configure) sent.get(0)).config());
        assertTrue(next.closed());
    }

    @Test void castingAndRemovedTechniqueRejectSaveBeforeTheNextRefresh() {
        var state = windows.open("zhenmai.sever_chain", bounds);
        casting = true;
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.save(state, new JsonObject()).kind());
        windows.refresh();
        assertTrue(state.closed());
        assertNull(windows.open("zhenmai.sever_chain", bounds));
        casting = false;
        var reopened = windows.open("zhenmai.sever_chain", bounds);
        techniques.clear();
        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, windows.save(reopened, new JsonObject()).kind());
        windows.refresh();
        assertTrue(reopened.closed());
        assertTrue(sent.isEmpty(), "施法与技能失效均禁止配置提交");
    }
}
