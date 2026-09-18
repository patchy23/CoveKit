import type { ProcessInfo, SystemdService } from '../contracts'

/** 保守名单：安装在系统目录不代表系统基础服务，Docker/数据库/Web 服务不在名单内。 */
const systemUnits = new Set([
  'systemd-journald',
  'systemd-journal-flush',
  'systemd-logind',
  'systemd-udevd',
  'systemd-udev-trigger',
  'systemd-networkd',
  'systemd-networkd-wait-online',
  'systemd-resolved',
  'systemd-timesyncd',
  'systemd-tmpfiles-setup',
  'systemd-tmpfiles-setup-dev',
  'systemd-tmpfiles-clean',
  'systemd-sysctl',
  'systemd-modules-load',
  'systemd-remount-fs',
  'systemd-random-seed',
  'systemd-update-utmp',
  'systemd-update-utmp-runlevel',
  'systemd-user-sessions',
  'systemd-hostnamed',
  'systemd-localed',
  'systemd-timedated',
  'dbus',
  'dbus-broker',
  'getty@',
  'serial-getty@',
  'user@',
  'user-runtime-dir@',
  'cron',
  'crond',
  'rsyslog',
  'syslog',
  'polkit',
  'NetworkManager',
  'NetworkManager-wait-online',
  'networking',
  'chrony',
  'chronyd',
  'udev',
  'kmod-static-nodes',
  'modprobe@',
  'console-setup',
  'keyboard-setup',
])

export function isSystemService(service: SystemdService): boolean {
  if (service.hasOverrides || !service.fragmentPath) return false
  const match = service.fragmentPath.match(/^\/(?:usr\/)?lib\/systemd\/system\/([^/]+)\.service$/)
  return Boolean(match && systemUnits.has(match[1]))
}

const systemExecutables = new Set([
  '/sbin/init',
  '/usr/sbin/init',
  '/lib/systemd/systemd',
  '/usr/lib/systemd/systemd',
  ...['/lib/systemd/', '/usr/lib/systemd/'].flatMap((prefix) =>
    [
      'systemd-journald',
      'systemd-logind',
      'systemd-udevd',
      'systemd-networkd',
      'systemd-resolved',
      'systemd-timesyncd',
    ].map((name) => prefix + name)
  ),
  '/usr/bin/dbus-daemon',
  '/usr/bin/dbus-broker',
  '/usr/bin/dbus-broker-launch',
  '/usr/sbin/cron',
  '/usr/sbin/crond',
  '/usr/sbin/rsyslogd',
  '/usr/lib/polkit-1/polkitd',
  '/usr/libexec/polkitd',
])

export function isSystemProcess(process: ProcessInfo): boolean {
  const command = process.command.trim()
  // 不把所有 root 进程或带方括号的僵尸进程都当内核线程。
  if (
    (process.user === 'root' || process.user === '0') &&
    process.memoryBytes === 0 &&
    /^\[(?:kthreadd|kworker\/[^\]]+|ksoftirqd\/\d+|migration\/\d+|watchdog\/\d+|cpuhp\/\d+|rcu_[^\]]+|kcompactd\d+|kswapd\d+|khungtaskd|kdevtmpfs|kauditd|kthrotld|oom_reaper|writeback|jbd2\/[^\]]+|kblockd)\]$/.test(
      command
    )
  )
    return true
  return systemExecutables.has(command.split(/\s+/, 1)[0])
}
