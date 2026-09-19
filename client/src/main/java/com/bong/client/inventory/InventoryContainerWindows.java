package com.bong.client.inventory;

import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** 库存快照拥有容器身份；工作台和 HUD 共用同一网格，不为每个窗口订阅库存。 */
public final class InventoryContainerWindows {
    public static final UiWindowDefinition DEFINITION = new UiWindowDefinition(
        "inventory-container", "inventory-container", 180, 120, Set.of(UiWindowDefinition.Capability.WINDOW));
    private final UiWindowManager manager;
    private final UiStateSource<InventoryModel> source;
    private final Map<String, BackpackGridPanel> grids = new LinkedHashMap<>();
    private InventoryModel model;

    public InventoryContainerWindows(UiWindowManager manager, UiStateSource<InventoryModel> source) {
        this.manager = manager;
        this.source = source;
    }

    public void refresh() {
        InventoryModel next = source.snapshot();
        if (next == model) return;
        model = next;
        var definitions = next.containers();
        for (String id : List.copyOf(grids.keySet())) {
            if (definitions.stream().noneMatch(def -> def.id().equals(id))) {
                manager.close(manager.key(DEFINITION.windowType(), id));
                grids.remove(id);
            }
        }
        for (var def : definitions) {
            var grid = grids.get(def.id());
            if (grid == null || grid.rows() != def.rows() || grid.cols() != def.cols()) {
                grid = new BackpackGridPanel(def.id(), def.rows(), def.cols());
                grids.put(def.id(), grid);
            }
            grid.populateFromModel(next);
        }
    }

    public UiWindowManager.WindowState open(String id, UiWindowManager.Rect bounds) {
        refresh();
        if (!grids.containsKey(id)) return null;
        return manager.openOrFocus(DEFINITION, manager.key(DEFINITION.windowType(), id), bounds);
    }

    public BackpackGridPanel grid(String id) { return grids.get(id); }
    public List<BackpackGridPanel> grids() { return List.copyOf(grids.values()); }
    public InventoryModel snapshot() { return model; }

    public InventoryModel.ContainerDef definition(String id) {
        return model == null ? null : model.containers().stream().filter(def -> def.id().equals(id)).findFirst().orElse(null);
    }

    public void reset() {
        grids.clear();
        model = null;
    }
}
