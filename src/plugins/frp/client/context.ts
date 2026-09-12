/**
 * 客户端状态的注入键：工作台 provide 一份，弹窗与档案选择器 inject 同一份。
 * 若各处各建一份 composable，弹窗里改完默认项、详情页仍显示旧绑定。
 */
import type { InjectionKey } from 'vue'
import type { useFrpClients } from './useFrpClients'

/** 客户端清单共享状态（由 FrpWorkbench 提供） */
export const FRP_CLIENTS_KEY = Symbol('frp-clients') as InjectionKey<
  ReturnType<typeof useFrpClients>
>
