import type { TutorialHookV1 } from "@bong/schema";

export type SpawnTutorialNarrationBaseline = {
  hook: TutorialHookV1;
  text: string;
  style: "narration" | "perception";
};

export const SPAWN_TUTORIAL_HOOK_KEYS = [
  "spawn_entered",
  "coffin_opened",
  "moved200_blocks",
  "first_meridian_opened",
  "realm_advanced_to_induce",
] as const satisfies readonly TutorialHookV1[];

export const SPAWN_TUTORIAL_NARRATION_BASELINES: readonly SpawnTutorialNarrationBaseline[] = [
  {
    hook: "spawn_entered",
    text: "你醒在冷石与湿土之间，四野无声，像有人把一口旧气重新塞回胸腔。",
    style: "narration",
  },
  {
    hook: "coffin_opened",
    text: "棺缝里有一枚龛石，灰白如骨，握住时并不温热，只是肯认你这一遭命数。",
    style: "narration",
  },
  {
    hook: "moved200_blocks",
    text: "风从草根下穿过，淡青之气一阵厚一阵薄，地脉没有说话，却已分出深浅。",
    style: "perception",
  },
  {
    hook: "first_meridian_opened",
    text: "第一线经脉被你硬生生推开，痛意退后，真元像细水入渠，终于有了去处。",
    style: "narration",
  },
  {
    hook: "realm_advanced_to_induce",
    text: "醒灵旧壳在一息间裂开，远处灵气忽然有了颜色；天地仍冷，只少了一层蒙眼的灰。",
    style: "perception",
  },
] as const;
