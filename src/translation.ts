export type TranslationQuery = { mode: 'translate' | 'dictionary'; text: string };
export type TranslationRequest = TranslationQuery & { direction: 'en-zh' | 'zh-en' };
export type DictionaryMeaning = { partOfSpeech: string; definition: string; example: string };
export type TranslationResult = { text: string; source: string; phonetic: string; meanings: DictionaryMeaning[] };
export type TranslationState = { pending: boolean; result: TranslationResult | null; error: string };

export function parseTranslationQuery(query: string): TranslationQuery | null {
  const match = /^(翻译|translate|fy|查词|dictionary|dict)(?:\s+([\s\S]*))?$/iu.exec(query.trim());
  if (!match) return null;
  return { mode: /^(查词|dictionary|dict)$/iu.test(match[1]) ? 'dictionary' : 'translate', text: (match[2] ?? '').trim() };
}

export function translationError(value: unknown) {
  const raw = value instanceof Error ? value.message : String(value);
  const messages: [RegExp, string][] = [
    [/disabled/i, '请先在设置 → 扩展中启用“翻译与词典”。'],
    [/quota|429/i, '公共服务额度已用完或请求过于频繁，请稍后再试。'],
    [/timeout|timed out/i, '服务请求超时，请检查网络后重试。'],
    [/not found/i, '未找到这个英文单词，请检查拼写后重试。'],
    [/too long/i, '文本过长；翻译最多 500 个 UTF-8 字节（常见汉字约 166 个）。'],
    [/invalid word/i, '查词仅支持单个英文单词，可包含连字符或英文撇号。'],
    [/empty/i, '请先输入要翻译的文本或查询的单词。'],
    [/busy/i, '上一个查询仍在处理中，请稍后重试。'],
    [/clipboard/i, '无法复制内容，请检查剪贴板后重试。'],
    [/preview/i, '预览仅展示样例，真实翻译请使用桌面应用。'],
  ];
  return messages.find(([pattern]) => pattern.test(raw))?.[1] ?? '公共服务暂不可用，请检查网络后重试。';
}

export function createTranslationSession(
  lookup: (request: TranslationRequest) => Promise<TranslationResult>,
  changed: (state: TranslationState) => void,
) {
  let epoch = 0;
  let pending = false;
  return {
    reset() {
      epoch++;
      pending = false;
      changed({ pending: false, result: null, error: '' });
    },
    async submit(request: TranslationRequest) {
      if (pending) return;
      const current = ++epoch;
      pending = true;
      changed({ pending: true, result: null, error: '' });
      try {
        const result = await lookup(request);
        if (current === epoch) changed({ pending: false, result, error: '' });
      } catch (value) {
        if (current === epoch) changed({ pending: false, result: null, error: translationError(value) });
      } finally {
        // 查询变更、离开面板或隐藏窗口后，迟到回包不能重置新请求的状态。
        if (current === epoch) pending = false;
      }
    },
  };
}
