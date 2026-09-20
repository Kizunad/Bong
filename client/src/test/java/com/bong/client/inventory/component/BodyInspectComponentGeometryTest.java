package com.bong.client.inventory.component;

import com.bong.client.inventory.model.ChannelState;
import com.bong.client.inventory.model.MeridianChannel;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** 退役二维像素表镜像断言；保护没有有效真元流量时停止周流动画的契约。 */
class BodyInspectComponentGeometryTest {
    @Test void absentQiClosedSeveredAndFullyContaminatedRoutesMustNotAnimateFlow() {
        var ch=MeridianChannel.LU;
        assertTrue(BodyInspectComponent.carriesQi(ChannelState.healthy(ch,10), .5));
        assertFalse(BodyInspectComponent.carriesQi(ChannelState.healthy(ch,10), 0));
        assertFalse(BodyInspectComponent.carriesQi(null, .5));
        for(var state: new ChannelState[]{
            new ChannelState(ch,10,10,ChannelState.DamageLevel.INTACT,0,0,true),
            new ChannelState(ch,10,10,ChannelState.DamageLevel.SEVERED,0,0,false),
            new ChannelState(ch,10,10,ChannelState.DamageLevel.INTACT,1,0,false),
            new ChannelState(ch,10,0,ChannelState.DamageLevel.INTACT,0,0,false)}) {
            assertFalse(BodyInspectComponent.carriesQi(state,.5), "无有效流量不能显示通畅周流");
        }
    }
}
