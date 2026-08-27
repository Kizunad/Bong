package com.bong.client.ui.preview;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftScreen;
import com.bong.client.craft.CraftStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost.ComponentBounds;
import com.bong.client.ui.contract.UiViewport;
import net.minecraft.client.gui.screen.Screen;

import java.util.List;
import java.util.Map;

/** UI 截图场景白名单。新增场景必须显式登记并提供确定性 fixture。 */
final class UiPreviewScenes {
    private static final Map<String, UiPreviewScene> SCENES = Map.of(
        "craft", new CraftScene()
    );

    private UiPreviewScenes() {
    }

    static UiPreviewScene require(String sceneId) {
        UiPreviewScene scene = SCENES.get(sceneId);
        if (scene == null) {
            throw new IllegalArgumentException("未登记的 UI preview scene: " + sceneId);
        }
        return scene;
    }

    static boolean isRegistered(String sceneId) {
        return SCENES.containsKey(sceneId);
    }

    private static final class CraftScene implements UiPreviewScene {
        // 仅用于界面排版的玩家快照，不代表 qi_physics 全局灵气总量。
        private static final double PREVIEW_PLAYER_QI_CURRENT = 63.0;
        private static final double PREVIEW_PLAYER_QI_MAX = 96.0;

        @Override
        public void installFixture() {
            CraftStore.clear();
            CraftStore.replaceRecipes(List.of(
                recipe("rough_knife", CraftCategory.TOOL, "粗铁短刀", "rust_iron", 3, true),
                recipe("herb_wrap", CraftCategory.MISC, "枯草裹伤布", "withered_herb", 2, true),
                recipe("sealed_powder", CraftCategory.POISON_POWDER, "未辨毒粉", "bitter_root", 4, false)
            ));
            InventoryStateStore.replace(InventoryModel.builder()
                .gridItem(item(1001L, "rust_iron", "锈铁片", 12), 0, 0)
                .gridItem(item(1002L, "withered_herb", "枯草", 8), 0, 1)
                .gridItem(item(1003L, "bitter_root", "苦根", 1), 0, 2)
                .cultivation("醒灵", PREVIEW_PLAYER_QI_CURRENT, PREVIEW_PLAYER_QI_MAX, 1.0)
                .build());
            SkillSetStore.replace(SkillSetSnapshot.empty());
        }

        @Override
        public Screen createScreen() {
            return new CraftScreen();
        }

        @Override
        public String selectedTemplateId(Screen screen) {
            if (!(screen instanceof CraftScreen craft)) {
                throw new IllegalStateException("craft scene 打开的不是 CraftScreen");
            }
            return craft.selectedTemplateIdForTests();
        }

        @Override
        public boolean isReady(Screen screen) {
            return screen instanceof CraftScreen craft && craft.hostReadyForTests();
        }

        @Override
        public boolean initializationFailed(Screen screen) {
            return screen instanceof CraftScreen craft && craft.hostInitializationFailedForTests();
        }

        @Override
        public void validateGeometry(Screen screen, UiPreviewShot shot) {
            if (!(screen instanceof CraftScreen craft)) {
                throw new IllegalStateException("craft scene 打开的不是 CraftScreen");
            }
            int logicalWidth = shot.expectedLogicalWidth();
            int logicalHeight = shot.expectedLogicalHeight();
            ComponentBounds panel = craft.componentBoundsForPreview("craft-panel");
            ComponentBounds expectedPanel = new ComponentBounds(
                10, 6, logicalWidth - 20, logicalHeight - 12
            );
            if (!expectedPanel.equals(panel)) {
                throw new IllegalStateException(
                    "响应式面板没有填满安全区: expected=" + expectedPanel + ", actual=" + panel);
            }
            for (String id : new String[] {"craft-panel", "craft-header", "action-host"}) {
                requireInViewport(id, craft.componentBoundsForPreview(id), logicalWidth, logicalHeight);
            }
            String layoutBodyId = "craft-compact".equals(craft.selectedTemplateIdForTests())
                ? "craft-content-scroll"
                : "craft-columns";
            requireInViewport(
                layoutBodyId,
                craft.componentBoundsForPreview(layoutBodyId),
                logicalWidth,
                logicalHeight
            );
            for (String bridgeId : new String[] {"recipe-host", "material-host", "output-host"}) {
                ComponentBounds bounds = craft.componentBoundsForPreview(bridgeId);
                if (!bounds.isPositive()) {
                    throw new IllegalStateException("动态 bridge 没有有效布局: " + bridgeId + " -> " + bounds);
                }
            }
            validateHitRegions(craft, panel, shot);
            validateFocusOrder(craft);
            if ("craft-compact".equals(craft.selectedTemplateIdForTests())) {
                validateCompactScrollReachability(craft);
            }
        }

