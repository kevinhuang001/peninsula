import { cli, Strategy } from '@jackwener/opencli/registry';
import { AuthRequiredError, CommandExecutionError } from '@jackwener/opencli/errors';

cli({
  site: 'peninsula-bilibili',
  name: 'likes',
  description: 'Read recent likes received by the current Bilibili account',
  access: 'read',
  example: 'opencli peninsula-bilibili likes -f json',
  domain: 'api.bilibili.com',
  strategy: Strategy.COOKIE,
  args: [{ name: 'limit', type: 'int', default: 50, help: 'Maximum like events to return' }],
  columns: ['id', 'actor', 'content_type', 'content', 'timestamp', 'url'],
  func: async (page, kwargs) => {
    const limit = Math.max(1, Math.min(200, Number(kwargs.limit ?? 50)));
    await page.goto('https://api.bilibili.com/');
    const result = await page.evaluate(async (maxItems) => {
      const rows = [];
      for (let pn = 1; rows.length < maxItems && pn <= 10; pn++) {
        const response = await fetch(`/x/msgfeed/like?platform=web&ps=20&pn=${pn}`, { credentials: 'include' });
        if (!response.ok) throw new Error(`Bilibili returned HTTP ${response.status}`);
        const payload = await response.json();
        if (payload.code === -101 || payload.code === -111) return { auth: false };
        if (payload.code !== 0) throw new Error(`like feed failed: ${payload.message ?? payload.code}`);
        const source = payload.data?.like_list ?? payload.data?.items ?? payload.data?.list ?? [];
        if (!Array.isArray(source) || source.length === 0) break;
        rows.push(...source);
        if (source.length < 20) break;
      }
      return {
        auth: true,
        events: rows.slice(0, maxItems).map((event) => {
          const item = event.item ?? event;
          const user = event.user ?? event.users?.[0] ?? {};
          const target = item.item ?? item;
          const rawType = String(target.type ?? target.business ?? event.business ?? '内容');
          const targetId = String(target.id ?? target.business_id ?? target.dynamic_id ?? '');
          const targetUrl = String(target.uri ?? target.url ?? (rawType === 'archive' && target.bvid
            ? `https://www.bilibili.com/video/${target.bvid}`
            : targetId ? `https://t.bilibili.com/${targetId}` : 'https://www.bilibili.com/'));
          return {
            id: String(event.id ?? event.like_id ?? `${event.ctime ?? event.timestamp}:${targetId}:${user.mid ?? ''}`),
            actor: String(user.uname ?? event.uname ?? event.users?.map((u) => u.uname).join('、') ?? 'Bilibili 用户'),
            content_type: rawType,
            content: String(target.title ?? target.desc ?? target.message ?? '').slice(0, 240),
            timestamp: Number(event.ctime ?? event.timestamp ?? 0),
            url: targetUrl,
          };
        }),
      };
    }, limit);
    if (!result.auth) throw new AuthRequiredError('Bilibili login is required in the connected browser');
    if (!Array.isArray(result.events)) throw new CommandExecutionError('Bilibili returned an unexpected like response');
    return result.events;
  },
});
