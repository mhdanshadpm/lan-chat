import { useEffect, useRef, useState } from 'react';
import { EmojiPicker } from './EmojiPicker';
import { IconPaperclip, IconSend } from './Icons';

interface Props {
  disabled?: boolean;
  placeholder?: string;
  onSend: (text: string) => Promise<void> | void;
  onAttach: (files: File[]) => Promise<void> | void;
  onTyping: (active: boolean) => void;
}

export function Composer({ disabled, placeholder, onSend, onAttach, onTyping }: Props) {
  const [text, setText] = useState('');
  const [sending, setSending] = useState(false);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const typingTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    return () => {
      if (typingTimer.current) window.clearTimeout(typingTimer.current);
    };
  }, []);

  const resize = () => {
    const area = areaRef.current;
    if (!area) return;
    area.style.height = 'auto';
    area.style.height = `${Math.min(area.scrollHeight, 140)}px`;
  };

  const signalTyping = () => {
    onTyping(true);
    if (typingTimer.current) window.clearTimeout(typingTimer.current);
    typingTimer.current = window.setTimeout(() => onTyping(false), 2500);
  };

  const submit = async () => {
    const value = text.trim();
    if (!value || disabled || sending) return;
    setSending(true);
    setText('');
    onTyping(false);
    if (areaRef.current) areaRef.current.style.height = 'auto';
    try {
      await onSend(value);
    } finally {
      setSending(false);
      areaRef.current?.focus();
    }
  };

  const attach = async (files: FileList | File[] | null) => {
    if (!files) return;
    const list = Array.from(files);
    if (list.length === 0) return;
    await onAttach(list);
  };

  return (
    <div className="border-t px-3 pb-3 pt-2.5 divider" style={{ background: 'var(--bg-elevated)' }}>
      <div
        className="flex items-end gap-1.5 rounded-2xl border p-1.5 transition-shadow focus-within:shadow-[0_0_0_3px_var(--accent-soft)] divider"
        style={{ background: 'var(--bg-soft)' }}
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          void attach(event.dataTransfer.files);
        }}
      >
        <button
          type="button"
          className="icon-btn"
          disabled={disabled}
          onClick={() => fileRef.current?.click()}
          aria-label="Attach files"
          title="Attach photo, video or file"
        >
          <IconPaperclip size={19} />
        </button>
        <input
          ref={fileRef}
          type="file"
          multiple
          className="hidden"
          onChange={(event) => {
            void attach(event.target.files);
            event.target.value = '';
          }}
        />
        <textarea
          ref={areaRef}
          rows={1}
          className="max-h-[140px] min-h-[38px] flex-1 resize-none bg-transparent px-1.5 py-2 text-sm leading-5 outline-none"
          style={{ color: 'var(--text)' }}
          placeholder={placeholder ?? 'Write a message…'}
          value={text}
          disabled={disabled}
          onChange={(event) => {
            setText(event.target.value);
            resize();
            signalTyping();
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault();
              void submit();
            }
          }}
          onPaste={(event) => {
            const files = Array.from(event.clipboardData.files);
            if (files.length > 0) {
              event.preventDefault();
              void attach(files);
            }
          }}
        />
        <EmojiPicker
          onPick={(emoji) => {
            const area = areaRef.current;
            const start = area?.selectionStart ?? text.length;
            const end = area?.selectionEnd ?? text.length;
            const next = `${text.slice(0, start)}${emoji}${text.slice(end)}`;
            setText(next);
            requestAnimationFrame(() => {
              area?.focus();
              area?.setSelectionRange(start + emoji.length, start + emoji.length);
              resize();
            });
          }}
        />
        <button
          type="button"
          className="btn-primary h-9 w-9 !p-0"
          disabled={disabled || !text.trim() || sending}
          onClick={() => void submit()}
          aria-label="Send message"
        >
          <IconSend size={17} />
        </button>
      </div>
      <p className="mt-2 hidden text-[10px] sm:block" style={{ color: 'var(--muted)' }}>
        Enter to send · Shift+Enter for a new line · Paste or drop files to attach
      </p>
    </div>
  );
}
