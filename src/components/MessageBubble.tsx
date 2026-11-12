import type { FileProgress, Message, Peer } from '../types';
import { Avatar } from './Avatar';
import { Attachment } from './Attachment';
import { IconCheck, IconCheckDouble } from './Icons';
import { cls, colorHex, formatTime } from '../utils';

interface Props {
  message: Message;
  meId: string;
  peers: Peer[];
  showSender: boolean;
  showAvatar: boolean;
  progress?: FileProgress;
  replyTo?: Message;
  onAccept: (id: string) => void;
  onDecline: (id: string) => void;
}

function highlight(text: string, names: string[]) {
  const valid = names.filter(Boolean);
  if (valid.length === 0) return text;
  const pattern = new RegExp(
    `(@(?:${valid
      .map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
      .join('|')}))\\b`,
    'gi',
  );
  const parts = text.split(pattern);
  return parts.map((part, index) =>
    index % 2 === 1 ? (
      <span className="mention" key={index}>
        {part}
      </span>
    ) : (
      part
    ),
  );
}

function StateIcon({ state }: { state: Message['state'] }) {
  if (state === 'failed') return <span className="text-[10px] font-bold">!</span>;
  if (state === 'read') return <IconCheckDouble size={14} />;
  if (state === 'delivered') return <IconCheckDouble size={14} />;
  if (state === 'sent') return <IconCheck size={13} />;
  if (state === 'queued' || state === 'sending') return <span className="text-[10px]">…</span>;
  return null;
}

export function MessageBubble({
  message,
  meId,
  peers,
  showSender,
  showAvatar,
  progress,
  replyTo,
  onAccept,
  onDecline,
}: Props) {
  const mine = message.outgoing || message.sender_id === meId;
  const peer = peers.find((item) => item.id === message.sender_id);
  const color = peer?.avatar_color ?? 0x9aa6b4;
  const mentionNames = message.mentions
    .map((id) => (id === meId ? 'You' : peers.find((p) => p.id === id)?.display_name ?? ''))
    .filter(Boolean);
  const hasAttachment =
    message.kind === 'image' ||
    message.kind === 'video' ||
    message.kind === 'audio' ||
    message.kind === 'file';

  if (message.kind === 'system') {
    return (
      <div className="my-2 flex justify-center">
        <span className="chip max-w-[80%] text-center">{message.text}</span>
      </div>
    );
  }

  return (
    <div className={cls('flex w-full gap-2.5', mine ? 'flex-row-reverse' : 'flex-row')}>
      <div className="w-8 shrink-0">
        {!mine && showAvatar && (
          <Avatar name={message.sender_name} color={color} size={30} />
        )}
      </div>
      <div className={cls('flex max-w-[min(78%,560px)] flex-col', mine ? 'items-end' : 'items-start')}>
        {showSender && !mine && (
          <span className="mb-1 ml-1 text-[11px] font-semibold" style={{ color: colorHex(color) }}>
            {message.sender_name}
          </span>
        )}
        <div
          className={cls(
            'rounded-2xl px-3.5 py-2 text-[13px] leading-relaxed shadow-sm',
            mine ? 'rounded-br-md text-white' : 'rounded-bl-md',
          )}
          style={
            mine
              ? { background: 'var(--own-bubble)' }
              : { background: 'var(--bg-soft)', border: '1px solid var(--border)', color: 'var(--text)' }
          }
        >
          {replyTo && (
            <div
              className={cls(
                'mb-1.5 rounded-lg border-l-2 px-2 py-1 text-[11px]',
                mine ? 'border-white/60 bg-white/10 text-white/85' : 'border-[var(--accent)]',
              )}
              style={!mine ? { background: 'var(--bg-elevated)', color: 'var(--muted)' } : undefined}
            >
              <span className="font-semibold">{replyTo.sender_name}</span>
              <div className="truncate opacity-80">{replyTo.text || 'Attachment'}</div>
            </div>
          )}

          {hasAttachment && (
            <Attachment
              message={message}
              progress={progress}
              compact
              onAccept={onAccept}
              onDecline={onDecline}
            />
          )}

          {message.text && message.text.trim() && (message.kind === 'text' || !hasAttachment) && (
            <p className="message-text whitespace-pre-wrap break-words">
              {highlight(message.text, mentionNames)}
            </p>
          )}
        </div>
        <span
          className="mt-1 flex items-center gap-1 px-1 text-[10px]"
          style={{ color: 'var(--muted)' }}
        >
          {formatTime(message.ts)}
          {mine && <StateIcon state={message.state} />}
        </span>
      </div>
    </div>
  );
}
