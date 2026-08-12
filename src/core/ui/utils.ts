import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

/** shadcn-vue 风格的类名合并入口，业务组件不直接依赖实现细节。 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
