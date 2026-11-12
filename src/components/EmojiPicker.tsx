import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { IconSmile } from './Icons';

const EMOJIS = [
  '😀', '😄', '😁', '😊', '🙂', '😉', '😍', '🤩',
  '😂', '🤣', '😅', '🙃', '😇', '🤔', '🤨', '😐',
  '😴', '🥱', '😮', '😲', '🥳', '😎', '🤝', '🙌',
  '👏', '👍', '👎', '👌', '✌️', '🤞', '💪', '🙏',
  '❤️', '🧡', '💛', '💚', '💙', '💜', '🖤', '💯',
  '🔥', '✨', '🎉', '🎈', '☕', '🍕', '🌮', '🍻',
  '🚀', '💡', '📌', '📎', '✅', '❌', '⏰', '👀',
];

interface Props {
  onPick: (emoji: string) => void;
  align?: 'left' | 'right';
}

export function EmojiPicker({ onPick, align = 'right' }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', close);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        className="icon-btn"
        aria-label="Emoji picker"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <IconSmile size={19} />
      </button>
      {open && (
        <div
          className="card absolute bottom-11 z-30 w-64 animate-pop p-2 shadow-soft"
          style={{ [align]: 0 } as CSSProperties}
        >
          <div className="grid grid-cols-8 gap-0.5">
            {EMOJIS.map((emoji) => (
              <button
                key={emoji}
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-md text-lg leading-none hover:bg-[var(--bg-hover)]"
                onClick={() => {
                  onPick(emoji);
                  setOpen(false);
                }}
              >
                {emoji}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
