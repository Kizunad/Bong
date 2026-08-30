package com.bong.client.coffin;

import com.bong.client.ui.contract.UiIntent;
import net.minecraft.util.math.BlockPos;

import java.util.Objects;

/** 延寿棺菜单允许发送的类型化意图。 */
public sealed interface CoffinMenuIntent extends UiIntent
    permits CoffinMenuIntent.Enter, CoffinMenuIntent.Reclaim {

    /** 请求进入当前延寿棺。 */
    record Enter(BlockPos coffinPos) implements CoffinMenuIntent {
        public Enter {
            Objects.requireNonNull(coffinPos, "coffinPos must not be null");
        }
    }

    /** 请求回收当前延寿棺。 */
    record Reclaim(BlockPos coffinPos) implements CoffinMenuIntent {
        public Reclaim {
            Objects.requireNonNull(coffinPos, "coffinPos must not be null");
        }
    }
}