        private static void validateHitRegions(CraftScreen craft, ComponentBounds panel, UiPreviewShot shot) {
            for (String id : List.of("craft-search", "craft-fill", "craft-minus", "craft-plus", "craft-start")) {
                ComponentBounds bounds = craft.componentBoundsForPreview(id);
                if (!panel.contains(bounds)) {
                    throw new IllegalStateException("交互 hit region 不在面板安全区: " + id + " -> " + bounds);
                }
                requireHit(craft, id, bounds.centerX(), bounds.centerY(), "逻辑中心");
                UiViewport viewport = new UiViewport(
                    shot.expectedLogicalWidth(), shot.expectedLogicalHeight(), shot.guiScale(), shot.guiScale()
                );
                UiViewport.Point logical = new UiViewport.Point(bounds.centerX(), bounds.centerY());
                UiViewport.Point roundTrip = viewport.physicalToLogical(viewport.logicalToPhysical(logical));
                requireHit(craft, id, roundTrip.x(), roundTrip.y(), "物理坐标逆变换");
            }
        }

        private static void requireHit(
            CraftScreen craft,
            String expectedId,
            double logicalX,
            double logicalY,
            String source
        ) {
            String actualId = craft.componentIdAtForPreview(logicalX, logicalY);
            if (!expectedId.equals(actualId)) {
                throw new IllegalStateException(
                    source + "没有命中预期组件: expected=" + expectedId + ", actual=" + actualId
                        + ", point=" + logicalX + "," + logicalY);
            }
        }

        private static void validateFocusOrder(CraftScreen craft) {
            List<String> actual = craft.focusOrderForPreview();
            List<String> expected = "craft-compact".equals(craft.selectedTemplateIdForTests())
                ? List.of(
                    "craft-content-scroll", "craft-search", "craft-recipe-scroll",
                    "craft-fill", "craft-minus", "craft-plus", "craft-start"
                )
                : List.of(
                    "craft-search", "craft-recipe-scroll",
                    "craft-fill", "craft-minus", "craft-plus", "craft-start"
                );
            int cursor = -1;
            for (String id : expected) {
                int next = actual.subList(cursor + 1, actual.size()).indexOf(id);
                if (next < 0) {
                    throw new IllegalStateException(
                        "Tab 焦点顺序缺失或倒置: expected subsequence=" + expected + ", actual=" + actual);
                }
                cursor += next + 1;
            }
            if (actual.stream().anyMatch(id -> id.startsWith("<missing:"))) {
                throw new IllegalStateException("可聚焦组件缺少稳定 id: " + actual);
            }
        }

        private static void validateCompactScrollReachability(CraftScreen craft) {
            ComponentBounds scroll = craft.componentBoundsForPreview("craft-content-scroll");
            ComponentBounds content = craft.componentBoundsForPreview("craft-scroll-content");
            if (content.height() <= scroll.height() || content.width() > scroll.width()) {
                throw new IllegalStateException(
                    "compact 内容没有形成可用纵向滚动范围: viewport=" + scroll + ", content=" + content);
            }
            int previousBottom = content.y();
            for (String id : List.of("recipe-host", "material-host", "output-host")) {
                ComponentBounds bounds = craft.componentBoundsForPreview(id);
                if (!content.contains(bounds) || bounds.y() < previousBottom) {
                    throw new IllegalStateException(
                        "compact 滚动区域不可按顺序到达: " + id + " -> " + bounds + ", content=" + content);
                }
                previousBottom = bounds.y() + bounds.height();
            }
        }

        @Override
        public void cleanup() {
            SessionScopedStoreRegistry.clearAllOnDisconnect();
        }

        private static CraftRecipe recipe(
            String id,
            CraftCategory category,
            String name,
            String material,
            int count,
            boolean unlocked
        ) {
            return new CraftRecipe(
                id, category, name,
                List.of(new CraftRecipe.MaterialEntry(material, count)),
                5.0, 80L, id, 1, CraftRecipe.Requirements.NONE, unlocked
            );
        }

        private static InventoryItem item(long instanceId, String id, String name, int count) {
            return InventoryItem.createFull(
                instanceId, id, name, 1, 1, 0.2, "common", "UI preview fixture",
                count, 1.0, 1.0
            );
        }

        private static void requireInViewport(
            String id,
            ComponentBounds bounds,
            int logicalWidth,
            int logicalHeight
        ) {
            if (!bounds.fitsInside(logicalWidth, logicalHeight)) {
                throw new IllegalStateException(
                    "关键组件越界: " + id + " -> " + bounds
                        + ", viewport=" + logicalWidth + "x" + logicalHeight);
            }
        }
    }
}
