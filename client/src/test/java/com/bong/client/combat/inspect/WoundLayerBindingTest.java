package com.bong.client.combat.inspect;

import com.bong.client.combat.store.WoundsStore;
import com.bong.client.inventory.model.BodyPart;
import com.bong.client.inventory.model.PhysicalBody;
import com.bong.client.inventory.model.WoundLevel;
import com.bong.client.inventory.model.bodyplan.BodyPlanLayout;
import com.bong.client.inventory.model.bodyplan.PartAnchor;
import com.bong.client.inventory.model.bodyplan.Point2;
import com.bong.client.inventory.state.BodyPlanLayoutStore;
import com.bong.client.inventory.state.PhysicalBodyStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class WoundLayerBindingTest {
    @AfterEach void tearDown() {
        WoundsStore.resetForTests();
        PhysicalBodyStore.resetForTests();
        BodyPlanLayoutStore.resetForTests();
    }

    @Test void severityMapsToWoundLevel() {
        WoundsStore.Wound w = new WoundsStore.Wound(
            "chest", "cut", 0.6f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L
        );
        assertEquals(WoundLevel.LACERATION, WoundLayerBinding.toWoundLevel(w));

        WoundsStore.Wound bf = new WoundsStore.Wound(
            "left_hand", "bone_fracture", 0.6f, WoundsStore.HealingState.STANCHED, 0f, false, 0L
        );
        assertEquals(WoundLevel.FRACTURE, WoundLayerBinding.toWoundLevel(bf));

        WoundsStore.Wound mild = new WoundsStore.Wound(
            "head", "cut", 0.1f, WoundsStore.HealingState.BLEEDING, 0f, false, 0L
        );
        assertEquals(WoundLevel.BRUISE, WoundLayerBinding.toWoundLevel(mild));
    }

    @Test void buildBodyMapsChestWoundIntoPhysicalBody() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("chest", "cut", 0.6f,
                WoundsStore.HealingState.BLEEDING, 0f, false, 0L)
        ));
        PhysicalBody body = WoundLayerBinding.buildBody();
        assertEquals(WoundLevel.LACERATION, body.part(BodyPart.CHEST).wound());
        assertTrue(body.part(BodyPart.CHEST).bleedRate() > 0);
        assertEquals(WoundLevel.INTACT, body.part(BodyPart.HEAD).wound());
    }

    @Test void applyPushesToPhysicalBodyStore() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("head", "cut", 0.3f,
                WoundsStore.HealingState.BLEEDING, 0f, false, 0L)
        ));
        WoundLayerBinding.apply();
        assertNotNull(PhysicalBodyStore.snapshot());
        assertEquals(WoundLevel.ABRASION, PhysicalBodyStore.snapshot().part(BodyPart.HEAD).wound());
    }

    @Test void unknownPartIgnored() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("tail", "cut", 0.9f,
                WoundsStore.HealingState.BLEEDING, 0f, false, 0L)
        ));
        PhysicalBody body = WoundLayerBinding.buildBody();
        for (BodyPart bp : BodyPart.values()) {
            assertEquals(WoundLevel.INTACT, body.part(bp).wound());
        }
    }

    // plan-race-system-v1 P2b — resolvePart 的规范 id 校验改读 BodyPlanLayoutStore。

    @Test void resolvePartFallsBackToOpenWhenNoLayoutLoaded() {
        BodyPlanLayoutStore.resetForTests();
        assertEquals(BodyPart.CHEST, WoundLayerBinding.resolvePart("chest"),
            "store 尚未收到任何 layout（首帧竞态）时应放行，不能误伤既有渲染");
        assertEquals(BodyPart.HEAD, WoundLayerBinding.resolvePart("HEAD"),
            "大小写不敏感");
    }

    @Test void resolvePartLegacyAliasesWorkRegardlessOfLayoutState() {
        assertEquals(BodyPart.ABDOMEN, WoundLayerBinding.resolvePart("belly"));
        assertEquals(BodyPart.LEFT_THIGH, WoundLayerBinding.resolvePart("l_thigh"));
        assertEquals(BodyPart.RIGHT_HAND, WoundLayerBinding.resolvePart("r_hand"));

        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("humanoid", List.of(),
            List.of(new PartAnchor("chest", new Point2(0.5, 0.5))), List.of(), List.of()));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");
        assertEquals(BodyPart.ABDOMEN, WoundLayerBinding.resolvePart("belly"),
            "别名不受当前 layout 声明集合影响（不是规范 id 校验对象）");
    }

    @Test void resolvePartAcceptsCanonicalIdDeclaredByCurrentLayout() {
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("humanoid", List.of(),
            List.of(new PartAnchor("chest", new Point2(0.5, 0.5))), List.of(), List.of()));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertEquals(BodyPart.CHEST, WoundLayerBinding.resolvePart("chest"),
            "layout 的 anchors 声明了 chest，规范 id 应正常解析");
    }

    @Test void resolvePartRejectsCanonicalIdNotDeclaredByCurrentLayout() {
        // Layout 只声明了 chest；head 不在其 anchors / part_display_map 里。
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("whale", List.of(),
            List.of(new PartAnchor("chest", new Point2(0.5, 0.5))), List.of(), List.of()));
        BodyPlanLayoutStore.setCurrentPlanId("whale");

        assertNull(WoundLayerBinding.resolvePart("head"),
            "非人形构型（whale 示例）没有声明 head 时，规范 id 必须原样落空而非误配到人形枚举");
    }

    @Test void unknownWireIdStillNullEvenWithLayoutLoaded() {
        BodyPlanLayoutStore.putLayout(new BodyPlanLayout("humanoid", List.of(),
            List.of(new PartAnchor("chest", new Point2(0.5, 0.5))), List.of(), List.of()));
        BodyPlanLayoutStore.setCurrentPlanId("humanoid");

        assertNull(WoundLayerBinding.resolvePart("tail"));
        assertNull(WoundLayerBinding.resolvePart("back"),
            "server 7→16 遗留映射的 'back' 在 client 侧无对应 16 段枚举，原样透传为不可解析");
    }
}
