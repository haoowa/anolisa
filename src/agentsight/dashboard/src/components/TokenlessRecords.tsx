import React, { useEffect, useRef, useState } from 'react';
import { fetchTokenlessRecords, refreshTokenlessRecords, fetchTokenlessRecord } from '../utils/apiClient';
import type { SavedTokenlessRecord } from '../utils/apiClient';
import { useI18n } from '../i18n';

/** View saved statistics and request full content only when explicitly selected. */
export const TokenlessRecords: React.FC<{ startMs: number; endMs: number; onRefresh: () => void }> =
({ startMs, endMs, onRefresh }) => {
  const { t } = useI18n();
  const [records, setRecords] = useState<SavedTokenlessRecord[]>([]);
  const [session, setSession] = useState('');
  const [tool, setTool] = useState('');
  const [sourceNext, setSourceNext] = useState<string | null>(null);
  const [next, setNext] = useState<number | null>(null);
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [paged, setPaged] = useState(false);
  const [detail, setDetail] = useState<SavedTokenlessRecord | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    const id = ++generation.current;
    setRecords([]); setNext(null); setPaged(false); setDetail(null); setError(''); setBusy(true);
    fetchTokenlessRecords(startMs * 1e6, endMs * 1e6).then(page => {
      if (generation.current === id) { setRecords(page.records); setNext(page.next_before_cache_id); setConfigured(page.api_configured); }
    }).catch(e => { if (generation.current === id) setError(String(e)); })
      .finally(() => { if (generation.current === id) setBusy(false); });
    return () => { generation.current++; };
  }, [startMs, endMs]);

  const act = async (operation: 'refresh' | 'sourceNext' | 'next' | 'first' | number) => {
    const id = ++generation.current;
    setBusy(true); setError(''); setDetail(null);
    try {
      if (typeof operation === 'number') {
        const response = await fetchTokenlessRecord(operation);
        if (generation.current === id) setDetail(response.record);
      } else {
        if (operation === 'refresh' || operation === 'sourceNext') {
          const result = await refreshTokenlessRecords(session.trim(), tool.trim(),
            operation === 'sourceNext' ? sourceNext ?? undefined : undefined);
          if (generation.current === id) setSourceNext(result.next_before_id);
        }
        const page = await fetchTokenlessRecords(startMs * 1e6, endMs * 1e6,
          operation === 'next' ? next ?? undefined : undefined);
        if (generation.current === id) {
          setRecords(page.records); setNext(page.next_before_cache_id); setConfigured(page.api_configured);
          setPaged(operation === 'next');
          if (operation === 'refresh' || operation === 'sourceNext') onRefresh();
        }
      }
    } catch (e) { if (generation.current === id) setError(String(e)); }
    finally { if (generation.current === id) setBusy(false); }
  };

  if (configured === false && !records.length && !error) return null;
  return <section className="bg-white rounded-xl border border-gray-200 p-4 space-y-3">
    <h2 className="font-semibold">{t('tokenless.records')}</h2>
    <p className="text-sm text-gray-600">{t('tokenless.explanation')}</p>
    <div className="flex gap-2 flex-wrap">
      <input aria-label={t('tokenless.sourceSession')} placeholder={t('tokenless.sourceSession')}
        value={session} onChange={e => { setSession(e.target.value); setSourceNext(null); }} className="border rounded p-2" />
      <input aria-label={t('tokenless.toolId')} placeholder={t('tokenless.toolId')}
        value={tool} onChange={e => { setTool(e.target.value); setSourceNext(null); }} className="border rounded p-2" />
      <button disabled={busy || configured !== true} onClick={() => void act('refresh')}
        className="border rounded p-2 disabled:opacity-50">{t('tokenless.refresh')}</button>
      {sourceNext !== null && <button disabled={busy} onClick={() => void act('sourceNext')}
        className="border rounded p-2">{t('tokenless.sourceNext')}</button>}
    </div>
    {error && <p role="alert" className="text-red-700">{error}</p>}
    {busy && <p role="status">{t('app.loading')}</p>}
    <div className="overflow-x-auto"><table className="w-full text-sm">
      <thead><tr>{['tokenless.time', 'tokenless.sourceSession', 'tokenless.toolId', 'tokenless.localSession',
        'tokenless.mode', 'tokenless.tokens', 'tokenless.content'].map(key =>
        <th key={key} className="text-left p-2">{t(key as import('../i18n').MessageKey)}</th>)}</tr></thead>
      <tbody>{records.map(saved => <tr key={saved.cache_id} className="border-t">
        <td className="p-2 whitespace-nowrap">{saved.record.timestamp}</td>
        <td className="p-2">{saved.record.session_id ?? t('tokenless.unknown')}</td>
        <td className="p-2">{saved.record.tool_use_id ?? t('tokenless.unknown')}</td>
        <td className="p-2">{saved.agentsight_session_id ?? t('tokenless.unassociated')}</td>
        <td className="p-2">{saved.record.mode ?? t('tokenless.unknown')}</td>
        <td className="p-2">{saved.record.before_tokens} → {saved.record.after_tokens}</td>
        <td className="p-2"><button disabled={busy} onClick={() => void act(saved.cache_id)}
          className="underline">{t('tokenless.detail')}</button></td>
      </tr>)}</tbody>
    </table></div>
    {!busy && !records.length && <p>{t('tokenless.empty')}</p>}
    {paged && <button disabled={busy} onClick={() => void act('first')} className="border rounded p-2">
      {t('tokenless.first')}</button>}
    {next !== null && <button disabled={busy} onClick={() => void act('next')} className="border rounded p-2">
      {t('tokenless.next')}</button>}
    {detail && <div className="space-y-2">
      <h3>{t('tokenless.detail')} #{detail.record.id}</h3>
      <p className="text-sm">{detail.source}</p>
      {(['before_text', 'after_text', 'before_output', 'after_output'] as const).map(field =>
        <div key={field}><h4>{t(`tokenless.${field}`)}</h4><pre className="bg-gray-50 p-2 whitespace-pre-wrap break-all max-h-64 overflow-auto">
          {detail.record[field] ?? t('tokenless.unavailable')}</pre></div>)}
      <details><summary>{t('tokenless.original')}</summary><pre className="whitespace-pre-wrap break-all">
        {JSON.stringify(detail.record, null, 2)}</pre></details>
    </div>}
  </section>;
};
