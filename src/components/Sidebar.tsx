import { useMemo, useState } from 'react';
import type { Conversation, Identity } from '../types';
import { Avatar } from './Avatar';
import { BrandMark } from './Brand';
import {
  IconBell,
  IconBellOff,
  IconMoon,
  IconPlus,
  IconSearch,
  IconSettings,
  IconSun,
  IconWifi,
} from './Icons';
import { cls, relativeListTime } from '../utils';

type Topic = 'all' | 'direct' | 'groups';

interface Props {
  identity: Identity;
  conversations: Conversation[];
  activeId: string | null;
  onlineCount: number;
  port: number;
  dnd: boolean;
  dark: boolean;
  tauri: boolean;
  onSelect: (id: string) => void;
  onSettings: () => void;
  onNewGroup: () => void;
  onToggleDnd: () => void;
  onToggleTheme: () => void;
}

export function Sidebar({
  identity,
  conversations,
  activeId,
  onlineCount,
  port,
  dnd,
  dark,
  tauri,
  onSelect,
  onSettings,
  onNewGroup,
  onToggleDnd,
  onToggleTheme,
}: Props) {
  const [query, setQuery] = useState('');
  const [topic, setTopic] = useState<Topic>('all');

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return conversations.filter((conv) => {
      if (topic === 'direct' && conv.kind !== 'direct') return false;
      if (topic === 'groups' && conv.kind !== 'group') return false;
      if (!needle) return true;
      return (
        conv.title.toLowerCase().includes(needle) ||
        conv.last_preview.toLowerCase().includes(needle) ||
        conv.last_sender.toLowerCase().includes(needle)
      );
    });
  }, [conversations, query, topic]);

  const unreadTotal = conversations.reduce((sum, conv) => sum + conv.unread, 0);

  return (
    <aside className="flex h-full w-full flex-col border-r divider" style={{ background: 'var(--bg-elevated)' }}>
      <div className="flex items-center gap-2.5 px-4 pb-3 pt-4">
        <BrandMark size={32} />
        <div className="min-w-0">
          <div className="text-sm font-semibold leading-tight">LAN Chat</div>
          <div className="flex items-center gap-1 text-[11px]" style={{ color: 'var(--muted)' }}>
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-500" />
            {onlineCount} online · port {port}
          </div>
        </div>
        <button className="icon-btn ml-auto" onClick={onSettings} aria-label="Settings">
          <IconSettings size={19} />
        </button>
      </div>

      <div className="mx-3 mb-3 flex items-center gap-2.5 rounded-2xl px-3 py-2" style={{ background: 'var(--bg-soft)' }}>
        <Avatar name={identity.display_name || 'You'} color={identity.avatar_color} size={34} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold">{identity.display_name || 'You'}</div>
          <div className="truncate text-[11px]" style={{ color: 'var(--muted)' }}>
            {identity.device_name}
          </div>
        </div>
        {dnd && (
          <span className="chip" title="Do not disturb">
            <IconBellOff size={12} /> DND
          </span>
        )}
      </div>

      <div className="px-3">
        <div className="relative">
          <IconSearch
            size={16}
            className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2"
          />
          <input
            className="field pl-9"
            placeholder="Filter conversations"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            style={{ background: 'var(--bg-soft)' }}
          />
        </div>
      </div>

      <div className="mx-3 mt-3 grid grid-cols-3 gap-1 rounded-xl p-0.5" style={{ background: 'var(--bg-soft)' }}>
        {(
          [
            ['all', 'All'],
            ['direct', 'Chats'],
            ['groups', 'Groups'],
          ] as const
        ).map(([value, label]) => (
          <button
            key={value}
            onClick={() => setTopic(value)}
            className={cls(
              'rounded-lg py-1.5 text-xs font-semibold transition-colors',
              topic === value ? 'shadow-sm' : 'opacity-70 hover:opacity-100',
            )}
            style={{
              background: topic === value ? 'var(--bg-elevated)' : 'transparent',
              color: 'var(--text)',
            }}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="scroll-thin mt-2 min-h-0 flex-1 overflow-y-auto px-2 pb-3">
        {visible.length === 0 && (
          <p className="px-3 py-8 text-center text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
            {conversations.length === 0
              ? 'No conversations yet. People on your network appear here automatically.'
              : 'Nothing matches that filter.'}
          </p>
        )}
        {visible.map((conv) => (
          <ConversationRow
            key={conv.id}
            conv={conv}
            active={conv.id === activeId}
            onClick={() => onSelect(conv.id)}
          />
        ))}
      </div>

      <div className="border-t px-3 py-3 divider">
        <div className="mb-2 flex items-center justify-between">
          <span className="flex items-center gap-1.5 text-[11px] font-medium" style={{ color: 'var(--muted)' }}>
            <IconWifi size={14} />
            {tauri ? 'LAN discovery active' : 'Browser preview'}
          </span>
          <span className="chip">{unreadTotal > 0 ? `${unreadTotal} unread` : 'All caught up'}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <button className="btn-soft flex-1 justify-start" onClick={onNewGroup}>
            <IconPlus size={16} /> New group
          </button>
          <button className="icon-btn" onClick={onToggleDnd} aria-label="Toggle do not disturb" title="Do not disturb">
            {dnd ? <IconBellOff size={18} /> : <IconBell size={18} />}
          </button>
          <button className="icon-btn" onClick={onToggleTheme} aria-label="Toggle theme" title="Toggle theme">
            {dark ? <IconSun size={18} /> : <IconMoon size={18} />}
          </button>
        </div>
      </div>
    </aside>
  );
}

function ConversationRow({
  conv,
  active,
  onClick,
}: {
  conv: Conversation;
  active: boolean;
  onClick: () => void;
}) {
  const kind = conv.kind === 'group' ? (conv.is_everyone ? 'everyone' : 'group') : 'direct';
  const preview =
    conv.kind === 'group' && conv.last_sender
      ? `${conv.last_sender.split(' ')[0]}: ${conv.last_preview}`
      : conv.last_preview;

  return (
    <button className={cls('nav-item', active && 'nav-item-active')} onClick={onClick}>
      <Avatar
        name={conv.title}
        color={conv.avatar_color}
        size={40}
        kind={kind}
        online={conv.kind === 'direct' ? conv.online : null}
      />
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline justify-between gap-2">
          <span className="truncate text-sm font-semibold">{conv.title}</span>
          <span className="shrink-0 text-[10px] tabular-nums" style={{ color: 'var(--muted)' }}>
            {relativeListTime(conv.last_ts)}
          </span>
        </span>
        <span className="mt-0.5 flex items-center gap-1.5">
          <span
            className={cls('truncate text-xs', conv.unread > 0 && 'font-medium')}
            style={{ color: conv.unread > 0 ? 'var(--text)' : 'var(--muted)' }}
          >
            {preview || 'No messages yet'}
          </span>
          <span className="ml-auto flex shrink-0 items-center gap-1">
            {conv.mentioned && conv.unread > 0 && !conv.muted && (
              <span className="text-[10px] font-bold uppercase" style={{ color: 'var(--accent)' }}>
                @
              </span>
            )}
            {conv.muted && <IconBellOff size={12} />}
            {conv.unread > 0 && (
              <span
                className="inline-flex h-5 min-w-[20px] items-center justify-center rounded-full px-1.5 text-[10px] font-bold text-white"
                style={{ background: conv.mentioned ? 'var(--accent)' : conv.muted ? 'var(--muted)' : '#ef4444' }}
              >
                {conv.unread > 99 ? '99+' : conv.unread}
              </span>
            )}
          </span>
        </span>
      </span>
    </button>
  );
}
