import { useEffect, useState } from 'react';
import { api } from '../api';
import type { FileProgress, FileRecord, Message } from '../types';
import { cls, fileKind, formatBytes } from '../utils';
import {
  IconDownload,
  IconExternal,
  IconFile,
  IconImage,
  IconMic,
  IconVideo,
} from './Icons';

interface Props {
  message: Message;
  progress?: FileProgress;
  compact?: boolean;
  onAccept: (id: string) => void;
  onDecline: (id: string) => void;
}

export function Attachment({ message, progress, compact, onAccept, onDecline }: Props) {
  const [record, setRecord] = useState<FileRecord | null>(null);
  const [thumb, setThumb] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const transferId = message.file_id;

  useEffect(() => {
    if (!transferId) return;
    let alive = true;
    api
      .fileRecord(transferId)
      .then((value) => {
        if (alive) setRecord(value);
      })
      .catch(() => undefined);
    api
      .fileThumbnail(transferId)
      .then((value) => {
        if (alive) setThumb(value);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [transferId, progress?.state]);

  if (!transferId) return null;

  const mime = record?.mime ?? progress?.mime ?? '';
  const name = record?.name ?? progress?.name ?? 'Attachment';
  const size = record?.size ?? progress?.size ?? 0;
  const received = progress?.received ?? record?.received ?? 0;
  const state = progress?.state ?? record?.state ?? 'awaiting_accept';
  const kind = fileKind(mime, name) === 'file' ? (message.kind === 'audio' ? 'audio' : 'file') : fileKind(mime, name);
  const busy = state === 'downloading' || state === 'uploading';
  const percent = size > 0 ? Math.min(100, Math.round((received / size) * 100)) : busy ? 15 : 0;
  const pending = state === 'pending_offer' && message.kind !== 'system';

  const run = async (action: 'open' | 'reveal') => {
    try {
      setError(null);
      if (action === 'open') await api.openFile(transferId);
      else await api.revealFile(transferId);
    } catch (err) {
      setError(String(err));
    }
  };

  const caption = message.kind === 'text' || message.kind === 'system' ? null : message.text;

  return (
    <div className={cls('mt-1 max-w-full', compact ? 'text-xs' : 'text-sm')}>
      {(kind === 'image' || kind === 'video') && (
        <div
          className="relative w-full overflow-hidden rounded-xl border divider"
          style={{ background: 'linear-gradient(135deg, rgba(79,93,255,0.25), rgba(16,179,162,0.22))' }}
        >
          {thumb ? (
            <img src={thumb} alt={name} className="max-h-64 w-full object-cover" draggable={false} />
          ) : (
            <div className="flex h-36 w-full items-center justify-center" style={{ color: 'var(--muted)' }}>
              {kind === 'image' ? <IconImage size={30} /> : <IconVideo size={30} />}
            </div>
          )}
          {kind === 'video' && (
            <span className="absolute inset-0 flex items-center justify-center">
              <span className="flex h-11 w-11 items-center justify-center rounded-full bg-black/55 text-white backdrop-blur">
                <IconVideo size={20} />
              </span>
            </span>
          )}
          {busy && (
            <div className="absolute inset-x-0 bottom-0 h-1 bg-black/20">
              <div className="h-full transition-all" style={{ width: `${percent}%`, background: 'var(--accent)' }} />
            </div>
          )}
        </div>
      )}

      {kind !== 'image' && kind !== 'video' && (
        <div className="flex items-center gap-3 rounded-xl border p-2.5 divider" style={{ background: 'var(--bg-soft)' }}>
          <span
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg text-white"
            style={{ background: 'linear-gradient(140deg, var(--accent), var(--accent-2))' }}
          >
            {kind === 'audio' ? <IconMic size={19} /> : <IconFile size={19} />}
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate font-medium">{name}</div>
            <div className="text-[11px]" style={{ color: 'var(--muted)' }}>
              {size ? formatBytes(size) : 'Attachment'}
              {busy ? ` · ${percent}%` : ''}
              {state === 'done' ? ' · downloaded' : ''}
              {state === 'rejected' ? ' · declined' : ''}
              {state === 'failed' ? ' · failed' : ''}
              {state === 'paused' ? ' · paused' : ''}
            </div>
            {busy && (
              <div className="mt-1.5 h-1 overflow-hidden rounded-full" style={{ background: 'var(--border)' }}>
                <div className="h-full transition-all" style={{ width: `${percent}%`, background: 'var(--accent)' }} />
              </div>
            )}
          </div>
          {state === 'done' && message.kind !== 'system' && (
            <div className="flex shrink-0 gap-1">
              <button className="icon-btn h-8 w-8" onClick={() => run('open')} title="Open file">
                <IconExternal size={16} />
              </button>
              <button className="icon-btn h-8 w-8" onClick={() => run('reveal')} title="Show in folder">
                <IconDownload size={16} />
              </button>
            </div>
          )}
        </div>
      )}

      {kind === 'image' || kind === 'video' ? (
        <div className="mt-1 flex items-center justify-between gap-2 text-[11px]" style={{ color: 'var(--muted)' }}>
          <span className="truncate">
            {name}
            {size ? ` · ${formatBytes(size)}` : ''}
            {state === 'rejected' ? ' · declined' : ''}
            {state === 'failed' ? ' · failed' : ''}
            {state === 'done' ? ' · ready' : ''}
          </span>
          {state === 'done' && message.kind !== 'system' && (
            <span className="flex shrink-0 gap-0.5">
              <button className="icon-btn h-7 w-7" onClick={() => run('open')} title="Open">
                <IconExternal size={14} />
              </button>
              <button className="icon-btn h-7 w-7" onClick={() => run('reveal')} title="Show in folder">
                <IconDownload size={14} />
              </button>
            </span>
          )}
        </div>
      ) : null}

      {caption && (
        <p className="mt-1 whitespace-pre-wrap break-words" style={{ color: 'inherit' }}>
          {caption}
        </p>
      )}

      {pending && (
        <div className="mt-2 flex gap-2">
          <button className="btn-primary px-3 py-1.5 text-xs" onClick={() => onAccept(transferId)}>
            Accept
          </button>
          <button className="btn-ghost px-3 py-1.5 text-xs" onClick={() => onDecline(transferId)}>
            Decline
          </button>
        </div>
      )}

      {error && <p className="mt-1 text-[11px] text-red-500">{error}</p>}
    </div>
  );
}
