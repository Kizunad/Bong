package com.bong.client.ui.preview;

import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.InventoryContainerWindows;
import com.bong.client.inventory.InventoryLoadoutWindows;
import com.bong.client.combat.QuickUseSlotStore;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.inventory.state.PhysicalBodyStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.UiWindowRuntime;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/** 真实窗口命中、跨容器拖放和库存失效回归；物品定义来自服务端 TOML 导出。 */
final class UiInventoryWindowPreviewScene implements UiPreviewScene {
    private InventoryItem pouch;
    private InventoryItem herb;
    private InventoryItem pickaxe;
    private String packId;

    @Override public void installFixture(UiPreviewConfig config) {
        UiWindowRuntime.beginPreview();
        UiWindowRuntime.manager().reset();
        PhysicalBodyStore.replace(PhysicalBody.builder().build());
        pouch = config.items().get("grass_pouch");
        herb = config.items().get("herb_bundle");
        pickaxe = config.items().get("pickaxe_iron");
        if (pouch == null || herb == null || pickaxe == null) throw new IllegalArgumentException("容器预览缺少 TOML 物品");
        packId = "pack_" + pouch.instanceId();
        TechniquesListPanel.replace(List.of(new TechniquesListPanel.Technique(
            "sword.thrust", "刺", TechniquesListPanel.Grade.MORTAL, 1, true, "", "", "",
            List.of(), 0, 0, 0, 0)));
        SkillBarStore.replace(SkillBarConfig.empty().withSlot(0, SkillBarEntry.skill("sword.thrust", "刺", 0, 0, "")));
        InventoryStateStore.replace(inventory(false, true));
    }

    private InventoryModel inventory(boolean moved, boolean withPack) {
        var definitions = new ArrayList<InventoryModel.ContainerDef>();
        definitions.add(new InventoryModel.ContainerDef("body_pocket", "贴身口袋", 3, 4, null, true));
        if (withPack) definitions.add(new InventoryModel.ContainerDef(packId, pouch.displayName(), 4, 5, pouch.instanceId()));
        var builder = InventoryModel.builder().containers(definitions)
            .gridItem(pickaxe, "body_pocket", 0, 0);
        if (withPack) builder.equip(EquipSlotType.CHEST, pouch);
        if (moved && withPack) builder.gridItem(herb, packId, 1, 3);
        else builder.gridItem(herb, "body_pocket", 0, 2);
        return builder.build();
    }

