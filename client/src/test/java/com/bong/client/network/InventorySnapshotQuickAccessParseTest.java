package com.bong.client.network;

import com.bong.client.inventory.model.InventoryModel;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * B 准则（快捷栏来源限制）—— InventorySnapshotHandler.parseContainers 对 server
 * {@code quick_access} 字段的解析对拍：含 true → quickAccess()==true；含 false / 缺字段（旧 server）→ false。
 */
public class InventorySnapshotQuickAccessParseTest {

    private static JsonObject container(String id, String name, int rows, int cols) {
        JsonObject o = new JsonObject();
        o.addProperty("id", id);
        o.addProperty("name", name);
        o.addProperty("rows", rows);
        o.addProperty("cols", cols);
        return o;
    }

    @Test
    void parsesQuickAccessTruePresent() {
        JsonObject bp = container("body_pocket", "贴身口袋", 2, 3);
        bp.addProperty("quick_access", true);
        JsonArray arr = new JsonArray();
        arr.add(bp);

        List<InventoryModel.ContainerDef> defs = InventorySnapshotHandler.parseContainers(arr);
        assertEquals(1, defs.size());
        assertTrue(defs.get(0).quickAccess(),
            "server 下发 quick_access=true 应解析为 ContainerDef.quickAccess()==true");
    }

    @Test
    void parsesQuickAccessFalsePresent() {
        JsonObject pack = container("pack_1007", "破草包", 3, 3);
        pack.addProperty("quick_access", false);
        JsonArray arr = new JsonArray();
        arr.add(pack);

        List<InventoryModel.ContainerDef> defs = InventorySnapshotHandler.parseContainers(arr);
        assertFalse(defs.get(0).quickAccess(),
            "quick_access=false 应解析为 quickAccess()==false");
    }

    @Test
    void missingQuickAccessDefaultsFalseForLegacyServer() {
        // 旧 server skip_serializing_if 省略 false（或完全不发该键）→ client 缺字段读为 false，不退化。
        JsonArray arr = new JsonArray();
        arr.add(container("main_pack", "主背包", 5, 7));

        List<InventoryModel.ContainerDef> defs = InventorySnapshotHandler.parseContainers(arr);
        assertFalse(defs.get(0).quickAccess(),
            "缺 quick_access 字段（旧 server）应默认 false（向后兼容，不误判为可入快捷栏）");
    }
}
