<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { Bell, Timer, X, RefreshCw } from 'lucide-svelte';
  import { call, preview, subscribe } from './bridge';
  import { remainingTime, reminderError, type Reminder, type ReminderQuery } from './reminders';
  import { createLatestRefresh } from './refresh';

  let { query, enabled, active = true, openExtensions }: { query: ReminderQuery; enabled: boolean; active?: boolean; openExtensions: () => void } = $props();
  let reminders = $state<Reminder[]>([]);
  let pending = $state(false);
  let loading = $state(false);
  let error = $state('');
  let readError = $state('');
  let notice = $state('');
  let submitted = $state(false);
  let now = $state(Date.now());
  let current = true;
  let draftEpoch = 0;
  let refresher: ReturnType<typeof createLatestRefresh<Reminder[]>> | null = null;

  $effect(() => {
    // 输入更新不重建面板；较早创建请求的回包不能标记当前草稿为已创建。
    query.seconds; query.title; query.error; enabled; active;
    draftEpoch++;
    submitted = false; notice = ''; error = '';
  });

  async function refresh() {
    const target = refresher;
    if (!enabled || !active || !target) return;
    loading = true;
    try { await target.run(); }
    finally { if (current && target === refresher) loading = false; }
  }

  export async function submit() {
    if (!enabled || !active || query.error || query.seconds === null || pending || submitted) return;
    const epoch = draftEpoch;
    pending = true;
    error = '';
    try {
      await call('create_reminder', { seconds: query.seconds, title: query.title });
      if (!current) return;
      if (epoch === draftEpoch) {
        submitted = true;
        notice = preview ? '已创建会话样例；浏览器预览不会发送桌面通知。' : '提醒已创建，隐藏搜索窗口后继续计时。';
      }
      await refresh();
    } catch (value) { if (current && epoch === draftEpoch) error = reminderError(value); }
    finally { if (current) pending = false; }
  }

  async function cancel(id: string) {
    if (!enabled || !active || pending) return;
    const epoch = draftEpoch;
    pending = true;
    error = '';
    try {
      await call('cancel_reminder', { id });
      if (current) { if (epoch === draftEpoch) notice = '提醒已取消'; await refresh(); }
    } catch (value) { if (current && epoch === draftEpoch) error = reminderError(value); }
    finally { if (current) pending = false; }
  }

  $effect(() => {
    if (!enabled || !active) return;
    return untrack(() => {
      let listening = true;
      const target = createLatestRefresh(() => call<Reminder[]>('list_reminders'), result => {
        reminders = result; now = Date.now(); readError = '';
      }, value => { readError = reminderError(value); });
      refresher = target;
      let unsubscribe = () => {};
      // 先开始订阅，再读取快照；安装监听期间到来的事件会触发一次合并刷新。
      void subscribe('reminders_changed', () => { void refresh(); }).then(value => {
        if (listening) { unsubscribe = value; void refresh(); } else value();
      }).catch(value => { if (listening) readError = reminderError(value); });
      return () => { listening = false; target.dispose(); refresher = null; loading = false; unsubscribe(); };
    });
  });

  $effect(() => {
    // 空列表、已完成列表和不可见面板不计时；系统通知仍由原生后台负责。
    if (!enabled || !active || !reminders.some(reminder => !reminder.completed)) return;
    now = Date.now();
    const interval = setInterval(() => { now = Date.now(); }, 1000);
    return () => clearInterval(interval);
  });

  onDestroy(() => { current = false; draftEpoch++; });
</script>

<section class="utility-panel reminder-panel" aria-label="倒计时提醒">
  <header class="utility-heading"><Bell size={24} /><div class="utility-title"><h2>倒计时提醒</h2><p>{preview ? '会话样例 · 不发送系统通知' : '后台计时，到时桌面通知'}</p></div></header>
  {#if !enabled}<div class="utility-empty"><Timer size={32} /><h3>倒计时提醒尚未启用</h3><p>可设置休息、开会等临时提醒。</p><button class="primary" onclick={openExtensions}>前往扩展设置</button></div>
  {:else}
    {#if query.error}<p class="field-error" role="alert">{query.error}</p>
    {:else if query.seconds !== null}<div class="reminder-create"><div><strong>{query.title}</strong><p>{remainingTime(query.seconds * 1000, 0)} 后提醒</p></div><button class="primary" disabled={pending || submitted} onclick={submit}>{submitted ? '已创建' : pending ? '创建中…' : '创建提醒'}</button></div>
    {:else}<p class="utility-help">输入 <code>提醒 20m 休息</code> / <code>remind 20m 休息</code>，或 <code>倒计时 30s</code> / <code>timer 30s</code>，回车创建。支持 s 秒、m 分钟、h 小时、d 天，也可组合为 <code>1h30m</code>。</p>{/if}
    <div class="utility-notice" role="status">{notice}</div>
    {#if error}<p class="field-error" role="alert">{error}</p>{/if}
    {#if readError}<p class="field-error" role="alert">{readError}</p>{/if}
    <div class="reminder-list-header"><h3>提醒列表</h3><button class="text-button" disabled={pending || loading} onclick={refresh}><RefreshCw size={13} />刷新</button></div>
    <div class="reminder-list" aria-busy={loading}>
      {#each reminders as reminder (reminder.id)}<div class="reminder-row"><Timer size={18} /><div><strong>{reminder.title}</strong><small>{reminder.completed ? reminder.notificationError ? '已到期 · 系统通知未能发送' : '已到期' : remainingTime(reminder.dueAt, now)}</small></div><button class="icon-button" aria-label={`${reminder.completed ? '移除' : '取消'}提醒：${reminder.title}`} disabled={pending} onclick={() => cancel(reminder.id)}><X size={16} /></button></div>
      {:else}<p class="utility-help">{loading ? '正在读取提醒…' : '暂无提醒'}</p>{/each}
    </div>
    <p class="utility-help reminder-limits">退出程序期间无法即时通知；重新打开后恢复计时并提示已到期项。请允许 SuperSearch 的 Windows 通知。</p>
  {/if}
</section>