    @Override public Screen createScreen() { return new InspectScreen(InventoryStateStore.snapshot()); }
    @Override public String selectedTemplateId(Screen screen) { return "inventory-container"; }
    @Override public boolean isReady(Screen screen) { return ((InspectScreen) screen).windowHostReadyForPreview(); }
    @Override public boolean initializationFailed(Screen screen) { return ((InspectScreen) screen).windowHostFailedForPreview(); }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        var client = MinecraftClient.getInstance();
        var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        var manager = UiWindowRuntime.manager();
        if (shot.name().contains("loadout")) {
            prepareLoadout(screen, context, shot);
            UiWindowRuntime.previewMotion(true);
            return;
        }
        for (var window : manager.snapshot()) {
            if (window.definition().equals(com.bong.client.inventory.InventoryLoadoutWindows.EQUIPMENT)
                || window.definition().equals(com.bong.client.inventory.InventoryLoadoutWindows.SHORTCUTS)) manager.close(window.key());
        }
        UiWindowRuntime.openContainer("body_pocket");
        UiWindowRuntime.openContainer(packId);
        var pocket = state("body_pocket");
        var pack = state(packId);
        manager.settleAt(pocket.key(), new UiWindowManager.Rect(4, 8, 180, 190));
        manager.settleAt(pack.key(), new UiWindowManager.Rect(shot.expectedLogicalWidth() - 192, 56, 188, 216));
        render(screen, context);
        var pocketGrid = UiWindowRuntime.containers().grid("body_pocket");
        var packGrid = UiWindowRuntime.containers().grid(packId);
        var source = pocketGrid.slotAt(0, 2);
        // 拾取会置顶源窗口，落点必须仍处于目标窗口的露出区域。
        var target = packGrid.slotAt(1, 3);
        requireGridAt(pocketGrid, source.x() + 8, source.y() + 8);
        requireGridAt(packGrid, target.x() + 8, target.y() + 8);
        var requests = new ArrayList<com.google.gson.JsonObject>();
        ClientRequestSender.setBackendForTests((channel, bytes) -> requests.add(
            JsonParser.parseString(new String(bytes, StandardCharsets.UTF_8)).getAsJsonObject()));
        try {
            drag(screen, source.x() + 8, source.y() + 8, target.x() + 8, target.y() + 8);
            if (requests.size() != 1) throw new IllegalStateException("跨窗拖放请求数量错误: " + requests);
            var request = requests.get(0);
            if (!"inventory_move_intent".equals(request.get("type").getAsString())
                || request.get("instance_id").getAsLong() != herb.instanceId()
                || !"body_pocket".equals(request.getAsJsonObject("from").get("container_id").getAsString())
                || !packId.equals(request.getAsJsonObject("to").get("container_id").getAsString())) {
                throw new IllegalStateException("跨窗拖放未使用真实 instance/container identity: " + request);
            }
            InventoryStateStore.replace(inventory(true, true));
            render(screen, context);

            // 窗口焦点不能吞掉物品旋转键，旋转结果必须进入真实移动协议。
            var tool = pocketGrid.slotAt(0, 0);
            var toolTarget = packGrid.slotAt(0, 3);
            screen.mouseClicked(tool.x() + 8, tool.y() + 8, 0);
            screen.mouseDragged(tool.x() + 18, tool.y() + 8, 0, 10, 0);
            screen.keyPressed(org.lwjgl.glfw.GLFW.GLFW_KEY_R, 0, 0);
            screen.mouseReleased(toolTarget.x() + 8, toolTarget.y() + 8, 0);
            if (requests.size() != 2 || !requests.get(1).has("rotated")
                || !requests.get(1).get("rotated").getAsBoolean()
                || requests.get(1).get("instance_id").getAsLong() != pickaxe.instanceId()) {
                throw new IllegalStateException("窗口内旋转未进入移动协议: " + requests);
            }
            InventoryStateStore.replace(inventory(true, true));
            render(screen, context);

            // 上层详情窗口覆盖格子时，格子不得继续接受底层拾取或落位。
            UiWindowRuntime.openItem(herb.instanceId());
            var detail = manager.snapshot().get(manager.snapshot().size() - 1);
            manager.settleAt(detail.key(), new UiWindowManager.Rect(pack.bounds().x(), pack.bounds().y(), 180, 170));
            render(screen, context);
            if (UiWindowRuntime.containerGridAt(target.x() + 8, target.y() + 8) != null) {
                throw new IllegalStateException("被详情窗口挡住的容器仍能命中");
            }
            manager.close(detail.key());
            render(screen, context);

            var bounds = pack.bounds();
            click(screen, bounds.x() + bounds.width() - 32, bounds.y() + 10);
            click(screen, bounds.x() + bounds.width() - 54, bounds.y() + 10);
            if (!pack.pinned() || !pack.minimized() || pack.scope().isClosed()) {
                throw new IllegalStateException("容器固定/最小化没有保留窗口 scope");
            }
            render(screen, context);
            var anchor = UiWindowRuntime.restoreBoundsForPreview(pack.key());
            click(screen, anchor.x() + 12, anchor.y() + 8);
            render(screen, context);
            if (state(packId) != pack || pack.minimized()) throw new IllegalStateException("恢复复制了容器窗口");

            click(screen, bounds.x() + bounds.width() - 10, bounds.y() + 10);
            if (!pack.scope().isClosed()) throw new IllegalStateException("显式关闭未释放容器窗口");
            UiWindowRuntime.openContainer(packId);
            pack = state(packId);
            manager.settleAt(pack.key(), bounds);
            render(screen, context);
            target = packGrid.slotAt(1, 3);
            requireGridAt(packGrid, target.x() + 8, target.y() + 8);
            if (packGrid.itemAt(1, 3) == null) throw new IllegalStateException("关闭后重开丢失容器内容");

            // 从容器拖出物品期间，权威快照移除容器，松手必须无请求。
            screen.mouseClicked(target.x() + 8, target.y() + 8, 0);
            screen.mouseDragged(target.x() + 18, target.y() + 8, 0, 10, 0);
            requests.clear();
            InventoryStateStore.replace(inventory(false, false));
            screen.mouseReleased(source.x() + 8, source.y() + 8, 0);
            if (!pack.scope().isClosed() || !requests.isEmpty()) {
                throw new IllegalStateException("容器失效后仍保留窗口或发送旧来源拖放: " + requests);
            }
            InventoryStateStore.replace(inventory(false, true));
            UiWindowRuntime.openContainer(packId);
            manager.settleAt(state(packId).key(), bounds);
            render(screen, context);
            if (shot.name().contains("scroll")) {
                manager.resize(state(packId).key(), "180", "120");
                render(screen, context);
                var grid = UiWindowRuntime.containers().grid(packId);
                var last = grid.slotAt(3, 4);
                if (UiWindowRuntime.containerGridAt(last.x() + 8, last.y() + 8) != null) {
                    throw new IllegalStateException("窗口裁剪以外的格子仍可命中");
                }
                screen.mouseScrolled(bounds.x() + 20, bounds.y() + 48, -4);
                render(screen, context);
            }
            if (shot.name().contains("resize")) {
                bounds = state(packId).bounds();
                click(screen, bounds.x() + bounds.width() - 76, bounds.y() + 10);
                render(screen, context);
            }
            UiWindowRuntime.previewMotion(true);
        } finally {
            ClientRequestSender.resetBackendForTests();
        }
    }

    private static UiWindowManager.WindowState state(String id) {
        return UiWindowRuntime.manager().snapshot().stream().filter(state ->
            state.definition().equals(InventoryContainerWindows.DEFINITION) && state.key().identity().equals(id))
            .findFirst().orElseThrow();
    }

    private void prepareLoadout(Screen screen, DrawContext context, UiPreviewShot shot) {
        QuickUseSlotStore.replace(QuickSlotConfig.empty().withEligibleItems(java.util.Set.of(herb.itemId())));
        var manager = UiWindowRuntime.manager();
        var equipment = manager.snapshot().stream().filter(w -> w.definition().equals(InventoryLoadoutWindows.EQUIPMENT)).findFirst().orElseThrow();
        var shortcuts = manager.snapshot().stream().filter(w -> w.definition().equals(InventoryLoadoutWindows.SHORTCUTS)).findFirst().orElseThrow();
        manager.close(state(packId).key());
        manager.settleAt(state("body_pocket").key(), new UiWindowManager.Rect(198, 8, 180, 192));
        manager.settleAt(equipment.key(), new UiWindowManager.Rect(4, 8, 180, 256));
        manager.settleAt(shortcuts.key(), new UiWindowManager.Rect(392, 8, 180, 194));
        render(screen, context);
        var loadout = UiWindowRuntime.loadout();
        if (shot.expectedLogicalWidth() < 580) {
            manager.settleAt(equipment.key(), new UiWindowManager.Rect(4, 8, 180, 140));
            manager.settleAt(shortcuts.key(), new UiWindowManager.Rect(shot.expectedLogicalWidth() - 184, 48, 180, 120));
            manager.focus(equipment.key());
            render(screen, context);
            var feet = loadout.equipment().slotFor(EquipSlotType.FEET);
            if (UiWindowRuntime.equipmentAt(feet.x() + 8, feet.y() + 8) != null) {
                throw new IllegalStateException("装备窗口裁剪外的槽位仍可命中");
            }
            screen.mouseScrolled(equipment.bounds().x() + 20, equipment.bounds().y() + 48, -4);
            render(screen, context);
            return;
        }
        var hand = loadout.equipment().slotFor(EquipSlotType.MAIN_HAND);
        var grid = UiWindowRuntime.containers().grid("body_pocket");
        var tool = grid.slotAt(0, 0);
        var requests = new ArrayList<com.google.gson.JsonObject>();
        ClientRequestSender.setBackendForTests((channel, bytes) -> requests.add(
            JsonParser.parseString(new String(bytes, StandardCharsets.UTF_8)).getAsJsonObject()));
        try {
            drag(screen, tool.x() + 8, tool.y() + 8, hand.x() + 8, hand.y() + 8);
            if (requests.size() != 1 || requests.get(0).get("instance_id").getAsLong() != pickaxe.instanceId()
                || !"main_hand".equals(requests.get(0).getAsJsonObject("to").get("slot").getAsString())) {
                throw new IllegalStateException("装备窗未发送真实装备移动请求: " + requests);
            }
            InventoryStateStore.replace(InventoryModel.builder().containers(inventory(false, true).containers())
                .equip(EquipSlotType.MAIN_HAND, pickaxe).equip(EquipSlotType.CHEST, pouch)
                .gridItem(herb, "body_pocket", 0, 2).build());
            render(screen, context);
            requests.clear();
            // 独立装备窗关闭重开不清除装备；组件只挂载到一个 adapter。
            manager.close(equipment.key());
            UiWindowRuntime.openLoadout(InventoryLoadoutWindows.EQUIPMENT);
            equipment = manager.snapshot().stream().filter(w -> w.definition().equals(InventoryLoadoutWindows.EQUIPMENT)).findFirst().orElseThrow();
            manager.settleAt(equipment.key(), new UiWindowManager.Rect(4, 8, 180, 256));
            render(screen, context);
            hand = loadout.equipment().slotFor(EquipSlotType.MAIN_HAND);
            if (hand.representative() == null || hand.representative().instanceId() != pickaxe.instanceId()
                || UiWindowRuntime.equipmentAt(hand.x() + 8, hand.y() + 8) != hand) {
                throw new IllegalStateException("装备窗重开后内容或命中丢失");
            }
            manager.minimize(equipment.key());
            render(screen, context);
            if (UiWindowRuntime.equipmentAt(hand.x() + 8, hand.y() + 8) != null) throw new IllegalStateException("最小化装备仍接受操作");
            manager.restore(equipment.key());
            render(screen, context);
            drag(screen, hand.x() + 8, hand.y() + 8, tool.x() + 8, tool.y() + 8);
            if (requests.size() != 1 || !"body_pocket".equals(requests.get(0).getAsJsonObject("to").get("container_id").getAsString())) {
                throw new IllegalStateException("装备拖回容器未发送原有移动请求: " + requests);
            }
            InventoryStateStore.replace(inventory(false, true));
            render(screen, context);
            requests.clear();
            var quick = loadout.quickUseSlot(0);
            var herbSlot = grid.slotAt(0, 2);
            drag(screen, herbSlot.x() + 8, herbSlot.y() + 8, quick.x() + 8, quick.y() + 8);
            if (requests.size() != 1 || !"quick_slot_bind".equals(requests.get(0).get("type").getAsString())
                || herb.instanceId() != requests.get(0).get("instance_id").getAsLong()) {
                throw new IllegalStateException("快捷窗未走绑定确认链路: " + requests);
            }
            var request = requests.get(0);
            QuickUseSlotStore.replaceAuthoritative(QuickSlotConfig.empty().withEligibleItems(java.util.Set.of(herb.itemId())).withSlot(0,
                new QuickSlotEntry(herb.instanceId(), herb.stackCount(), herb.itemId(), herb.displayName(), 1500, 500, "")),
                request.get("request_id").getAsString(), true);
            render(screen, context);
            if (quick.item() == null || quick.item().instanceId() != herb.instanceId()) {
                throw new IllegalStateException("快捷槽未显示权威确认的物品");
            }
            requests.clear();
            var skillSource = loadout.hotbarSlot(0);
            var skillTarget = loadout.hotbarSlot(1);
            drag(screen, skillSource.x() + 8, skillSource.y() + 8, skillTarget.x() + 8, skillTarget.y() + 8);
            if (requests.size() != 2 || SkillBarStore.snapshot().slot(0) != null
                || SkillBarStore.snapshot().slot(1) == null
                || !"sword.thrust".equals(SkillBarStore.snapshot().slot(1).id())
                || requests.stream().anyMatch(payload -> !"skill_bar_bind".equals(payload.get("type").getAsString()))) {
                throw new IllegalStateException("技能换槽被窗口吞掉或依赖旧功法标签: " + requests);
            }
            requests.clear();
            screen.mouseClicked(skillTarget.x() + 8, skillTarget.y() + 8, 1);
            screen.mouseReleased(skillTarget.x() + 8, skillTarget.y() + 8, 1);
            if (requests.size() != 1 || SkillBarStore.snapshot().slot(1) != null
                || !requests.get(0).get("binding").isJsonNull()) {
                throw new IllegalStateException("独立技能槽右键解绑失败: " + requests);
            }
            manager.pin(shortcuts.key(), true);
            manager.minimize(shortcuts.key());
            QuickUseSlotStore.replace(QuickSlotConfig.empty().withEligibleItems(java.util.Set.of(herb.itemId())));
            loadout.refresh();
            manager.restore(shortcuts.key());
            render(screen, context);
            if (quick.item() != null) throw new IllegalStateException("隐藏快捷窗未跟随绑定清空");
            manager.settleAt(state("body_pocket").key(), shortcuts.bounds());
            manager.focus(state("body_pocket").key());
            render(screen, context);
            if (UiWindowRuntime.quickUseAt(quick.x() + 8, quick.y() + 8) >= 0) throw new IllegalStateException("快捷槽被遮挡仍接受操作");
            manager.settleAt(state("body_pocket").key(), new UiWindowManager.Rect(198, 8, 180, 192));
            manager.close(shortcuts.key());
            UiWindowRuntime.openLoadout(InventoryLoadoutWindows.SHORTCUTS);
            shortcuts = manager.snapshot().stream().filter(w -> w.definition().equals(InventoryLoadoutWindows.SHORTCUTS)).findFirst().orElseThrow();
            manager.settleAt(shortcuts.key(), new UiWindowManager.Rect(392, 8, 180, 194));
            render(screen, context);
            if (loadout.quickUseSlot(0) != quick || UiWindowRuntime.quickUseAt(quick.x() + 8, quick.y() + 8) != 0) {
                throw new IllegalStateException("快捷窗重开后组件重复或命中丢失");
            }
            InventoryStateStore.replace(InventoryModel.builder().containers(inventory(false, true).containers())
                .equip(EquipSlotType.MAIN_HAND, pickaxe).equip(EquipSlotType.CHEST, pouch)
                .gridItem(herb, "body_pocket", 0, 2).build());
            QuickUseSlotStore.replace(QuickSlotConfig.empty().withEligibleItems(java.util.Set.of(herb.itemId())).withSlot(0,
                new QuickSlotEntry(herb.instanceId(), herb.stackCount(), herb.itemId(), herb.displayName(), 1500, 500, "")));
            SkillBarStore.replace(SkillBarConfig.empty().withSlot(0, SkillBarEntry.skill("sword.thrust", "刺", 0, 0, "")));
            render(screen, context);
        } finally {
            ClientRequestSender.resetBackendForTests();
        }
    }

    private static void requireGridAt(BackpackGridPanel expected, double x, double y) {
        var actual = UiWindowRuntime.containerGridAt(x, y);
        if (actual != expected) throw new IllegalStateException("容器格子命中错误: expected=" + expected.containerId()
            + " actual=" + (actual == null ? null : actual.containerId()) + " point=" + x + "," + y
            + " grid=" + expected.container().x() + "," + expected.container().y()
            + " window=" + state(expected.containerId()).bounds());
    }

    private static void render(Screen screen, DrawContext context) { screen.render(context, -1, -1, 0); context.draw(); }
    private static void click(Screen screen, double x, double y) { screen.mouseClicked(x, y, 0); screen.mouseReleased(x, y, 0); }
    private static void drag(Screen screen, double x, double y, double targetX, double targetY) {
        screen.mouseClicked(x, y, 0);
        screen.mouseDragged(x + 6, y, 0, 6, 0);
        screen.mouseDragged(targetX, targetY, 0, targetX - x - 6, targetY - y);
        screen.mouseReleased(targetX, targetY, 0);
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        for (var state : UiWindowRuntime.manager().snapshot()) {
            var b = state.bounds();
            if (b.x() < 0 || b.y() < 0 || b.x() + b.width() > shot.expectedLogicalWidth()
                || b.y() + b.height() > shot.expectedLogicalHeight() - 28) {
                throw new IllegalStateException("容器窗口越出 viewport: " + b);
            }
        }
    }

    @Override public void cleanup() {
        UiWindowRuntime.manager().reset();
        UiWindowRuntime.containers().reset();
        InventoryStateStore.replace(InventoryModel.empty());
        PhysicalBodyStore.replace(null);
        QuickUseSlotStore.replace(QuickSlotConfig.empty().withEligibleItems(java.util.Set.of(herb.itemId())));
        SkillBarStore.replace(SkillBarConfig.empty());
        TechniquesListPanel.replace(List.of());
        UiWindowRuntime.endPreview();
    }
}
