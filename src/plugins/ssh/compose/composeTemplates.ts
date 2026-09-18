/** 模板仅生成可编辑配置；数据使用命名卷，不隐式创建主机配置文件。 */
export const composeTemplates = [
  { value: 'empty', label: '空白编排', content: 'services: {}\n' },
  {
    value: 'nginx',
    label: 'Nginx · Web 服务',
    content:
      'services:\n  web:\n    image: nginx:stable\n    restart: unless-stopped\n    ports:\n      - "8080:80"\n',
  },
  {
    value: 'redis',
    label: 'Redis · 缓存',
    content:
      'services:\n  redis:\n    image: redis:7-alpine\n    restart: unless-stopped\n    command: redis-server --appendonly yes\n    volumes:\n      - redis_data:/data\nvolumes:\n  redis_data:\n',
  },
  {
    value: 'postgres',
    label: 'PostgreSQL · 数据库',
    content:
      'services:\n  db:\n    image: postgres:17-alpine\n    restart: unless-stopped\n    environment:\n      POSTGRES_DB: app\n      POSTGRES_USER: app\n      POSTGRES_PASSWORD: "${DB_PASSWORD:?请在项目目录的 .env 中设置 DB_PASSWORD}"\n    volumes:\n      - db_data:/var/lib/postgresql/data\nvolumes:\n  db_data:\n',
  },
  {
    value: 'wordpress',
    label: 'WordPress ＋ MySQL · 应用与数据库',
    content:
      'services:\n  web:\n    image: wordpress:6-apache\n    restart: unless-stopped\n    ports:\n      - "8080:80"\n    environment:\n      WORDPRESS_DB_HOST: db\n      WORDPRESS_DB_USER: wordpress\n      WORDPRESS_DB_NAME: wordpress\n      WORDPRESS_DB_PASSWORD: "${DB_PASSWORD:?请在项目目录的 .env 中设置 DB_PASSWORD}"\n    depends_on:\n      - db\n    volumes:\n      - web_data:/var/www/html\n  db:\n    image: mysql:8.4\n    restart: unless-stopped\n    environment:\n      MYSQL_DATABASE: wordpress\n      MYSQL_USER: wordpress\n      MYSQL_PASSWORD: "${DB_PASSWORD:?请设置 DB_PASSWORD}"\n      MYSQL_ROOT_PASSWORD: "${DB_ROOT_PASSWORD:?请设置 DB_ROOT_PASSWORD}"\n    volumes:\n      - db_data:/var/lib/mysql\nvolumes:\n  web_data:\n  db_data:\n',
  },
]

/** 远程 POSIX 路径；不套用运行客户端的 Windows 路径规则。 */
export function composePath(directory: string, filename: string) {
  return `${directory.trim().replace(/\/+$/, '')}/${filename.trim()}`
}

export function parentDirectory(path: string) {
  return path.slice(0, path.lastIndexOf('/')) || '/'
}

export function composeStatus(status: string) {
  if (/^running\(\d+\)$/.test(status)) return '运行中'
  if (status.includes('running')) return '部分运行'
  if (/exited|stopped|created/.test(status)) return '已停止'
  if (/paused/.test(status)) return '已暂停'
  return status.includes('未') ? '未部署' : status
}

/** 复用 Compose 项目状态中的计数，不为列表逐个查询容器。 */
export function composeContainerCount(status: string): number | null {
  if (status.includes('未部署') || status.includes('未查询到容器') || status.includes('尚未部署'))
    return 0
  const counts = [...status.matchAll(/\((\d+)\)/g)]
  return counts.length ? counts.reduce((sum, match) => sum + Number(match[1]), 0) : null
}
