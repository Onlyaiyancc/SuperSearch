import { describe, expect, it } from 'vitest';
import { createTranslationSession, parseTranslationQuery, translationError } from '../src/translation';
import type { TranslationResult, TranslationState } from '../src/translation';

describe('translation command routing', () => {
  it('recognizes explicit prefixes and preserves the submitted text', () => {
    expect(parseTranslationQuery('fy hello world')).toEqual({ mode: 'translate', text: 'hello world' });
    expect(parseTranslationQuery('  翻译  开发计划  ')).toEqual({ mode: 'translate', text: '开发计划' });
    expect(parseTranslationQuery('DICT hello')).toEqual({ mode: 'dictionary', text: 'hello' });
    expect(parseTranslationQuery('查词 hello')).toEqual({ mode: 'dictionary', text: 'hello' });
    expect(parseTranslationQuery('fy')).toEqual({ mode: 'translate', text: '' });
  });

  it.each(['翻译', 'translate', 'TRANSLATE', 'fy'])('supports translation command %s with or without text', command => {
    expect(parseTranslationQuery(`  ${command}  hello world  `)).toEqual({ mode: 'translate', text: 'hello world' });
    expect(parseTranslationQuery(command)).toEqual({ mode: 'translate', text: '' });
  });

  it.each(['查词', 'dictionary', 'DICTIONARY', 'dict'])('supports dictionary command %s with or without text', command => {
    expect(parseTranslationQuery(`${command}\thello`)).toEqual({ mode: 'dictionary', text: 'hello' });
    expect(parseTranslationQuery(command)).toEqual({ mode: 'dictionary', text: '' });
  });

  it.each(['fyi.txt', 'dictionary.pdf', 'translator.md', 'translate.txt', 'translatehello', 'dictionaryhello', '翻译文档.docx', '查词笔记', 'C:\\fy\\hello', 'hello'])('leaves ordinary searches alone: %s', query => {
    expect(parseTranslationQuery(query)).toBeNull();
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (value: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe('explicit lookup session', () => {
  const request = { mode: 'translate' as const, text: 'hello', direction: 'en-zh' as const };
  const result: TranslationResult = { text: '你好', source: 'MyMemory', phonetic: '', meanings: [] };

  it('does not send anything before explicit submission', () => {
    let calls = 0;
    const session = createTranslationSession(async () => { calls++; return result; }, () => {});
    session.reset();
    expect(calls).toBe(0);
  });

  it('publishes loading and the final result', async () => {
    const reply = deferred<typeof result>();
    const states: TranslationState[] = [];
    const session = createTranslationSession(() => reply.promise, state => states.push(state));
    const work = session.submit(request);
    expect(states.at(-1)?.pending).toBe(true);
    reply.resolve(result);
    await work;
    expect(states.at(-1)).toMatchObject({ pending: false, result });
  });

  it('discards success and failure replies after reset', async () => {
    for (const failed of [false, true]) {
      const reply = deferred<typeof result>();
      let current: unknown;
      const session = createTranslationSession(() => reply.promise, state => { current = state; });
      const work = session.submit(request);
      session.reset();
      const reset = current;
      if (failed) reply.reject(new Error('Translation timeout')); else reply.resolve(result);
      await work;
      expect(current).toBe(reset);
    }
  });

  it('prevents duplicate submissions while one request is running', async () => {
    const reply = deferred<typeof result>();
    let calls = 0;
    const session = createTranslationSession(() => { calls++; return reply.promise; }, () => {});
    const work = session.submit(request);
    await session.submit(request);
    expect(calls).toBe(1);
    reply.resolve(result);
    await work;
  });

  it('shows a retryable error without exposing raw server content', async () => {
    let current: { error: string } | undefined;
    const session = createTranslationSession(async () => { throw new Error('Translation quota'); }, state => { current = state; });
    await session.submit(request);
    expect(current?.error).toContain('额度');
    expect(translationError(new Error('<html>private server error</html>'))).not.toContain('html');
  });
});
