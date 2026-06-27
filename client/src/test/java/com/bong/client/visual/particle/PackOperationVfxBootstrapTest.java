package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-tarkov-backpack-v1 P5 — 锁死套包三类视听反馈 event_id 与 server emit 端
 * {@code INVENTORY_PACK_UNEQUIP / _EQUIP / _STOW} 逐字符对齐 + 全部注册。
 *
 * <p>server handle_inventory_move 经 classify_pack_move emit {@code bong:inventory_pack_*}，
 * client 漏注册或注册成别的 path → 对应粒子+音效静默丢失（孤岛）。本测试在编译期外兜底。
 */
public class PackOperationVfxBootstrapTest {
    @Test
    void bootstrapRegistersAllThreePackOperationRoutes() {
        VfxRegistry.instance().clearForTests();

        VfxBootstrap.registerDefaults();

        assertTrue(
            VfxRegistry.instance().contains(PackOperationVfxPlayer.UNEQUIP_EVENT),
            "卸包 event_id 必须注册，否则 server emit 的 bong:inventory_pack_unequip 视听静默丢失"
        );
        assertTrue(
            VfxRegistry.instance().contains(PackOperationVfxPlayer.EQUIP_EVENT),
            "装包 event_id 必须注册，否则 server emit 的 bong:inventory_pack_equip 视听静默丢失"
        );
        assertTrue(
            VfxRegistry.instance().contains(PackOperationVfxPlayer.STOW_EVENT),
            "拖入 event_id 必须注册，否则 server emit 的 bong:inventory_pack_stow 视听静默丢失"
        );
    }

    /** event_id 字面值必须与 server 常量逐字符一致（gameplay_vfx.rs INVENTORY_PACK_*）。 */
    @Test
    void eventIdsMatchServerEmitConstants() {
        assertEquals("bong:inventory_pack_unequip", PackOperationVfxPlayer.UNEQUIP_EVENT.toString());
        assertEquals("bong:inventory_pack_equip", PackOperationVfxPlayer.EQUIP_EVENT.toString());
        assertEquals("bong:inventory_pack_stow", PackOperationVfxPlayer.STOW_EVENT.toString());
    }

    /** 三类 event_id 必须互不相同——否则三类反馈派发到同一 player，差异化失效。 */
    @Test
    void eventIdsAreMutuallyDistinct() {
        assertNotEquals(PackOperationVfxPlayer.UNEQUIP_EVENT, PackOperationVfxPlayer.EQUIP_EVENT);
        assertNotEquals(PackOperationVfxPlayer.UNEQUIP_EVENT, PackOperationVfxPlayer.STOW_EVENT);
        assertNotEquals(PackOperationVfxPlayer.EQUIP_EVENT, PackOperationVfxPlayer.STOW_EVENT);
    }
}
