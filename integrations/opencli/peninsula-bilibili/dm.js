import { cli, Strategy } from '@jackwener/opencli/registry';
import { AuthRequiredError, CommandExecutionError } from '@jackwener/opencli/errors';

cli({
  site: 'peninsula-bilibili',
  name: 'dm',
  description: 'Read recent Bilibili private messages without marking them read',
  access: 'read',
  example: 'opencli peninsula-bilibili dm -f json',
  domain: 'message.bilibili.com',
  strategy: Strategy.COOKIE,
  args: [{ name: 'limit', type: 'int', default: 50, help: 'Maximum messages to return' }],
  columns: ['id', 'session_id', 'sender_uid', 'sender_name', 'summary', 'timestamp', 'unread', 'url'],
  func: async (page, kwargs) => {
    const limit = Math.max(1, Math.min(200, Number(kwargs.limit ?? 50)));
    await page.goto('https://message.bilibili.com/');
    const result = await page.evaluate(async (maxItems) => {
      const request = async (url) => {
        const response = await fetch(url, { credentials: 'include' });
        if (!response.ok) throw new Error(`Bilibili returned HTTP ${response.status}`);
        return response.json();
      };
      const nav = await request('https://api.bilibili.com/x/web-interface/nav');
      const ownUid = String(nav.data?.mid ?? '');
      if (nav.code === -101 || !nav.data?.isLogin) return { auth: false };
      const sessionsResponse = await request('/session_svr/v1/session_svr/get_sessions?session_type=1&group_fold=1&unfollow_fold=0&sort_rule=1&build=0&mobi_app=web');
      if (sessionsResponse.code === -101 || sessionsResponse.code === -111) return { auth: false };
      if (sessionsResponse.code !== 0) throw new Error(`get_sessions failed: ${sessionsResponse.message ?? sessionsResponse.code}`);
      const sessions = sessionsResponse.data?.session_list ?? [];
      const messages = [];
      for (const session of sessions) {
        if (messages.length >= maxItems) break;
        const talker = String(session.talker_id ?? session.talker?.mid ?? '');
        const sessionId = String(session.session_id ?? talker);
        if (!talker) continue;
        let endSeqno = 0;
        for (let pageIndex = 0; pageIndex < 10 && messages.length < maxItems; pageIndex++) {
          const url = `/svr_sync/v1/svr_sync/fetch_session_msgs?talker_id=${encodeURIComponent(talker)}&session_type=1&begin_seqno=0&end_seqno=${endSeqno}&size=20&sender_device_id=1&build=0&mobi_app=web`;
          const pageData = await request(url);
          if (pageData.code !== 0) throw new Error(`fetch_session_msgs failed: ${pageData.message ?? pageData.code}`);
          const rows = pageData.data?.messages ?? [];
          if (!Array.isArray(rows) || rows.length === 0) break;
          let oldestSeqno = Number.POSITIVE_INFINITY;
          for (const message of rows) {
            const seqno = Number(message.msg_seqno ?? message.msg_key ?? 0);
            if (seqno > 0) oldestSeqno = Math.min(oldestSeqno, seqno);
            const senderUid = String(message.sender_uid ?? message.sender_uid_v2 ?? '');
            if (senderUid && ownUid && senderUid === ownUid) continue;
            let body = '';
            try {
              const content = typeof message.content === 'string' ? JSON.parse(message.content) : message.content;
              body = String(content?.content ?? content?.text ?? content?.url ?? '').slice(0, 240);
            } catch { body = String(message.content ?? '').slice(0, 240); }
            messages.push({
              id: String(message.msg_key ?? message.msg_seqno ?? message.timestamp),
              session_id: sessionId,
              sender_uid: senderUid,
              sender_name: String(message.sender_info?.uname ?? session.session_info?.uname ?? 'Bilibili 用户'),
              summary: body,
              timestamp: Number(message.timestamp ?? 0),
              unread: Number(session.unread_count ?? 0) > 0,
              url: `https://message.bilibili.com/#/im?talker_id=${encodeURIComponent(talker)}`,
            });
            if (messages.length >= maxItems) break;
          }
          if (!Number.isFinite(oldestSeqno) || oldestSeqno <= 1) break;
          endSeqno = oldestSeqno - 1;
        }
      }
      return { auth: true, messages };
    }, limit);
    if (!result.auth) throw new AuthRequiredError('Bilibili login is required in the connected browser');
    if (!Array.isArray(result.messages)) throw new CommandExecutionError('Bilibili returned an unexpected message response');
    return result.messages;
  },
});
