import type { Narration, ZongCoreActivationV1, ZongmenOriginIdV1 } from "@bong/schema";

export const ZONG_ORIGIN_NAMES: Record<ZongmenOriginIdV1, string> = {
  1: "血溪",
  2: "北陵",
  3: "南渊",
  4: "赤霞",
  5: "玄水",
  6: "太初",
  7: "幽暗",
};

export const ZONG_STELE_FRAGMENTS: Record<ZongmenOriginIdV1, readonly string[]> = {
  1: [
    "斗台下的血早干了，石缝却还记得当年万人呼吸同乱。",
    "血溪旧徒把经脉当作兵刃，兵刃折时，人才知道痛会传给后人。",
    "碑背有一行细字：爆脉可胜一阵，不可胜一世。",
  ],
  2: [
    "北陵量天，先量地脉，再量人心；两者皆歪，阵便自吃其主。",
    "残柱影子落成八门，缺的那一门被人用骨灰补过。",
    "碑面剥落，只剩一句：阵眼不可久醒。",
  ],
  3: [
    "南渊蛊池已空，池沿仍有药香与毒气争一口旧气。",
    "医者救人，蛊者借人；末法来时，两种手段都要付账。",
    "残皿中刻着丹方半句，后半句被虫啃得干净。",
  ],
  4: [
    "赤霞塔残，铜骨向天，像仍在等一道不会再来的正雷。",
    "雷法旧谱烧成黑边，最亮处反倒没有字。",
    "碑下压着半枚暗器，雷纹与杀意缠在一处。",
  ],
  5: [
    "玄水试剑碑林皆向北倾，像一群弟子还在等师长回头。",
    "剑痕入石三寸，水意只余一线寒光。",
    "有人在碑角补刻：截脉先截己疑。",
  ],
  6: [
    "太初阵盘黑白相咬，碎纹绕成一条断掉的经络图。",
    "任督旧义讲求通达，末法之后，通达二字最先堵死。",
    "盘心紫晶微亮，亮得太平，反像有人刚刚离开。",
  ],
  7: [
    "幽暗影壁不投人影，只把脚步声还给来者。",
    "替尸旧术刻在灯下，读到第三行，字便像换了位置。",
    "墙根有暗器孔七十二，只有一孔仍向外吐冷风。",
  ],
};

export function renderZongCoreActivationNarration(event: ZongCoreActivationV1): Narration {
  const origin = ZONG_ORIGIN_NAMES[event.origin_id];
  return {
    scope: "zone",
    target: event.zone_id,
    text: `${origin}故地灵脉异动，残阵亮了一息又压住尘土。近处的人会闻到旧殿石粉，远处的人只觉得北斗偏了一寸。`,
    style: "perception",
  };
}

export function renderZongSteleNarration(
  originId: ZongmenOriginIdV1,
  fragmentIndex: number,
  zoneId: string,
): Narration {
  const fragments = ZONG_STELE_FRAGMENTS[originId];
  const index = Math.abs(Math.trunc(fragmentIndex)) % fragments.length;
  return {
    scope: "zone",
    target: zoneId,
    text: fragments[index] ?? fragments[0],
    style: "perception",
  };
}
