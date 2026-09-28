package com.bong.client.alchemy;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.window.UiWindowManager;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Sizing;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/** 工位只管理炉位交互；丹方与炉记交给独立窗口。 */
public final class AlchemyWindowContent implements AutoCloseable {
    private final AlchemyWindows windows;
    private final UiWindowManager.WindowState owner;
    private final AlchemyFurnaceComponent furnace;
    private final Runnable openNotes;
    private final Runnable openHistory;
    private long collectUntil;
    private boolean collectKeyDown;
    private boolean draggingMaterial;
    private String recipeId = "";
    private int elapsed;

    public AlchemyWindowContent(FlowLayout root, AlchemyWindows windows,
                                UiWindowManager.WindowState owner, Runnable openNotes, Runnable openHistory) {
        this.windows = windows;
        this.owner = owner;
        this.openNotes = openNotes;
        this.openHistory = openHistory;
        furnace = new AlchemyFurnaceComponent(windows, this::hoverLines, this::interact);
        furnace.id("alchemy-furnace-art");
        furnace.sizing(Sizing.fill(100), Sizing.fill(100));
        root.child(furnace);
    }

    public void draggingMaterial(boolean dragging) {
        draggingMaterial = dragging;
        furnace.draggingMaterial(dragging);
        if (dragging) collectUntil = 0;
    }

    public void cancelInput() {
        collectUntil = 0;
        collectKeyDown = false;
        draggingMaterial = false;
        furnace.cancelInput();
    }

    @Override public void close() { furnace.close(); }

    public void tick() {
        var session = windows.model().session();
        if (owner.closed() || owner.minimized()) cancelInput();
        if (!windows.active()
            || !recipeId.equals(session.recipeId()) || session.elapsedTicks() < elapsed) {
            collectUntil = 0;
        }
        recipeId = session.recipeId();
        elapsed = session.elapsedTicks();
        furnace.tick();
    }

    public boolean drop(double x, double y, InventoryItem item) {
        if (owner.closed() || owner.minimized()) return false;
        if (furnace.incenseAt(x, y)) {
            windows.placeIncense(item);
            return true;
        }
        if (!furnace.mouthAt(x, y)) return false;
        windows.feed(item);
        return true;
    }

    /** Runtime 统一检查窗口焦点，按键不再依赖鼠标悬停部位。 */
    public boolean keyPressed(int key, int modifiers) {
        if (owner.closed() || owner.minimized() || draggingMaterial || modifiers != 0) return false;
        switch (key) {
            case GLFW.GLFW_KEY_I -> windows.ignite();
            case GLFW.GLFW_KEY_F -> windows.inject();
            case GLFW.GLFW_KEY_J -> windows.temperature(-.02);
            case GLFW.GLFW_KEY_K -> windows.temperature(.02);
            case GLFW.GLFW_KEY_R -> {
                if (!collectKeyDown) collect();
                collectKeyDown = true;
            }
            case GLFW.GLFW_KEY_N -> openNotes.run();
            case GLFW.GLFW_KEY_H -> openHistory.run();
            default -> { return false; }
        }
        return true;
    }

    public void keyReleased(int key) {
        if (key == GLFW.GLFW_KEY_R) collectKeyDown = false;
    }

    private void interact(AlchemyFurnaceComponent.Part part) {
        if (owner.closed() || owner.minimized()) return;
        switch (part) {
            case RECIPE -> openNotes.run();
            case JOURNAL -> openHistory.run();
            case QI -> windows.inject();
            case OUTLET -> collect();
            case INCENSE -> windows.requestIncenseHint();
            default -> { }
        }
    }

    private void collect() {
        if (!windows.ready() || !windows.model().furnace().hasSession()) return;
        long now = System.nanoTime();
        if (windows.model().session().elapsedTicks() < windows.model().session().targetTicks()
            && now >= collectUntil) {
            collectUntil = now + 3_000_000_000L;
            windows.requestEarlySettleWarning();
            return;
        }
        if (windows.settle().kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) collectUntil = 0;
    }

