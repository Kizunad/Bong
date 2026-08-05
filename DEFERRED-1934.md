[minor] 抗平行武器对齐使用反射矩阵而非 180° 旋转；修复会改变几何算法，超出本轮仅 wiring/test 范围。
[major] Blockbench 并发保存存在 TOCTOU 覆盖窗口；需要锁/hash/原子替换设计，超出本轮范围。
[minor] render_player_pose 的 --size 无上限导致平方级内存风险；需要性能/资源策略，超出本轮范围。
[major] gen_jian_player 的 --size 无上限导致预览渲染 OOM 风险；需要性能/资源策略，超出本轮范围。
[minor] anim_pose_table 未按 Emotecraft degrees discriminator 解码；改变输入单位语义，超出本轮 wiring/test 范围。
[major] lower_dash 时间线与服务端四 tick 状态窗口不一致；需要动画时间线设计变更，禁止修改数值参数。
[minor] render_anim 两角 anchor 无法稳定任意 3/4 yaw 的全局投影边界；需要几何算法设计变更，超出本轮范围。
[major] render_jian_in_hand 的 --size 无上限导致多视图平方级内存风险；需要性能/资源策略，超出本轮范围。
