export type ReminderQuery = { seconds: number | null; title: string; error: string };
export type Reminder = { id: string; title: string; dueAt: number; completed: boolean; notificationError?: boolean };

const durationToken = /^(\d+)\s*(seconds?|secs?|s|minutes?|mins?|m|hours?|hrs?|h|days?|d|小时|分钟|秒钟|秒|时|分|天|日)/iu;
const unitSeconds: Record<string, number> = {
  秒: 1, 秒钟: 1, s: 1, sec: 1, secs: 1, second: 1, seconds: 1,
  分: 60, 分钟: 60, m: 60, min: 60, mins: 60, minute: 60, minutes: 60,
  时: 3600, 小时: 3600, h: 3600, hr: 3600, hrs: 3600, hour: 3600, hours: 3600,
  天: 86400, 日: 86400, d: 86400, day: 86400, days: 86400,
};

export function parseReminderQuery(value: string): ReminderQuery | null {
  const command = /^(?:提醒|remind|reminder|倒计时|timer|countdown)(?:\s+|$)(.*)$/iu.exec(value.trim());
  if (!command) return null;
  const body = command[1].trim();
  if (!body) return { seconds: null, title: '', error: '' };
  const invalid = { seconds: null, title: '', error: '请输入 1 秒到 7 天的时长，例如：提醒 20分钟 休息。' };
  let seconds = 0;
  let end = 0;
  while (end < body.length) {
    const rest = body.slice(end);
    const trimmed = rest.trimStart();
    const token = durationToken.exec(trimmed);
    if (!token) break;
    const after = trimmed.slice(token[0].length);
    // 一个单位必须完整结束；空白后的“10s后开会”等文字保留为标题。
    if (after && !/^[\s\d]/u.test(after)) break;
    seconds += Number(token[1]) * unitSeconds[token[2].toLowerCase()];
    end += rest.length - trimmed.length + token[0].length;
  }
  const tail = body.slice(end);
  if (!end || tail && !/^\s/u.test(tail)) return invalid;
  if (!Number.isSafeInteger(seconds) || seconds < 1 || seconds > 604800) return invalid;
  const title = tail.trim() || '倒计时结束';
  if (Array.from(title).length > 100 || /[\u0000-\u001f\u007f]/u.test(title)) return { seconds: null, title: '', error: '提醒内容最多 100 个字符，不能包含控制字符。' };
  return { seconds, title, error: '' };
}

export function remainingTime(dueAt: number, now: number) {
  const total = Math.max(0, Math.ceil((dueAt - now) / 1000));
  if (!total) return '即将提醒';
  const seconds = String(total % 60).padStart(2, '0');
  const minutes = String(Math.floor(total / 60) % 60).padStart(2, '0');
  return total >= 3600 ? `${Math.floor(total / 3600)}:${minutes}:${seconds}` : `${minutes}:${seconds}`;
}

export function reminderError(value: unknown) {
  const raw = String(value);
  if (/disabled/i.test(raw)) return '倒计时提醒尚未启用。';
  if (/limit|capacity|maximum/i.test(raw)) return '最多同时保留 20 个待执行提醒，请先取消部分提醒。';
  if (/duration|seconds/i.test(raw)) return '时长应在 1 秒到 7 天之间。';
  if (/title/i.test(raw)) return '提醒内容最多 100 个字符，不能包含控制字符。';
  if (/not found|no longer/i.test(raw)) return '此提醒已更新，请刷新列表。';
  return '无法更新提醒，请重试。';
}