    private List<String> hoverLines(AlchemyFurnaceComponent.Part part) {
        var lines = new ArrayList<String>();
        var model = windows.model();
        var session = model.session();
        var recipe = model.recipes().current();
        // 桌面阅读和计时不依赖炉次，炉讯失效时仍可查看。
        switch (part) {
            case INCENSE -> {
                var incense = windows.incenseView();
                lines.add(switch (incense.state()) {
                    case EMPTY -> "空香座 · 待添香";
                    case BURNING -> "香火正燃";
                    case SPENT -> "香已燃尽";
                });
                if (!incense.kind().isBlank()) lines.add(windows.materialName(incense.kind()));
                lines.add("燃烧长度 " + incense.durationSeconds() + " 秒");
                if (incense.state() != AlchemyIncenseTimer.State.EMPTY) {
                    lines.add("余香 " + (incense.remainingMillis() + 999) / 1000 + " 秒");
                }
                if (incense.kind().equals("incense_plain")) lines.add("只记时，不改药性");
                else if (incense.state() != AlchemyIncenseTimer.State.EMPTY) {
                    lines.add(String.format("火候容差 ×%.2f · 真元消耗 ×%.2f",
                        incense.tempBandScale(), incense.qiCostScale()));
                }
                return lines;
            }
            case RECIPE -> {
                lines.add(recipe == null ? "尚未选择炉方" : recipe.displayName());
                if (recipe != null && !recipe.author().isBlank()) lines.add(recipe.author());
                lines.add("已学丹方 " + model.recipes().learned().size() + " 卷");
                return lines;
            }
            case JOURNAL -> {
                lines.add("已记炼制结果 " + model.history().size() + " 炉");
                if (!model.history().isEmpty()) {
                    var last = model.history().get(model.history().size() - 1);
                    if (!last.pill().isBlank()) lines.add("上次产物 " + windows.materialName(last.pill()));
                }
                return lines;
            }
            default -> { }
        }
        if (!windows.available()) {
            if (!windows.awaitingOpen()) lines.add("离丹炉太远了");
            return lines;
        }
        switch (part) {
            case LID -> {
                lines.add("药炉 " + model.furnace().tier() + " 阶 · 完整度 " + Math.round(model.furnace().integrity())
                    + "/" + Math.round(model.furnace().integrityMax()));
                lines.add(recipe == null ? "先从桌上选择丹方" : "炉方 · " + recipe.displayName());
                lines.add(windows.active() ? "本炉正在炼制" : model.furnace().hasSession() ? "本炉已到时，等待收取" : "炉内空置");
            }
            case BODY -> {
                lines.add(windows.active() ? "炼制 " + session.elapsedTicks() + " / " + session.targetTicks() + " 刻"
                    : "炉内暂歇");
                if (!session.statusLabel().isBlank()) lines.add(session.statusLabel());
            }
            case MOUTH -> {
                if (!windows.active()) lines.add("起炉后按丹方时机投料");
                else for (int index = 0; index < session.stages().size(); index++) {
                    var stage = session.stages().get(index);
                    for (var ingredient : stage.ingredients()) {
                        lines.add(windows.materialName(ingredient.material()) + " " + ingredient.inserted() + "/" + ingredient.required()
                            + (ingredient.remaining() == 0 ? " · 已足" : windows.canFeedStage(index) ? " · 待投" : " · 未到时"));
                    }
                }
            }
            case FIRE -> {
                if (!windows.active()) lines.add("炉火未起");
                else {
                    lines.add(String.format("当前火候 %.2f", session.tempCurrent()));
                    lines.add(String.format("宜守 %.2f · 容差 ±%.2f", session.tempTarget(), session.tempBand()));
                    double offset = session.tempCurrent() - session.tempTarget();
                    lines.add(Math.abs(offset) <= session.tempBand() ? "火候平稳" : offset > 0 ? "火势偏旺" : "火势偏弱");
                }
            }
            case QI -> {
                if (!windows.active()) lines.add("起炉后可引入真元");
                else {
                    lines.add(String.format("已注真元 %.1f / %.1f", session.qiInjected(), session.qiTarget()));
                    lines.add(String.format("尚需 %.1f", Math.max(0, session.qiTarget() - session.qiInjected())));
                }
            }
            case OUTLET -> {
                if (!model.furnace().hasSession()) lines.add("当前没有可收取的炉次");
                else {
                    lines.add(session.elapsedTicks() < session.targetTicks() ? "本炉尚在炼制" : "本炉已到时");
                    if (session.elapsedTicks() < session.targetTicks()) lines.add("提前收取将立即结算本炉，需再次确认");
                }
            }
            default -> { }
        }
        return lines;
    }
}
