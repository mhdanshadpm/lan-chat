import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import type { Conversation, Message } from '../types';
import { Modal } from './Modal';
import { Avatar } from './Avatar';
import { IconSearch } from './Icons';
import { formatTime, dayLabel } from '../utils';

interface Props {
  conversations: Conversation[];
  onClose: () => void;
  onOpen: (conv: string) => void;
}

export function SearchModal({ conversations, onClose, onOpen }: Props) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Message[]>([]);
  const [searching, setSearching] = useState(false);
  const [touched, setTouched] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const needle = query.trim();
    if (needle.length < 2) {
      setResults([]);
      setTouched(false);
      setSearching(false);
      return;
    }
    let alive = true;
    setSearching(true);
    const timer = window.setTimeout(() => {
      api
        .searchMessages(needle, 100)
        .then((found) => {
          if (alive) setResults(found);
        })
        .catch(() => {
          if (alive) setResults([]);
        })
        .finally(() => {
          if (alive) {
            setSearching(false);
            setTouched(true);
          }
        });
    }, 240);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [query]);

  const conversation = (id: string) => conversations.find((item) => item.id === id);

  return (
    <Modal
      title="Search messages"
      subtitle="Searches the full history stored on this device."
      onClose={onClose}
      wide
      bodyClassName="!py-0"
      footer={
        <button className="btn-ghost" onClick={onClose}>
          Close
        </button>
      }
    >
      <div className="sticky top-0 z-10 -mx-5 border-b px-5 py-3 divider" style={{ background: 'var(--bg-elevated)' }}>
        <div className="relative">
          <IconSearch size={16} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            ref={inputRef}
            className="field pl-9"
            placeholder="Search text, names, file captions…"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
      </div>

      <div className="py-3">
        {query.trim().length < 2 && (
          <p className="py-6 text-center text-xs leading-relaxed" style={{ color: 'var(--muted)' }}>
            Type at least two characters. Search covers every conversation, including attachments'
            captions.
          </p>
        )}

        {query.trim().length >= 2 && searching && (
          <p className="py-6 text-center text-xs" style={{ color: 'var(--muted)' }}>
            Searching…
          </p>
        )}

        {query.trim().length >= 2 && !searching && touched && results.length === 0 && (
          <p className="py-6 text-center text-xs" style={{ color: 'var(--muted)' }}>
            No messages match “{query.trim()}”.
          </p>
        )}

        <div className="space-y-1">
          {results.map((message) => {
            const conv = conversation(message.conv);
            if (!conv) return null;
            const kind = conv.kind === 'group' ? (conv.is_everyone ? 'everyone' : 'group') : 'direct';
            return (
              <button
                key={message.id}
                className="flex w-full items-start gap-3 rounded-xl px-2 py-2 text-left transition-colors hover:bg-[var(--bg-hover)]"
                onClick={() => {
                  onOpen(message.conv);
                  onClose();
                }}
              >
                <Avatar name={conv.title} color={conv.avatar_color} size={34} kind={kind} />
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline gap-2">
                    <span className="truncate text-sm font-semibold">{conv.title}</span>
                    <span className="ml-auto shrink-0 text-[10px]" style={{ color: 'var(--muted)' }}>
                      {dayLabel(message.ts)} · {formatTime(message.ts)}
                    </span>
                  </span>
                  <span className="mt-0.5 block truncate text-xs" style={{ color: 'var(--muted)' }}>
                    <span className="font-medium" style={{ color: 'var(--text)' }}>
                      {message.outgoing ? 'You' : message.sender_name}:
                    </span>{' '}
                    {message.text?.trim() ||
                      (message.kind === 'image'
                        ? 'Photo'
                        : message.kind === 'video'
                          ? 'Video'
                          : message.kind === 'audio'
                            ? 'Voice message'
                            : 'Attachment')}
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </Modal>
  );
}
