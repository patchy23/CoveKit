/** Compose 查询返回的项目；配置文件顺序决定合并顺序。 */
export interface ComposeProject {
  name: string
  status: string
  configFiles: string[]
  workingDir?: string | null
}

export type ComposeAction =
  | 'up'
  | 'start'
  | 'stop'
  | 'restart'
  | 'down'
  | 'pull'
  | 'build'
  | 'ps'
  | 'logs'
  | 'config'
  | 'update'
  | 'recreate'
  | 'rebuild'

/** 实际命令结果，不以输出文本猜测成功。 */
export interface ComposeOutput {
  exitCode: number
  stdout: string
  stderr: string
}
