import type { ComposeProject } from '../contracts'

/** 项目名是 Docker 侧身份，查询到的快照优先于本地路径记录。 */
export function mergeComposeProjects(remote: ComposeProject[], remembered: ComposeProject[]) {
  const projects = new Map(
    remembered.map((project) => [project.name, { ...project, status: '本地记录 · 未查询到容器' }])
  )
  for (const project of remote) projects.set(project.name, project)
  return [...projects.values()].sort((a, b) => a.name.localeCompare(b.name))
}

export function validateComposeDraft(name: string, path: string) {
  if (!/^[a-z0-9][a-z0-9_-]*$/.test(name))
    return '项目名需以小写字母或数字开头，仅允许小写字母、数字、短横线和下划线'
  if (
    !path.startsWith('/') ||
    !/\.ya?ml$/i.test(path) ||
    [...path].some((char) => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127)
  )
    return '请输入远程 YAML 文件的绝对路径，如 /opt/app/compose.yaml'
  return ''
}

export const COMPOSE_TEMPLATE =
  'services:\n  app:\n    image: nginx:stable\n    restart: unless-stopped\n    ports:\n      - "8080:80"\n'
