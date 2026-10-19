import type { Provider } from './types';

type Refresh = (isCurrent: () => boolean) => Promise<Provider[] | undefined>;

export function watchLoadingProvider(refresh: Refresh) {
  let active = true;
  let timer: ReturnType<typeof setTimeout>;
  async function poll() {
    try {
      const providers = await refresh(() => active);
      // 等上次请求结束再计时；被其它状态请求取代时，保留下一次检查。
      if (active && (providers === undefined || providers.some(provider => provider.id === 'everything' && ['loading', 'indexLoading'].includes(provider.status)))) {
        timer = setTimeout(poll, 1000);
      }
    } catch {
      // 调用方负责展示错误，失败不产生无人观察的重试链。
    }
  }
  timer = setTimeout(poll, 1000);
  return () => { active = false; clearTimeout(timer); };
}
