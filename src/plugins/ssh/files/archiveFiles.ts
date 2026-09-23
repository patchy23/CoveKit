import type { ArchiveRequest } from '../contracts'
export function archiveFormat(path: string): ArchiveRequest['format'] | undefined {
  const name = path.toLowerCase()
  if (name.endsWith('.tar.gz') || name.endsWith('.tgz')) return 'tar.gz'
  if (name.endsWith('.zip')) return 'zip'
  if (name.endsWith('.gz')) return 'gz'
}
export function archiveOutputName(path: string) {
  const name = path.split('/').pop() ?? ''
  return name.replace(/\.(tar\.gz|tgz|zip|gz)$/i, '') || '解压文件'
}
