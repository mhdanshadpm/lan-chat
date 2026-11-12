import { useEffect, useRef } from 'react';
import type { Conversation, FileProgress, Message, Peer } from '../types';
import { Avatar } from './Avatar';
import { BrandMark } from './Brand';
import { Composer } from './Composer';
import { MessageBubble } from './MessageBubble';
import {
  IconBell,
  IconBellOff,
  IconChat,
  IconChevronLeft,
  IconSearch,
  IconUsers,
} from './Icons';
import { cls, dayLabel, formatTime, sameDay } from '../utils';

interface Props {
  conv: Conversation | null;
  meId: string;
  peers: Peer[];
  messages: Message[];
  typingName: string | null;
  transfers: Record<string, FileProgress>;
  hasMore: boolean;
  loading: boolean;
  onSend: (text: string) => Promise<void>;
  onAttach: (files: File[]) => Promise<void>;
  onTyping: (active: boolean) => void;
  onLoadMore: () => void;
  onMute: () => void;
  onManage: () => void;
  onSearch: () => void;
  onOpenBubble: () => void;
  onBack: () => void;
  onAccept: (id: string) => void;
  onDecline: (id: string) => void;
}

export function ChatView({
  conv,
  meId,
  peers,
  messages,
  typingName,
  transfers,
  hasMore,
  loading,
  onSend,
  onAttach,
  onTyping,
  onLoadMore,
  onMute,
  onManage,
  onSearch,
  onOpenBubble,
  onBack,
  onAccept,
  onDecline,
}: Props) {
  const listRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const lastConv = useRef<string | null>(null);
  const lastCount = useRef(0);

  useEffect(() => {
    if (!conv) return;
    if (lastConv.current !== conv.id) {
      lastConv.current = conv.id;
      stick.current = true;
      requestAnimationFrame(() => {
        const node = listRef.current;
        if (node) node.scrollTop = node.scrollHeight;
      });
    }
  }, [conv]);

  useEffect(() => {
    if (messages.length === lastCount.current) return;
    lastCount.current = messages.length;
    if (!stick.current) return;
    requestAnimationFrame(() => {
      const node = listRef.current;
      if (node) node.scrollTop = node.scrollHeight;
    });
  }, [messages.length, typingName]);

  if (!conv) {
    return (
      <div className="flex h-full flex-col items-center justify-center px-6 text-center">
        <BrandMark size={64} />
        <h2 className="mt-5 text-xl font-semibold tracking-tight">Pick a conversation</h2>
        <p className="mt-2 max-w-sm text-sm leading-relaxed" style={{ color: 'var(--muted)' }}>
          People on your network appear in the sidebar automatically. Messages stay on your
          devices — nothing is sent to the internet.
        </p>
        <div className="mt-6 flex flex-wrap justify-center gap-2">
          {peers.slice(0, 4).map((peer) => (
            <span key={peer.id} className="chip">
              <span
                className="inline-block h-2 w-2 rounded-full"
                style={{ background: peer.online ? '#22c55e' : '#9aa4b2' }}
              />
              {peer.display_name}
            </span>
          ))}
          {peers.length === 0 && <span className="chip">Waiting for peers…</span>}
        </div>
        <p className="mt-8 max-w-xs text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
          Tip: press <kbd className="chip">Ctrl/Cmd+Shift+Space</kbd> to summon the quick-reply
          bubble from anywhere.
        </p>
      </div>
    );
  }

  const kind = conv.kind === 'group' ? (conv.is_everyone ? 'everyone' : 'group') : 'direct';
  const peer = conv.kind === 'direct' ? peers.find((item) => item.id === conv.id) : undefined;
  const memberLabel = `${conv.member_count} ${conv.member_count === 1 ? 'member' : 'members'}`;
  const subtitle =
    conv.kind === 'group'
      ? conv.is_everyone
        ? `Everyone on the LAN · ${memberLabel}`
        : memberLabel
      : peer?.online
        ? 'Online now'
        : peer
          ? `Last seen ${formatTime(peer.last_seen)}`
          : 'Offline';

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header
        className="flex shrink-0 items-center gap-3 border-b px-3 py-2.5 divider sm:px-4"
        style={{ background: 'var(--bg-elevated)' }}
      >
        <button className="icon-btn md:hidden" onClick={onBack} aria-label="Back to conversations">
          <IconChevronLeft size={20} />
        </button>
        <Avatar
          name={conv.title}
          color={conv.avatar_color}
          size={38}
          kind={kind}
          online={conv.kind === 'direct' ? conv.online : null}
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <h1 className="truncate text-sm font-semibold">{conv.title}</h1>
            {conv.muted && <IconBellOff size={13} />}
          </div>
          <p className="truncate text-[11px]" style={{ color: 'var(--muted)' }}>
            {typingName ? `${typingName} is typing…` : subtitle}
          </p>
        </div>
        <button className="icon-btn" onClick={onSearch} title="Search messages" aria-label="Search messages">
          <IconSearch size={18} />
        </button>
        <button
          className="icon-btn"
          onClick={onMute}
          title={conv.muted ? 'Unmute conversation' : 'Mute conversation'}
          aria-label={conv.muted ? 'Unmute conversation' : 'Mute conversation'}
        >
          {conv.muted ? <IconBellOff size={18} /> : <IconBell size={18} />}
        </button>
        {conv.kind === 'group' && (
          <button
            className={cls('icon-btn', conv.is_everyone && 'opacity-50')}
            onClick={onManage}
            title={conv.is_everyone ? 'Everyone group' : 'Manage group'}
            aria-label="Manage group"
          >
            <IconUsers size={18} />
          </button>
        )}
        <button className="icon-btn" onClick={onOpenBubble} title="Open quick-reply bubble" aria-label="Open bubble">
          <IconChat size={18} />
        </button>
      </header>

      <div
        ref={listRef}
        className="scroll-thin min-h-0 flex-1 overflow-y-auto px-3 py-4 sm:px-6"
        onScroll={(event) => {
          const node = event.currentTarget;
          stick.current = node.scrollHeight - node.scrollTop - node.clientHeight < 160;
        }}
      >
        {hasMore && (
          <div className="mb-4 flex justify-center">
            <button className="btn-ghost text-xs" onClick={onLoadMore} disabled={loading}>
              {loading ? 'Loading…' : 'Load earlier messages'}
            </button>
          </div>
        )}

        {messages.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center text-center">
            <p className="text-sm font-medium">No messages yet</p>
            <p className="mt-1 text-xs" style={{ color: 'var(--muted)' }}>
              Say hello to {conv.title}.
            </p>
          </div>
        )}

        <div className="flex flex-col gap-1.5">
          {messages.map((message, index) => {
            const previous = messages[index - 1];
            const next = messages[index + 1];
            const newDay = !previous || !sameDay(previous.ts, message.ts);
            const showSender = conv.kind === 'group' && !message.outgoing;
            const showAvatar = showSender && previous?.sender_id !== message.sender_id;
            const grouped = next && next.sender_id === message.sender_id && !newDay && sameDay(next.ts, message.ts);
            return (
              <div key={message.id}>
                {newDay && (
                  <div className="my-4 flex items-center gap-3">
                    <span className="h-px flex-1" style={{ background: 'var(--border)' }} />
                    <span className="text-[10px] font-semibold uppercase tracking-wide" style={{ color: 'var(--muted)' }}>
                      {dayLabel(message.ts)}
                    </span>
                    <span className="h-px flex-1" style={{ background: 'var(--border)' }} />
                  </div>
                )}
                <div className={cls(grouped ? 'mt-0.5' : 'mt-2')}>
                  <MessageBubble
                    message={message}
                    meId={meId}
                    peers={peers}
                    showSender={showSender}
                    showAvatar={Boolean(showAvatar)}
                    progress={message.file_id ? transfers[message.file_id] : undefined}
                    replyTo={
                      message.reply_to ? messages.find((item) => item.id === message.reply_to) : undefined
                    }
                    onAccept={onAccept}
                    onDecline={onDecline}
                  />
                </div>
              </div>
            );
          })}
        </div>

        {typingName && (
          <div className="mt-3 flex items-center gap-2 pl-1">
            <div className="typing-dots flex items-center gap-1 rounded-2xl px-3 py-2" style={{ background: 'var(--bg-soft)' }}>
              <span />
              <span />
              <span />
            </div>
            <span className="text-[11px]" style={{ color: 'var(--muted)' }}>
              {typingName} is typing…
            </span>
          </div>
        )}
      </div>

      <Composer
        disabled={false}
        placeholder={`Message ${conv.title}…`}
        onSend={onSend}
        onAttach={onAttach}
        onTyping={onTyping}
      />
    </div>
  );
}
